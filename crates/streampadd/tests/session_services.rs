//! Desktop-session host calls against mock services on a private D-Bus: notifications, KDE global shortcuts,
//! and the "not supported on this desktop" errors when KWin is absent. Nothing reaches the real session.

mod common;

use common::Daemon;
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use zbus::zvariant::OwnedValue;

type Notification = (String, String, String, String, Option<String>);

struct Notifications {
    received: Arc<Mutex<Vec<Notification>>>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: String,
        _replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        _actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        _expire_timeout: i32,
    ) -> u32 {
        let entry = hints.get("desktop-entry").and_then(|v| String::try_from(v.clone()).ok());
        self.received.lock().unwrap().push((app_name, app_icon, summary, body, entry));
        1
    }
}

struct ShortcutComponent {
    invoked: Arc<Mutex<Vec<String>>>,
}

#[zbus::interface(name = "org.kde.kglobalaccel.Component")]
impl ShortcutComponent {
    #[zbus(name = "shortcutNames")]
    async fn shortcut_names(&self) -> Vec<String> {
        vec!["Overview".into(), "Window Maximize".into()]
    }

    #[zbus(name = "invokeShortcut")]
    async fn invoke_shortcut(&self, name: String) {
        self.invoked.lock().unwrap().push(name);
    }
}

/// `dbus-daemon` with no service directories, so nothing can be auto-started.
struct PrivateBus {
    child: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl PrivateBus {
    fn start() -> Option<PrivateBus> {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("bus");
        let config = dir.path().join("bus.conf");
        std::fs::write(
            &config,
            format!(
                r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={}</listen>
  <policy context="default"><allow send_destination="*"/><allow own="*"/></policy>
</busconfig>"#,
                socket.display()
            ),
        )
        .unwrap();
        let mut child = Command::new("dbus-daemon")
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address"])
            .stdout(Stdio::piped())
            .spawn()
            .ok()?;
        let mut address = String::new();
        BufReader::new(child.stdout.take()?).read_line(&mut address).ok()?;
        Some(PrivateBus { child, address: address.trim().to_string(), _dir: dir })
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn notify_and_kde_shortcuts_reach_the_session_and_window_features_fail_clearly() {
    let Some(bus) = PrivateBus::start() else {
        eprintln!("skipping: dbus-daemon is not available");
        return;
    };
    let notifications = Arc::new(Mutex::new(Vec::new()));
    let invoked = Arc::new(Mutex::new(Vec::new()));
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _services = rt.block_on(async {
        zbus::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name("org.freedesktop.Notifications")
            .unwrap()
            .name("org.kde.kglobalaccel")
            .unwrap()
            .serve_at("/org/freedesktop/Notifications", Notifications { received: notifications.clone() })
            .unwrap()
            .serve_at("/component/kwin", ShortcutComponent { invoked: invoked.clone() })
            .unwrap()
            .build()
            .await
            .unwrap()
    });

    let daemon = Daemon::start_with_env(None, &["--no-input", "--no-overlay"], &[("DBUS_SESSION_BUS_ADDRESS", &bus.address)]);
    let mut c = daemon.client();
    let hello = c.ok("hello", json!({ "client": "editor", "subscribe": true }));
    assert_eq!(hello["state"]["integrations"]["kwin"], false, "no KWin on this bus");

    let script = r#"
        notify("Hello", "from a macro");
        kde.shortcut("kwin", "Overview");
        const attempt = (label, f) => { try { f(); log(label, "ok"); } catch (e) { log(label, e.message); } };
        attempt("unknown shortcut:", () => kde.shortcut("kwin", "Nope"));
        attempt("unknown component:", () => kde.shortcut("org.example.none", "X"));
        attempt("maximize:", () => window.maximize());
        attempt("overview:", () => desktop.overview());
        log("active:", JSON.stringify(window.active()), "list:", JSON.stringify(window.list()), "running:", window.isRunning("x"));
    "#;
    let run = c.ok("runMacro", json!({ "macro": { "type": "script", "source": script } }));
    let run_id = run["runId"].clone();
    let finished = c.wait_event("run", |r| r["runId"] == run_id && r["status"] != "started");
    assert_eq!(finished["status"], "finished", "{finished}");
    let logs: Vec<String> = std::mem::take(&mut c.events)
        .into_iter()
        .filter(|(n, d)| n == "log" && d["runId"] == run_id)
        .map(|(_, d)| d["message"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(logs[0], "unknown shortcut: KDE component \"kwin\" has no shortcut \"Nope\"");
    assert!(logs[1].starts_with("unknown component: no KDE shortcut component \"org.example.none\""), "{}", logs[1]);
    assert!(logs[2].contains("not supported on this desktop"), "{}", logs[2]);
    assert!(logs[3].contains("not supported on this desktop"), "{}", logs[3]);
    assert_eq!(logs[4], "active: null list: [] running: false", "queries degrade gracefully off KDE");

    let received = notifications.lock().unwrap().clone();
    assert_eq!(
        received,
        [("StreamPad".to_string(), "io.github.ometavr.streampad".to_string(), "Hello".to_string(), "from a macro".to_string(), Some("io.github.ometavr.streampad".to_string()))]
    );
    assert_eq!(*invoked.lock().unwrap(), ["Overview"]);

    // Uncaught, the same error fails the run and shows an error toast.
    let run = c.ok("runMacro", json!({ "macro": { "type": "script", "source": "window.center();" } }));
    let run_id = run["runId"].clone();
    let toast = c.wait_event("toast", |t| t["kind"] == "error");
    assert!(toast["text"].as_str().unwrap().starts_with("Test run: "), "{toast}");
    let failed = c.wait_event("run", |r| r["runId"] == run_id && r["status"] == "failed");
    assert!(failed["error"].as_str().unwrap().contains("not supported on this desktop"));
}
