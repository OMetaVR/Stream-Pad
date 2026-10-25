//! `streampadd run`: wires the components together and drives the core from one event loop.

use crate::backend::{Cancellations, EngineInput, InputBackend, InputOverrides, MacroRunner, NoInput, ScriptRunner, engine_config};
use crate::cli::RunArgs;
use crate::command::{Command, CommandRx, CommandTx};
use crate::core::{Core, Deps, Effect, Initial};
use crate::host::DaemonHost;
use crate::host::desktop_entry;
use crate::host::launch::Launcher;
use crate::instance::{Instance, InstanceError};
use crate::ipc::hub::Hub;
use crate::ipc::server::{self, ServerCtx};
use crate::kwin::Kwin;
use crate::persist::PersistedState;
use crate::{config_store, overlay_proc};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};
use streampad_core::ipc::{DaemonState, events};
use streampad_core::paths::Paths;
use streampad_core::{KeyId, VERSION, defaults};
use streampad_input::{EngineEvent, KeyOutput};
use tokio::sync::{mpsc, oneshot, watch};

/// `state` events are coalesced to at most one per frame.
const STATE_INTERVAL: Duration = Duration::from_millis(16);
const CONFIG_DEBOUNCE: Duration = Duration::from_millis(150);
const PERSIST_DEBOUNCE: Duration = Duration::from_millis(500);
const BUS_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound for any single D-Bus call (KWin, notifications, global shortcuts).
const DBUS_CALL_TIMEOUT: Duration = Duration::from_secs(3);
/// Time for already-queued IPC messages (e.g. the `quit` response) to reach clients at shutdown.
const SHUTDOWN_FLUSH: Duration = Duration::from_millis(100);

pub fn run(args: RunArgs) -> anyhow::Result<ExitCode> {
    let paths = Arc::new(Paths::resolve());
    let mut instance = match Instance::acquire(&paths.socket) {
        Ok(instance) => instance,
        Err(e @ InstanceError::AlreadyRunning(_)) => {
            eprintln!("streampadd: {e}");
            return Ok(ExitCode::FAILURE);
        }
        Err(e) => return Err(e.into()),
    };
    log::info!("streampadd {VERSION} starting (socket {})", paths.socket.display());

    let (cfg, config_error) = config_store::load_or_create(&paths, installed_default_apps);
    let persisted = PersistedState::load(&paths.state_file());
    let initial = Initial::new(&cfg, persisted.as_ref());
    let overrides = InputOverrides { only_device: args.only_device.clone(), grab_timeout: args.grab_timeout };
    let (tx, rx) = mpsc::unbounded_channel();
    let (input, keys) = if args.no_input {
        log::info!("--no-input: not touching any input device");
        (Arc::new(NoInput) as Arc<dyn InputBackend>, None)
    } else {
        start_engine(engine_config(&cfg.settings, &overrides, initial.macro_mode), tx.clone())
    };

    let listener = instance.take_listener().ok_or_else(|| anyhow::anyhow!("socket listener already taken"))?;
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("streampadd").enable_all().build()?;
    let code = runtime.block_on(async move {
        let listener = tokio::net::UnixListener::from_std({
            listener.set_nonblocking(true)?;
            listener
        })?;
        let parts = Startup { args, paths, cfg, config_error, initial, overrides, input, keys, tx, rx, listener };
        anyhow::Ok(serve(parts).await)
    })?;
    runtime.shutdown_timeout(Duration::from_secs(1));
    drop(instance);
    Ok(code)
}

/// Apps-profile defaults: for every slot, the first candidate desktop id that is installed.
fn installed_default_apps() -> BTreeMap<KeyId, String> {
    defaults::APP_SLOTS
        .iter()
        .filter_map(|slot| Some((slot.key, slot.candidates.iter().find_map(|id| desktop_entry::find_exact(id))?.id)))
        .collect()
}

fn start_engine(cfg: streampad_input::EngineConfig, tx: CommandTx) -> (Arc<dyn InputBackend>, Option<KeyOutput>) {
    let (events_tx, events_rx) = crossbeam_channel::unbounded();
    let (engine, keys) = match EngineInput::start(cfg, events_tx) {
        Ok(started) => started,
        Err(e) => {
            log::error!("cannot start the input engine: {e:#}. Running without input; pad keys won't work.");
            return (Arc::new(NoInput), None);
        }
    };
    let forward = std::thread::Builder::new().name("engine-events".into()).spawn(move || {
        for ev in events_rx {
            if tx.send(Command::Engine(ev)).is_err() {
                break;
            }
        }
    });
    match forward {
        Ok(_) => (Arc::new(engine), Some(keys)),
        Err(e) => {
            log::error!("cannot start the input event thread: {e}. Releasing all devices.");
            engine.shutdown();
            (Arc::new(NoInput), None)
        }
    }
}

struct Startup {
    args: RunArgs,
    paths: Arc<Paths>,
    cfg: streampad_core::Config,
    config_error: Option<String>,
    initial: Initial,
    overrides: InputOverrides,
    input: Arc<dyn InputBackend>,
    keys: Option<KeyOutput>,
    tx: CommandTx,
    rx: CommandRx,
    listener: tokio::net::UnixListener,
}

enum Exit {
    Requested,
    GrabTimeout,
    EngineStopped(String),
}

async fn serve(s: Startup) -> ExitCode {
    let Startup { args, paths, cfg, config_error, initial, overrides, input, keys, tx, mut rx, listener } = s;
    let rt = tokio::runtime::Handle::current();
    let hub = Arc::new(Hub::default());
    let connect = async { zbus::connection::Builder::session()?.method_timeout(DBUS_CALL_TIMEOUT).build().await };
    let bus = match tokio::time::timeout(BUS_TIMEOUT, connect).await {
        Ok(Ok(conn)) => Some(conn),
        Ok(Err(e)) => {
            log::warn!("no D-Bus session bus: {e}");
            None
        }
        Err(_) => {
            log::warn!("the D-Bus session bus did not answer; running without desktop integration");
            None
        }
    };
    let kwin = match &bus {
        Some(conn) => Some(Kwin::start(conn.clone(), &paths, tx.clone()).await),
        None => None,
    };
    let launcher = Arc::new(Launcher::new(kwin.clone(), rt.clone()));
    let cancellations = Arc::new(Cancellations::default());
    let host = DaemonHost {
        core: tx.clone(),
        rt,
        hub: hub.clone(),
        kwin: kwin.clone(),
        bus,
        keys: keys.clone(),
        paths: paths.clone(),
        launcher: launcher.clone(),
        cancellations: cancellations.clone(),
    };
    let runner: Arc<dyn MacroRunner> = Arc::new(ScriptRunner::new(Arc::new(host), cancellations));
    let done_tx = tx.clone();
    let deps = Deps {
        paths: paths.clone(),
        input: input.clone(),
        runner: runner.clone(),
        on_run_done: Arc::new(move |run_id, outcome| {
            let _ = done_tx.send(Command::RunDone { run_id, outcome });
        }),
        overrides,
    };
    let now = Instant::now();
    let mut core = Core::new(cfg, config_error, initial, deps, now);
    if let Some(kwin) = &kwin {
        core.set_kwin(kwin.available(), kwin.outputs(), now);
    }

    let ctx = Arc::new(ServerCtx { hub: hub.clone(), core: tx.clone(), kwin: kwin.clone(), runner, launcher: launcher.clone() });
    let server = tokio::spawn(server::serve(listener, ctx));
    let watch_tx = tx.clone();
    let _watcher = config_store::watch(&paths, move || {
        let _ = watch_tx.send(Command::ConfigChanged);
    })
    .inspect_err(|e| log::warn!("config hot-reload disabled: {e}"))
    .ok();
    let (stop_overlay, overlay_stop_rx) = watch::channel(false);
    let overlay = (!args.no_overlay).then(|| tokio::spawn(overlay_proc::supervise(paths.socket.clone(), overlay_stop_rx)));
    tokio::spawn(forward_signals(tx.clone()));
    log::info!("ready");

    let mut event_loop = EventLoop::new(core, hub.clone(), kwin.clone(), launcher, tx, paths.state_file());
    let grab_deadline = args.grab_timeout.map(|t| Instant::now() + t);
    let exit = event_loop.run(&mut rx, grab_deadline).await;

    log::info!("shutting down");
    event_loop.core.shutdown();
    if let Some(keys) = &keys {
        keys.release_all();
    }
    event_loop.persist_now();
    server.abort();
    // In-flight answers (e.g. to `quit`) are still on their way to the client queues.
    tokio::time::sleep(SHUTDOWN_FLUSH).await;
    hub.close_all();
    if let Some(kwin) = &kwin {
        kwin.stop().await;
    }
    let _ = stop_overlay.send(true);
    if let Some(overlay) = overlay {
        let _ = overlay.await;
    }
    input.shutdown();
    match exit {
        Exit::Requested | Exit::GrabTimeout => ExitCode::SUCCESS,
        Exit::EngineStopped(reason) => {
            log::error!("input engine stopped: {reason}");
            ExitCode::FAILURE
        }
    }
}

async fn forward_signals(tx: CommandTx) {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut term), Ok(mut int), Ok(mut hup)) =
        (signal(SignalKind::terminate()), signal(SignalKind::interrupt()), signal(SignalKind::hangup()))
    else {
        log::warn!("cannot install signal handlers");
        return;
    };
    loop {
        let command = tokio::select! {
            _ = term.recv() => Command::Shutdown,
            _ = int.recv() => Command::Shutdown,
            _ = hup.recv() => Command::ConfigChanged,
        };
        if tx.send(command).is_err() {
            return;
        }
    }
}

struct EventLoop {
    core: Core,
    hub: Arc<Hub>,
    kwin: Option<Arc<Kwin>>,
    launcher: Arc<Launcher>,
    tx: CommandTx,
    state_file: PathBuf,
    last_state: Option<DaemonState>,
    last_flush: Instant,
    flush_at: Option<Instant>,
    persisted: crate::persist::PersistedState,
    persist_at: Option<Instant>,
    reload_at: Option<Instant>,
}

enum Wake {
    Command(Command),
    Timer,
    Closed,
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(d) => tokio::time::sleep_until(d.into()).await,
        None => std::future::pending().await,
    }
}

impl EventLoop {
    fn new(core: Core, hub: Arc<Hub>, kwin: Option<Arc<Kwin>>, launcher: Arc<Launcher>, tx: CommandTx, state_file: PathBuf) -> Self {
        let persisted = core.persisted();
        EventLoop {
            core,
            hub,
            kwin,
            launcher,
            tx,
            state_file,
            last_state: None,
            last_flush: Instant::now(),
            flush_at: None,
            persisted,
            persist_at: None,
            reload_at: None,
        }
    }

    async fn run(&mut self, rx: &mut CommandRx, grab_deadline: Option<Instant>) -> Exit {
        loop {
            let deadline = [self.core.next_deadline(), self.flush_at, self.persist_at, self.reload_at, grab_deadline].into_iter().flatten().min();
            let wake = tokio::select! {
                biased;
                cmd = rx.recv() => cmd.map_or(Wake::Closed, Wake::Command),
                () = sleep_until(deadline) => Wake::Timer,
            };
            let now = Instant::now();
            match wake {
                Wake::Command(cmd) => {
                    if let Some(exit) = self.handle(cmd, now) {
                        return exit;
                    }
                }
                Wake::Timer => {}
                Wake::Closed => return Exit::Requested,
            }
            if grab_deadline.is_some_and(|t| t <= now) {
                log::info!("grab timeout reached");
                return Exit::GrabTimeout;
            }
            if self.reload_at.is_some_and(|t| t <= now) {
                self.reload_at = None;
                // Failures are reported to clients by the core.
                let _ = self.core.reload_from_disk(now);
            }
            self.core.tick(now);
            if self.run_effects() {
                return Exit::Requested;
            }
            self.schedule(now);
        }
    }

    fn handle(&mut self, cmd: Command, now: Instant) -> Option<Exit> {
        match cmd {
            Command::Engine(EngineEvent::Stopped { reason }) => return Some(Exit::EngineStopped(reason)),
            Command::Engine(ev) => self.core.engine_event(ev, now),
            Command::Request { client, method, params, reply } => {
                let _ = reply.send(self.core.request(client, &method, params, now));
            }
            Command::Host { method, args, reply } => {
                let _ = reply.send(self.core.host_call(&method, args, now));
            }
            Command::RunDone { run_id, outcome } => self.core.run_done(run_id, outcome, now),
            Command::ClientGone(client) => self.core.client_gone(client, now),
            Command::ConfigChanged => self.reload_at = Some(now + CONFIG_DEBOUNCE),
            Command::Kwin { available, outputs } => self.core.set_kwin(available, outputs, now),
            Command::OutputResolved { seq, output } => self.core.output_resolved(seq, output, now),
            Command::Shutdown => return Some(Exit::Requested),
        }
        None
    }

    /// Carry out the core's effects. Returns true when the daemon should quit.
    fn run_effects(&mut self) -> bool {
        let mut quit = false;
        for effect in self.core.take_effects() {
            match effect {
                Effect::Event(e) => self.hub.broadcast(e.name, e.data, false),
                Effect::ResolveCursorOutput { seq } => {
                    let (kwin, tx) = (self.kwin.clone(), self.tx.clone());
                    tokio::spawn(async move {
                        let output = match kwin {
                            Some(kwin) => kwin.cursor_output().await.unwrap_or_else(|e| {
                                log::debug!("cursor output: {e}");
                                None
                            }),
                            None => None,
                        };
                        let _ = tx.send(Command::OutputResolved { seq, output });
                    });
                }
                Effect::OpenEditor => {
                    let (launcher, tx) = (self.launcher.clone(), self.tx.clone());
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = launcher.open_editor() {
                            log::warn!("open editor: {e}");
                            let (reply, _) = oneshot::channel();
                            let args = json!({ "text": e, "kind": "error", "durationMs": 4000 });
                            let _ = tx.send(Command::Host { method: "toast".into(), args, reply });
                        }
                    });
                }
                Effect::Quit => quit = true,
            }
        }
        quit
    }

    /// Coalesce `state` broadcasts and debounce state.json writes.
    fn schedule(&mut self, now: Instant) {
        if self.core.take_state_dirty() && self.flush_at.is_none() {
            self.flush_at = Some((self.last_flush + STATE_INTERVAL).max(now));
        }
        if self.flush_at.is_some_and(|t| t <= now) {
            self.flush_at = None;
            self.flush_state(now);
        }
        let persisted = self.core.persisted();
        if persisted != self.persisted {
            self.persisted = persisted;
            self.persist_at.get_or_insert(now + PERSIST_DEBOUNCE);
        }
        if self.persist_at.is_some_and(|t| t <= now) {
            self.persist_now();
        }
    }

    fn flush_state(&mut self, now: Instant) {
        self.last_flush = now;
        let before = self.core.take_pre_state_events();
        if !self.hub.has_subscribers() {
            self.last_state = None;
            return;
        }
        let state = self.core.snapshot();
        for e in before {
            self.hub.broadcast(e.name, e.data, false);
        }
        if self.last_state.as_ref() != Some(&state) {
            self.hub.broadcast(events::STATE, json!(state), false);
            self.last_state = Some(state);
        }
    }

    fn persist_now(&mut self) {
        self.persist_at = None;
        self.persisted = self.core.persisted();
        if let Err(e) = self.persisted.save(&self.state_file) {
            log::warn!("cannot write {}: {e}", self.state_file.display());
        }
    }
}
