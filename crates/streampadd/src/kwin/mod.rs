//! KWin bridge (docs/SPEC.md §5). Queries and window/desktop commands run as one-shot KWin scripts that answer
//! through our D-Bus object; a persistent watcher script pushes active-window and output changes.
//!
//! Scripts call back to this connection's *unique* bus name, so several daemons (tests) never cross wires;
//! the well-known name is still requested for discoverability. Calls are only accepted from KWin itself.

use crate::command::{Command, CommandTx};
use futures_util::StreamExt;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use streampad_core::ipc::{OutputInfo, WindowInfo};
use streampad_core::paths::Paths;
use tokio::sync::oneshot;
use zbus::fdo::{DBusProxy, RequestNameFlags};
use zbus::message::Header;
use zbus::names::WellKnownName;

pub const BUS_NAME: &str = "io.github.ometavr.StreamPad";
pub const OBJECT_PATH: &str = "/io/github/ometavr/StreamPad";
pub const INTERFACE: &str = "io.github.ometavr.StreamPad.KWin";

const KWIN: &str = "org.kde.KWin";
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(1);
pub const UNSUPPORTED: &str = "window management is not supported on this desktop (needs KDE Plasma / KWin)";

const PRELUDE_JS: &str = include_str!("prelude.js");
const OPS_JS: &str = include_str!("ops.js");
const WATCHER_JS: &str = include_str!("watcher.js");

/// State shared with the D-Bus object that receives script callbacks.
struct Shared {
    pending: Mutex<HashMap<String, oneshot::Sender<String>>>,
    kwin_owner: Mutex<Option<String>>,
    active: Mutex<Option<WindowInfo>>,
    outputs: Mutex<Vec<OutputInfo>>,
    available: AtomicBool,
    events: CommandTx,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    fn is_from_kwin(&self, header: &Header<'_>) -> bool {
        let sender = header.sender().map(|s| s.as_str());
        sender.is_some() && lock(&self.kwin_owner).as_deref() == sender
    }

    fn notify_status(&self) {
        let outputs = lock(&self.outputs).clone();
        let available = self.available.load(Ordering::Relaxed);
        let _ = self.events.send(Command::Kwin { available, outputs });
    }

    fn handle_event(&self, json: &str) {
        let Ok(ev) = serde_json::from_str::<Value>(json) else {
            log::warn!("kwin watcher sent invalid JSON");
            return;
        };
        match ev.get("type").and_then(Value::as_str) {
            Some("activeWindow") => {
                *lock(&self.active) = ev.get("window").cloned().and_then(|w| serde_json::from_value(w).ok());
            }
            Some("outputs") => {
                let outputs = ev.get("outputs").cloned().and_then(|o| serde_json::from_value(o).ok()).unwrap_or_default();
                *lock(&self.outputs) = outputs;
                self.notify_status();
            }
            other => log::debug!("kwin watcher: ignoring event {other:?}"),
        }
    }
}

struct BridgeObject {
    shared: Arc<Shared>,
}

#[zbus::interface(name = "io.github.ometavr.StreamPad.KWin")]
impl BridgeObject {
    async fn reply(&self, token: String, json: String, #[zbus(header)] header: Header<'_>) {
        if self.shared.is_from_kwin(&header)
            && let Some(tx) = lock(&self.shared.pending).remove(&token)
        {
            let _ = tx.send(json);
        }
    }

    async fn event(&self, json: String, #[zbus(header)] header: Header<'_>) {
        if self.shared.is_from_kwin(&header) {
            self.shared.handle_event(&json);
        }
    }
}

pub struct Kwin {
    conn: zbus::Connection,
    shared: Arc<Shared>,
    scripts_dir: PathBuf,
    watcher_name: String,
    seq: AtomicU64,
}

impl Kwin {
    /// Serve the callback object, load the watcher if KWin is running, and follow KWin restarts.
    pub async fn start(conn: zbus::Connection, paths: &Paths, events: CommandTx) -> Arc<Kwin> {
        let shared = Arc::new(Shared {
            pending: Mutex::new(HashMap::new()),
            kwin_owner: Mutex::new(None),
            active: Mutex::new(None),
            outputs: Mutex::new(Vec::new()),
            available: AtomicBool::new(false),
            events,
        });
        if let Err(e) = conn.object_server().at(OBJECT_PATH, BridgeObject { shared: shared.clone() }).await {
            log::warn!("cannot export the KWin bridge object: {e}");
        }
        match conn.request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into()).await {
            Ok(zbus::fdo::RequestNameReply::PrimaryOwner | zbus::fdo::RequestNameReply::AlreadyOwner) => {}
            Ok(_) => log::info!("{BUS_NAME} is owned by another StreamPad daemon; using the unique bus name"),
            Err(e) => log::warn!("cannot own {BUS_NAME}: {e}"),
        }
        let kwin = Arc::new(Kwin {
            conn,
            shared,
            scripts_dir: paths.kwin_scripts_dir(),
            watcher_name: format!("streampad-watcher-{:016x}", fnv1a(paths.socket.to_string_lossy().as_bytes())),
            seq: AtomicU64::new(0),
        });
        kwin.refresh().await;
        tokio::spawn(follow_kwin_restarts(kwin.clone()));
        kwin
    }

    pub fn available(&self) -> bool {
        self.shared.available.load(Ordering::Relaxed)
    }

    pub fn outputs(&self) -> Vec<OutputInfo> {
        lock(&self.shared.outputs).clone()
    }

    /// Focused window as last reported by the watcher (instant).
    pub fn active_window(&self) -> Option<WindowInfo> {
        if self.available() { lock(&self.shared.active).clone() } else { None }
    }

    pub async fn windows(&self) -> Result<Vec<WindowInfo>, String> {
        self.op("windows", json!({})).await
    }

    pub async fn cursor_output(&self) -> Result<Option<String>, String> {
        self.op("cursorOutput", json!({})).await
    }

    /// Focus a window whose app ids (by = "app") or title (by = "title") match `pattern` (activeApp semantics).
    pub async fn focus(&self, pattern: &str, by_title: bool) -> Result<bool, String> {
        self.op("focus", json!({ "match": pattern, "by": if by_title { "title" } else { "app" } })).await
    }

    /// Focus a window whose desktop file / resource class / resource name equals one of `ids` (case-insensitive).
    pub async fn focus_app(&self, ids: &[String]) -> Result<bool, String> {
        self.op("focusApp", json!({ "ids": ids })).await
    }

    /// Close every window whose app ids match (as `focus_app`); returns their process ids.
    pub async fn close_app(&self, ids: &[String]) -> Result<Vec<i64>, String> {
        self.op("closeApp", json!({ "ids": ids })).await
    }

    pub async fn window_action(&self, action: &str) -> Result<(), String> {
        self.op::<Value>("windowAction", json!({ "action": action })).await.map(drop)
    }

    pub async fn move_to_output(&self, target: &str) -> Result<(), String> {
        self.op::<Value>("moveToOutput", json!({ "target": target })).await.map(drop)
    }

    pub async fn desktop_switch(&self, target: &Value) -> Result<(), String> {
        self.op::<Value>("desktopSwitch", json!({ "target": target })).await.map(drop)
    }

    pub async fn desktop_move_window(&self, target: &Value, follow: bool) -> Result<(), String> {
        self.op::<Value>("desktopMoveWindow", json!({ "target": target, "follow": follow })).await.map(drop)
    }

    pub async fn desktop_create(&self) -> Result<(), String> {
        self.op::<Value>("desktopCreate", json!({})).await.map(drop)
    }

    pub async fn desktop_remove(&self) -> Result<(), String> {
        self.op::<Value>("desktopRemove", json!({})).await.map(drop)
    }

    pub async fn desktop_current(&self) -> Result<Value, String> {
        self.op("desktopCurrent", json!({})).await
    }

    /// Unload the watcher (shutdown).
    pub async fn stop(&self) {
        if self.available() {
            self.unload(&self.watcher_name).await;
        }
        let _ = std::fs::remove_file(self.script_path(&self.watcher_name));
    }

    // ---- internals ----------------------------------------------------------------------------------------

    async fn refresh(&self) {
        let owner = match DBusProxy::new(&self.conn).await {
            Ok(dbus) => dbus.get_name_owner(WellKnownName::from_static_str_unchecked(KWIN).into()).await.ok().map(|o| o.to_string()),
            Err(_) => None,
        };
        let running = owner.is_some();
        *lock(&self.shared.kwin_owner) = owner;
        let available = running && self.load_watcher().await;
        self.shared.available.store(available, Ordering::Relaxed);
        if !available {
            *lock(&self.shared.active) = None;
            lock(&self.shared.outputs).clear();
        }
        log::info!("KWin integration {}", if available { "active" } else { "unavailable" });
        self.shared.notify_status();
    }

    async fn load_watcher(&self) -> bool {
        // A previous daemon on the same socket may have left its watcher behind.
        self.unload(&self.watcher_name).await;
        let source = self.script_source("watch", "", &Value::Null, WATCHER_JS);
        match self.load_and_run(&self.watcher_name, &source).await {
            Ok(()) => true,
            Err(e) => {
                log::warn!("cannot load the KWin watcher script: {e}");
                false
            }
        }
    }

    /// Run `SP_OPS[op](args)` in a one-shot script and decode its answer.
    async fn op<T: DeserializeOwned>(&self, op: &str, args: Value) -> Result<T, String> {
        if !self.available() {
            return Err(UNSUPPORTED.into());
        }
        let seq = self.seq.fetch_add(1, Ordering::Relaxed);
        let name = format!("streampad-{}-{seq}", std::process::id());
        let token = format!("{:016x}{seq:x}", fastrand::u64(..));
        let (tx, rx) = oneshot::channel();
        lock(&self.shared.pending).insert(token.clone(), tx);
        let source = self.script_source(op, &token, &args, OPS_JS);
        let answer = async {
            self.load_and_run(&name, &source).await?;
            match tokio::time::timeout(SCRIPT_TIMEOUT, rx).await {
                Ok(Ok(json)) => Ok(json),
                _ => Err(format!("KWin did not answer \"{op}\" in time")),
            }
        }
        .await;
        lock(&self.shared.pending).remove(&token);
        self.unload(&name).await;
        let _ = std::fs::remove_file(self.script_path(&name));
        decode_answer(&answer?)
    }

    fn script_source(&self, op: &str, token: &str, args: &Value, body: &str) -> String {
        let service = self.conn.unique_name().map(|n| n.to_string()).unwrap_or_default();
        compose_script(&service, op, token, args, body)
    }

    fn script_path(&self, name: &str) -> PathBuf {
        self.scripts_dir.join(format!("{name}.js"))
    }

    async fn load_and_run(&self, name: &str, source: &str) -> Result<(), String> {
        let path = self.script_path(name);
        std::fs::create_dir_all(&self.scripts_dir).map_err(|e| format!("cannot create {}: {e}", self.scripts_dir.display()))?;
        std::fs::write(&path, source).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let path = path.to_string_lossy().into_owned();
        let reply = self
            .conn
            .call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "loadScript", &(path.as_str(), name))
            .await
            .map_err(|e| format!("KWin loadScript failed: {e}"))?;
        let id: i32 = reply.body().deserialize().map_err(|e| format!("KWin loadScript: {e}"))?;
        if id < 0 {
            return Err(format!("KWin refused to load script {name}"));
        }
        let object = format!("/Scripting/Script{id}");
        self.conn
            .call_method(Some(KWIN), object.as_str(), Some("org.kde.kwin.Script"), "run", &())
            .await
            .map_err(|e| format!("KWin script run failed: {e}"))?;
        Ok(())
    }

    async fn unload(&self, name: &str) {
        let result = self.conn.call_method(Some(KWIN), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(name,)).await;
        if let Err(e) = result {
            log::debug!("unloadScript {name}: {e}");
        }
    }
}

/// A complete KWin script: the `SP` header (where to answer, what to do), the shared prelude, then `body`.
fn compose_script(service: &str, op: &str, token: &str, args: &Value, body: &str) -> String {
    let header = json!({ "service": service, "path": OBJECT_PATH, "iface": INTERFACE, "token": token, "op": op, "args": args });
    // JSON is valid JS except, in older engines, for raw U+2028/U+2029 inside strings.
    let header = header.to_string().replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029");
    format!("var SP = {header};\n{PRELUDE_JS}\n{body}")
}

fn decode_answer<T: DeserializeOwned>(json: &str) -> Result<T, String> {
    let answer: Value = serde_json::from_str(json).map_err(|e| format!("invalid answer from KWin: {e}"))?;
    if let Some(error) = answer.get("error") {
        return Err(error.as_str().unwrap_or("KWin script failed").to_string());
    }
    serde_json::from_value(answer.get("ok").cloned().unwrap_or(Value::Null)).map_err(|e| format!("unexpected answer from KWin: {e}"))
}

/// Re-detect KWin (and reload the watcher) whenever `org.kde.KWin` changes owner.
async fn follow_kwin_restarts(kwin: Arc<Kwin>) {
    let Ok(dbus) = DBusProxy::new(&kwin.conn).await else { return };
    let Ok(mut changes) = dbus.receive_name_owner_changed_with_args(&[(0, KWIN)]).await else { return };
    while changes.next().await.is_some() {
        kwin.refresh().await;
    }
}

/// Stable 64-bit FNV-1a, used to derive a per-socket watcher name.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

#[cfg(test)]
mod script_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answers_decode() {
        assert_eq!(decode_answer::<Option<String>>(r#"{"ok":"DP-2"}"#).unwrap().as_deref(), Some("DP-2"));
        assert_eq!(decode_answer::<Option<String>>(r#"{"ok":null}"#).unwrap(), None);
        assert_eq!(decode_answer::<bool>(r#"{"error":"no active window"}"#).unwrap_err(), "no active window");
        assert!(decode_answer::<bool>("garbage").is_err());
    }

    #[test]
    fn watcher_names_are_stable_per_socket() {
        assert_eq!(fnv1a(b"/run/user/1000/streampad/daemon.sock"), fnv1a(b"/run/user/1000/streampad/daemon.sock"));
        assert_ne!(fnv1a(b"/tmp/a.sock"), fnv1a(b"/tmp/b.sock"));
    }
}
