//! Child processes: the overlay supervisor (restart with backoff, dies with the daemon), launching the editor
//! (`openEditor` and the hold-Enter global), and restoring state after a restart. Helper programs are fake
//! scripts on a private `PATH`.

mod common;

use common::{Daemon, WAIT, wait_for};
use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn write_script(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn path_with(bin: &Path) -> String {
    format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default())
}

fn lines(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).map(|s| s.lines().map(str::to_string).collect()).unwrap_or_default()
}

fn alive(pid: u32) -> bool {
    // Zombies count as gone: only their exit status is left.
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| !s.contains(") Z "))
}

#[test]
fn overlay_is_restarted_with_backoff_and_dies_with_the_daemon() {
    let tools = tempfile::tempdir().unwrap();
    let log = tools.path().join("overlay.log");
    // Crashes twice, then stays up (exec keeps the pid we logged).
    let script = format!(
        r#"echo "$$ $STREAMPAD_SOCKET" >> '{log}'
[ "$(wc -l < '{log}')" -ge 3 ] && exec sleep 300
exit 1"#,
        log = log.display()
    );
    write_script(tools.path(), "streampad-overlay", &script);
    let started = Instant::now();
    let mut daemon = Daemon::start_with_env(None, &["--no-input"], &[("PATH", &path_with(tools.path()))]);

    let deadline = Instant::now() + WAIT;
    while lines(&log).len() < 3 {
        assert!(Instant::now() < deadline, "overlay was not restarted: {:?}", lines(&log));
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(started.elapsed() >= Duration::from_millis(2800), "restarts back off 1 s, then 2 s");
    let runs = lines(&log);
    let socket = daemon.socket().display().to_string();
    assert!(runs.iter().all(|l| l.ends_with(&socket)), "STREAMPAD_SOCKET is passed: {runs:?}");

    let overlay_pid: u32 = runs[2].split_whitespace().next().unwrap().parse().unwrap();
    assert!(alive(overlay_pid));
    daemon.kill();
    wait_for("the overlay to follow the killed daemon (PR_SET_PDEATHSIG)", || !alive(overlay_pid));
}

#[test]
fn open_editor_launches_it_detached_and_hold_enter_does_too() {
    let tools = tempfile::tempdir().unwrap();
    let marker = tools.path().join("editor.log");
    write_script(tools.path(), "streampad", &format!("echo \"$PPID\" >> '{}'", marker.display()));
    let daemon = Daemon::start_with_env(None, &["--no-input", "--no-overlay"], &[("PATH", &path_with(tools.path()))]);
    let mut c = daemon.client();
    assert_eq!(c.ok("openEditor", Value::Null), json!({}));
    wait_for("the editor to start", || lines(&marker).len() == 1);
    let parent: u32 = lines(&marker)[0].parse().unwrap();
    assert_ne!(parent, daemon.pid(), "the editor is not the daemon's child");

    // Hold Numpad Enter past holdMs (600 ms) in macro mode.
    c.ok("hello", json!({ "client": "cli", "subscribe": true }));
    c.ok("setMacroMode", json!({ "on": true }));
    c.ok("simulateKey", json!({ "key": "KPEnter", "pressed": true }));
    std::thread::sleep(Duration::from_millis(800));
    c.ok("simulateKey", json!({ "key": "KPEnter", "pressed": false }));
    wait_for("hold-Enter to open the editor", || lines(&marker).len() == 2);
    assert_eq!(c.ok("getState", Value::Null)["overlay"]["pinned"], false, "a hold does not also tap");
}

#[test]
fn missing_editor_is_a_clear_error() {
    let empty = tempfile::tempdir().unwrap();
    let daemon = Daemon::start_with_env(None, &["--no-input", "--no-overlay"], &[("PATH", &empty.path().display().to_string())]);
    let err = daemon.client().call("openEditor", Value::Null).unwrap_err();
    assert!(err.contains("not installed"), "{err}");
}

#[test]
fn state_is_restored_after_a_restart() {
    let mut daemon = Daemon::start(None);
    let mut c = daemon.client();
    c.ok("setMacroMode", json!({ "on": true }));
    c.ok("setProfile", json!({ "index": 3 }));
    c.ok("overlay", json!({ "action": "show" }));
    c.ok("setVar", json!({ "name": "gameMode", "value": true }));
    c.ok("quit", Value::Null);
    assert!(daemon.wait_exit().success());
    let before: PathBuf = daemon.state_file();
    assert!(before.exists());

    daemon.restart();
    let state = daemon.client().ok("getState", Value::Null);
    assert_eq!((state["macroMode"].clone(), state["profileIndex"].clone()), (json!(true), json!(3)));
    assert_eq!(state["overlay"]["pinned"], true);
    assert_eq!(daemon.client().ok("getVars", Value::Null), json!({ "gameMode": true }));
}
