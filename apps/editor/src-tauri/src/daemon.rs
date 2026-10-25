//! Persistent client for the daemon socket (docs/SPEC.md §4): newline-delimited JSON, request/response
//! matched by id, events forwarded to the webview. Reconnects with backoff forever, so starting or
//! restarting the daemon at any time is picked up automatically.
//!
//! Webview events: `streampad:daemon-status` (DaemonStatus, on connection changes only — the webview
//! folds `state` events into its copy) and `streampad:daemon-event` (`{ event, data }` as received).

use crate::error::{CmdError, CmdResult};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use streampad_core::ipc::{ErrorBody, Incoming, Request, Response, events};
use streampad_core::paths::Paths;
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::OwnedReadHalf;
use tokio::sync::{Notify, mpsc, oneshot};

pub const STATUS_EVENT: &str = "streampad:daemon-status";
pub const DAEMON_EVENT: &str = "streampad:daemon-event";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const HELLO_TIMEOUT: Duration = Duration::from_secs(3);
const MIN_BACKOFF: Duration = Duration::from_millis(250);
const MAX_BACKOFF: Duration = Duration::from_secs(3);
/// After `start_daemon`, connection failures keep the status at "connecting" for this long.
const START_GRACE: Duration = Duration::from_secs(8);
const NOT_RUNNING: &str = "The StreamPad daemon is not running";

/// Mirrors `DaemonStatus` in `src/lib/backend.ts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DaemonStatus {
    Connected {
        state: Value,
    },
    Connecting,
    Stopped {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

type Reply = Result<Value, ErrorBody>;

struct Inner {
    status: Mutex<DaemonStatus>,
    /// Sender to the writer task of the live connection, if any.
    link: Mutex<Option<mpsc::UnboundedSender<String>>>,
    pending: Mutex<HashMap<u64, oneshot::Sender<Reply>>>,
    next_id: AtomicU64,
    wake: Notify,
    grace_until: Mutex<Option<Instant>>,
}

pub struct DaemonClient {
    inner: Arc<Inner>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl DaemonClient {
    /// Spawn the connection loop on Tauri's async runtime.
    pub fn start(app: AppHandle) -> DaemonClient {
        let inner = Arc::new(Inner {
            status: Mutex::new(DaemonStatus::Connecting),
            link: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            wake: Notify::new(),
            grace_until: Mutex::new(None),
        });
        tauri::async_runtime::spawn(connection_loop(app, inner.clone()));
        DaemonClient { inner }
    }

    pub fn status(&self) -> DaemonStatus {
        lock(&self.inner.status).clone()
    }

    /// The daemon is being started: show "connecting" and retry quickly for a while.
    pub fn expect_daemon(&self, app: &AppHandle) {
        *lock(&self.inner.grace_until) = Some(Instant::now() + START_GRACE);
        if !matches!(self.status(), DaemonStatus::Connected { .. }) {
            self.inner.set_status(app, DaemonStatus::Connecting);
        }
        self.inner.wake.notify_one();
    }

    pub async fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        self.inner.request(method, params, timeout).await
    }
}

impl Inner {
    fn set_status(&self, app: &AppHandle, next: DaemonStatus) {
        {
            let mut cur = lock(&self.status);
            if *cur == next {
                return;
            }
            *cur = next.clone();
        }
        if let Err(e) = app.emit(STATUS_EVENT, &next) {
            log::warn!("cannot emit daemon status: {e}");
        }
    }

    fn update_state(&self, state: Value) {
        if let DaemonStatus::Connected { state: cur } = &mut *lock(&self.status) {
            *cur = state;
        }
    }

    fn in_grace(&self) -> bool {
        lock(&self.grace_until).is_some_and(|t| Instant::now() < t)
    }

    async fn request(&self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = Request { id, method: method.to_string(), params };
        let mut line = serde_json::to_string(&req).map_err(|e| format!("cannot encode {method} request: {e}"))?;
        line.push('\n');
        let (tx, rx) = oneshot::channel();
        {
            let link = lock(&self.link);
            let Some(link) = link.as_ref() else { return Err(NOT_RUNNING.into()) };
            lock(&self.pending).insert(id, tx);
            if link.send(line).is_err() {
                lock(&self.pending).remove(&id);
                return Err(NOT_RUNNING.into());
            }
        }
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(body))) => Err(body.message),
            Ok(Err(_)) => Err("The daemon disconnected before answering".into()),
            Err(_) => {
                lock(&self.pending).remove(&id);
                Err(format!("The daemon did not answer \"{method}\" within {} s", timeout.as_secs()))
            }
        }
    }

    fn resolve(&self, r: Response) {
        let Some(tx) = lock(&self.pending).remove(&r.id) else { return };
        let reply = match r.error {
            Some(e) => Err(e),
            None => Ok(r.result.unwrap_or(Value::Null)),
        };
        let _ = tx.send(reply);
    }

    fn drop_connection(&self) {
        *lock(&self.link) = None;
        // Dropping the senders wakes every waiter with "disconnected".
        lock(&self.pending).clear();
    }
}

async fn connection_loop(app: AppHandle, inner: Arc<Inner>) {
    let mut backoff = MIN_BACKOFF;
    loop {
        let socket = Paths::resolve().socket;
        match UnixStream::connect(&socket).await {
            Ok(stream) => {
                backoff = MIN_BACKOFF;
                let reason = run_session(&app, &inner, stream).await;
                inner.drop_connection();
                log::info!("daemon connection ended: {reason}");
                inner.set_status(&app, DaemonStatus::Stopped { reason: Some(reason) });
            }
            Err(e) if inner.in_grace() => {
                log::debug!("daemon not up yet at {}: {e}", socket.display());
                backoff = MIN_BACKOFF;
            }
            Err(e) => {
                let reason = match e.kind() {
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => NOT_RUNNING.to_string(),
                    _ => format!("Cannot connect to {}: {e}", socket.display()),
                };
                inner.set_status(&app, DaemonStatus::Stopped { reason: Some(reason) });
            }
        }
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = inner.wake.notified() => backoff = MIN_BACKOFF,
        }
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

/// Handshake, then pump lines until the connection ends. Returns why it ended.
async fn run_session(app: &AppHandle, inner: &Arc<Inner>, stream: UnixStream) -> String {
    let (read, mut write) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    *lock(&inner.link) = Some(tx);
    let writer = tauri::async_runtime::spawn(async move {
        while let Some(line) = rx.recv().await {
            if write.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
    });

    let reader = read_loop(app.clone(), inner.clone(), BufReader::new(read));
    tokio::pin!(reader);
    let hello = inner.request("hello", json!({ "client": "editor", "subscribe": true, "trace": true }), HELLO_TIMEOUT);
    let reason = tokio::select! {
        reason = &mut reader => reason,
        result = hello => match result {
            Ok(v) => {
                *lock(&inner.grace_until) = None;
                inner.set_status(app, DaemonStatus::Connected { state: v.get("state").cloned().unwrap_or(Value::Null) });
                reader.await
            }
            Err(e) => format!("Handshake with the daemon failed: {e}"),
        },
    };
    writer.abort();
    reason
}

async fn read_loop(app: AppHandle, inner: Arc<Inner>, mut reader: BufReader<OwnedReadHalf>) -> String {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => return "The daemon closed the connection".into(),
            Ok(_) => {}
            Err(e) => return format!("Connection to the daemon lost: {e}"),
        }
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        match serde_json::from_str::<Incoming>(text) {
            Ok(Incoming::Response(r)) => inner.resolve(r),
            Ok(Incoming::Event(ev)) => {
                if ev.event == events::STATE {
                    inner.update_state(ev.data.clone());
                }
                if let Err(e) = app.emit(DAEMON_EVENT, &ev) {
                    log::warn!("cannot forward daemon event: {e}");
                }
            }
            Err(e) => log::warn!("ignoring malformed daemon message: {e}"),
        }
    }
}

#[tauri::command]
pub fn daemon_status(client: State<'_, DaemonClient>) -> DaemonStatus {
    client.status()
}

#[tauri::command]
pub async fn daemon_request(
    client: State<'_, DaemonClient>,
    method: String,
    params: Option<Value>,
    timeout_ms: Option<u64>,
) -> CmdResult<Value> {
    let timeout = timeout_ms.map(Duration::from_millis).unwrap_or(REQUEST_TIMEOUT);
    client.request(&method, params.unwrap_or(Value::Null), timeout).await.map_err(CmdError::new)
}
