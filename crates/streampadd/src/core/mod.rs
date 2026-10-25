//! The daemon's state machine: macro mode, profile, overlay visibility, learn mode, globals, runs and vars.
//!
//! It is synchronous and deterministic: every entry point takes `now`, side effects on the outside world are
//! queued as [`Effect`]s (or go through the injected [`InputBackend`] / [`MacroRunner`]), and timers are
//! exposed through [`Core::next_deadline`] + [`Core::tick`]. The event loop in `daemon.rs` drives it.

mod host_calls;
mod keys;
pub mod overlay;
pub mod render;
mod requests;
pub mod runs;
#[cfg(test)]
mod tests;

use crate::backend::{InputBackend, InputOverrides, MacroRunner, engine_config};
use crate::persist::PersistedState;
use overlay::OverlayModel;
use runs::{BindingRef, RunRegistry};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use streampad_core::compile::{CompileError, CompileOptions, CompiledMacro, compile};
use streampad_core::ipc::{
    DaemonState, DeviceInfo, Integrations, OutputInfo, OverlayState, ProfileSummary, RenderProfile, ToastEvent, ToastKind,
    events,
};
use streampad_core::paths::Paths;
use streampad_core::{Binding, Config, KeyId, Profile, UnboundKeys, VERSION};
use streampad_script::RunOutcome;

/// Identifies an IPC connection.
pub type ClientId = u64;

/// Learn mode switches itself off after this long.
pub const LEARN_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a show waits for KWin to say which output the cursor is on before letting the compositor choose.
pub const OUTPUT_WAIT: Duration = Duration::from_millis(250);

/// Something for the event loop to carry out.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Broadcast to subscribed clients now.
    Event(Event),
    /// Ask KWin for the output under the cursor and answer with [`Core::output_resolved`].
    ResolveCursorOutput { seq: u64 },
    OpenEditor,
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: &'static str,
    pub data: Value,
}

impl Event {
    pub fn new(name: &'static str, data: impl Serialize) -> Event {
        Event { name, data: serde_json::to_value(data).unwrap_or(Value::Null) }
    }
}

/// Completion of a run, delivered back to the event loop from a worker thread.
pub type RunDoneFn = Arc<dyn Fn(u64, RunOutcome) + Send + Sync>;

pub struct Deps {
    pub paths: Arc<Paths>,
    pub input: Arc<dyn InputBackend>,
    pub runner: Arc<dyn MacroRunner>,
    pub on_run_done: RunDoneFn,
    pub overrides: InputOverrides,
}

/// Runtime state at startup: persisted state if `rememberState`, else the configured defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct Initial {
    pub macro_mode: bool,
    pub profile_index: usize,
    pub pinned: bool,
    pub vars: BTreeMap<String, Value>,
}

impl Initial {
    pub fn new(cfg: &Config, persisted: Option<&PersistedState>) -> Initial {
        let remembered = persisted.filter(|_| cfg.settings.remember_state);
        Initial {
            macro_mode: remembered.map_or(cfg.settings.start_in_macro_mode, |p| p.macro_mode),
            profile_index: remembered.and_then(|p| cfg.profile_index(&p.profile_id)).unwrap_or(0),
            pinned: remembered.is_some_and(|p| p.pinned),
            vars: persisted.map(|p| p.vars.clone()).unwrap_or_default(),
        }
    }
}

struct Learn {
    until: Instant,
    owner: Option<ClientId>,
}

/// A pad key that is currently held down, and what its press did.
#[derive(Debug, Clone, PartialEq)]
enum Held {
    /// Global with a hold action: tap on release before `deadline`, hold action at `deadline`.
    GlobalPending { tap: streampad_core::GlobalAction, hold: streampad_core::GlobalAction, deadline: Instant },
    /// Global whose action has fired (tap-on-press, or the hold).
    GlobalDone,
    Macro { origin: BindingRef, runs: Vec<u64> },
    Empty,
    Learn,
}

pub struct Core {
    cfg: Config,
    config_error: Option<String>,
    deps: Deps,
    macro_mode: bool,
    profile_index: usize,
    overlay: OverlayModel,
    /// Overlay is (being) shown; published as visible once its output is known.
    shown: bool,
    preview_on: bool,
    output: Option<String>,
    output_wait: Option<(u64, Instant)>,
    output_seq: u64,
    learn: Option<Learn>,
    held: HashMap<KeyId, Held>,
    runs: RunRegistry,
    compiled: HashMap<BindingRef, Result<Arc<CompiledMacro>, CompileError>>,
    vars: BTreeMap<String, Value>,
    devices: Vec<DeviceInfo>,
    reported_outputs: Option<Vec<OutputInfo>>,
    kwin_outputs: Vec<OutputInfo>,
    integrations: Integrations,
    capture: HashSet<KeyId>,
    render_cache: Option<(RenderProfile, Vec<ProfileSummary>)>,
    toast_seq: u64,
    effects: Vec<Effect>,
    pre_state: Vec<Event>,
    state_dirty: bool,
}

impl Core {
    pub fn new(cfg: Config, config_error: Option<String>, initial: Initial, deps: Deps, now: Instant) -> Core {
        deps.runner.set_library(cfg.library.clone());
        let devices = deps.input.devices();
        let profile_index = initial.profile_index.min(cfg.profiles.len().saturating_sub(1));
        let mut core = Core {
            cfg,
            config_error,
            deps,
            macro_mode: initial.macro_mode,
            profile_index,
            overlay: OverlayModel::new(initial.pinned),
            shown: false,
            preview_on: false,
            output: None,
            output_wait: None,
            output_seq: 0,
            learn: None,
            held: HashMap::new(),
            runs: RunRegistry::default(),
            compiled: HashMap::new(),
            vars: initial.vars,
            devices,
            reported_outputs: None,
            kwin_outputs: Vec::new(),
            integrations: Integrations::default(),
            capture: HashSet::new(),
            render_cache: None,
            toast_seq: 0,
            effects: Vec::new(),
            pre_state: Vec::new(),
            state_dirty: true,
        };
        core.capture = core.wanted_capture();
        core.deps.input.set_capture(core.capture.clone());
        core.settle(now);
        core
    }

    // ---- event-loop interface -----------------------------------------------------------------------------

    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// Events that must reach clients right before the next `state` (e.g. `profile`).
    pub fn take_pre_state_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pre_state)
    }

    /// Whether something may have changed since the last call (the event loop dedups identical snapshots).
    pub fn take_state_dirty(&mut self) -> bool {
        std::mem::take(&mut self.state_dirty)
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        let holds = self.held.values().filter_map(|h| match h {
            Held::GlobalPending { deadline, .. } => Some(*deadline),
            _ => None,
        });
        holds
            .chain(self.overlay.next_deadline())
            .chain(self.learn.as_ref().map(|l| l.until))
            .chain(self.output_wait.map(|(_, t)| t))
            .min()
    }

    /// Fire every timer that is due.
    pub fn tick(&mut self, now: Instant) {
        self.fire_due_holds(now);
        if self.learn.as_ref().is_some_and(|l| l.until <= now) {
            self.learn = None;
        }
        if self.output_wait.is_some_and(|(_, t)| t <= now) {
            self.output_wait = None;
            self.output = None;
        }
        self.settle(now);
    }

    pub fn persisted(&self) -> PersistedState {
        PersistedState {
            macro_mode: self.macro_mode,
            profile_id: self.current_profile().id.clone(),
            pinned: self.overlay.pinned,
            vars: self.vars.clone(),
        }
    }

    pub fn snapshot(&mut self) -> DaemonState {
        let (profile, profiles) = self.rendered().clone();
        let overlay_settings = self.cfg.settings.overlay.clone();
        let accent = self.current_profile().color.clone().unwrap_or_else(|| overlay_settings.accent.clone());
        DaemonState {
            version: VERSION.into(),
            macro_mode: self.macro_mode,
            profile_index: self.profile_index,
            profiles,
            profile,
            overlay: OverlayState {
                visible: self.shown && self.output_wait.is_none(),
                pinned: self.overlay.pinned,
                preview: self.preview_on,
                output: self.output.clone(),
                settings: overlay_settings,
                accent,
            },
            learn: self.learn.is_some(),
            devices: self.devices.clone(),
            integrations: self.integrations.clone(),
            config_error: self.config_error.clone(),
        }
    }

    /// Cancel everything that is running (shutdown).
    pub fn shutdown(&mut self) {
        self.runs.cancel_all();
    }

    // ---- inputs from the outside world --------------------------------------------------------------------

    pub fn apply_config(&mut self, cfg: Config, now: Instant) {
        let current_id = self.current_profile().id.clone();
        let old = std::mem::replace(&mut self.cfg, cfg);
        self.config_error = None;
        self.profile_index =
            self.cfg.profile_index(&current_id).unwrap_or_else(|| self.profile_index.min(self.cfg.profiles.len() - 1));
        self.compiled.clear();
        self.render_cache = None;
        if old.library != self.cfg.library {
            self.deps.runner.set_library(self.cfg.library.clone());
        }
        let (s_old, s_new) = (&old.settings, &self.cfg.settings);
        if s_old.toggle_key != s_new.toggle_key || s_old.devices != s_new.devices {
            self.deps.input.reconfigure(engine_config(s_new, &self.deps.overrides, self.macro_mode));
        }
        if s_old.overlay.monitor != s_new.overlay.monitor && self.shown {
            self.resolve_output(now);
        }
        self.settle(now);
    }

    pub fn config_failed(&mut self, message: String, now: Instant) {
        log::warn!("{message}; keeping the previous config");
        self.emit(events::CONFIG_ERROR, serde_json::json!({ "message": message }));
        self.config_error = Some(message);
        self.settle(now);
    }

    /// Re-read config.json (watcher or `reload`). Errors are reported and also returned.
    pub fn reload_from_disk(&mut self, now: Instant) -> Result<(), String> {
        match crate::config_store::load(&self.deps.paths) {
            Ok(Some(cfg)) => {
                log::info!("reloaded config.json");
                self.apply_config(cfg, now);
                Ok(())
            }
            Ok(None) => {
                log::info!("config.json was removed; keeping the current config");
                Ok(())
            }
            Err(message) => {
                self.config_failed(message.clone(), now);
                Err(message)
            }
        }
    }

    pub fn engine_event(&mut self, ev: streampad_input::EngineEvent, now: Instant) {
        use streampad_input::EngineEvent;
        match ev {
            EngineEvent::ModeChanged { macro_mode, .. } if macro_mode != self.macro_mode => self.mode_changed(macro_mode, now),
            EngineEvent::ModeChanged { .. } => {}
            EngineEvent::PadKey { key, action } => self.pad_key(key, action, now),
            EngineEvent::DevicesChanged(devices) => self.devices = devices,
            EngineEvent::Warning(message) => {
                log::warn!("input: {message}");
                self.emit(events::LOG, streampad_core::ipc::LogEvent { run_id: None, level: streampad_core::ipc::LogLevel::Warn, message });
            }
            EngineEvent::Stopped { reason } => log::warn!("input engine stopped: {reason}"),
        }
        self.settle(now);
    }

    pub fn run_done(&mut self, run_id: u64, outcome: RunOutcome, now: Instant) {
        use streampad_core::ipc::{RunEvent, RunStatus};
        let Some(run) = self.runs.finish(run_id) else { return };
        let (status, error) = match outcome {
            RunOutcome::Finished => (RunStatus::Finished, None),
            RunOutcome::Cancelled => (RunStatus::Cancelled, None),
            RunOutcome::Failed(e) => (RunStatus::Failed, Some(e)),
        };
        if let Some(e) = &error {
            log::info!("macro \"{}\" failed: {e}", run.title);
            if self.cfg.settings.toast_on_error {
                self.toast(format!("{}: {}", run.title, summarize_error(e)), ToastKind::Error, 4000);
            }
        }
        self.emit(events::RUN, RunEvent { run_id, key: run.key, status, error });
        self.settle(now);
    }

    pub fn client_gone(&mut self, client: ClientId, now: Instant) {
        if self.learn.as_ref().is_some_and(|l| l.owner == Some(client)) {
            self.learn = None;
            self.settle(now);
        }
    }

    pub fn set_kwin(&mut self, available: bool, outputs: Vec<OutputInfo>, now: Instant) {
        self.integrations.kwin = available;
        self.kwin_outputs = outputs;
        self.settle(now);
    }

    pub fn output_resolved(&mut self, seq: u64, output: Option<String>, now: Instant) {
        if self.output_wait.is_some_and(|(s, _)| s == seq) {
            self.output_wait = None;
            self.output = output;
            self.settle(now);
        }
    }

    // ---- shared internals ---------------------------------------------------------------------------------

    fn current_profile(&self) -> &Profile {
        &self.cfg.profiles[self.profile_index]
    }

    fn rendered(&mut self) -> &(RenderProfile, Vec<ProfileSummary>) {
        let (cfg, paths, index) = (&self.cfg, &self.deps.paths, self.profile_index);
        self.render_cache
            .get_or_insert_with(|| (render::render_profile(cfg, index, paths), render::profile_summaries(cfg, paths)))
    }

    fn binding(&self, origin: &BindingRef) -> Option<&Binding> {
        let index = self.cfg.profile_index(&origin.profile_id)?;
        self.cfg.profiles[index].keys.get(&origin.key)
    }

    fn compiled_for(&mut self, origin: &BindingRef) -> Option<Result<Arc<CompiledMacro>, CompileError>> {
        if let Some(c) = self.compiled.get(origin) {
            return Some(c.clone());
        }
        let m = self.binding(origin)?.r#macro.as_ref()?;
        let result = compile(m, &CompileOptions { trace: true }).map(Arc::new);
        self.compiled.insert(origin.clone(), result.clone());
        Some(result)
    }

    fn emit(&mut self, name: &'static str, data: impl Serialize) {
        self.effects.push(Effect::Event(Event::new(name, data)));
    }

    fn toast(&mut self, text: String, kind: ToastKind, duration_ms: u32) {
        let icon = match kind {
            ToastKind::Error => Some("circle-alert"),
            ToastKind::Success => Some("circle-check"),
            ToastKind::Info => None,
        };
        let icon = icon.and_then(|name| self.deps.paths.lucide_icon(name)).map(|p| p.display().to_string());
        self.toast_seq += 1;
        let toast = ToastEvent { id: self.toast_seq, text, kind, duration_ms, icon };
        self.emit(events::TOAST, toast);
    }

    fn set_macro_mode(&mut self, on: bool, now: Instant) {
        if on != self.macro_mode {
            self.deps.input.set_macro_mode(on);
            self.mode_changed(on, now);
        }
    }

    fn mode_changed(&mut self, on: bool, now: Instant) {
        self.macro_mode = on;
        if on {
            if self.cfg.settings.overlay.peek_on_mode_change {
                self.overlay.peek(now + self.peek_duration());
            }
        } else {
            self.overlay.stop_peek();
            self.cancel_pending_globals();
        }
    }

    fn peek_duration(&self) -> Duration {
        Duration::from_millis(self.cfg.settings.overlay.peek_ms.into())
    }

    fn select_profile(&mut self, index: usize, direction: i8, now: Instant) {
        if index == self.profile_index || index >= self.cfg.profiles.len() {
            return;
        }
        self.profile_index = index;
        self.render_cache = None;
        self.pre_state.push(Event::new(events::PROFILE, streampad_core::ipc::ProfileEvent { index, direction }));
        if self.macro_mode && self.cfg.settings.overlay.peek_on_profile_change {
            self.overlay.peek(now + self.peek_duration());
        }
    }

    fn step_profile(&mut self, delta: i64, now: Instant) {
        let count = self.cfg.profiles.len() as i64;
        let index = (self.profile_index as i64 + delta).rem_euclid(count) as usize;
        self.select_profile(index, if delta < 0 { -1 } else { 1 }, now);
    }

    fn set_learn(&mut self, on: bool, owner: Option<ClientId>, now: Instant) {
        self.learn = on.then(|| Learn { until: now + LEARN_TIMEOUT, owner });
    }

    /// Recompute everything derived from the primary state. Every mutating entry point ends here.
    fn settle(&mut self, now: Instant) {
        self.sync_overlay(now);
        let capture = self.wanted_capture();
        if capture != self.capture {
            self.deps.input.set_capture(capture.clone());
            self.capture = capture;
        }
        self.state_dirty = true;
    }

    fn sync_overlay(&mut self, now: Instant) {
        self.overlay.expire(now);
        self.preview_on = self.overlay.preview_active(now);
        let wanted = self.overlay.wanted(self.macro_mode, now);
        if wanted && !self.shown {
            self.shown = true;
            self.resolve_output(now);
        } else if !wanted && self.shown {
            self.shown = false;
            self.output_wait = None;
        }
    }

    /// Pick the output for a show: the configured monitor if connected, else the cursor's (asked from KWin),
    /// else let the compositor choose.
    fn resolve_output(&mut self, now: Instant) {
        self.output_wait = None;
        if let Some(monitor) = self.cfg.settings.overlay.monitor.clone()
            && self.output_connected(&monitor)
        {
            self.output = Some(monitor);
        } else if self.integrations.kwin {
            self.output_seq += 1;
            self.output_wait = Some((self.output_seq, now + OUTPUT_WAIT));
            self.effects.push(Effect::ResolveCursorOutput { seq: self.output_seq });
        } else {
            self.output = None;
        }
    }

    fn known_outputs(&self) -> &[OutputInfo] {
        match &self.reported_outputs {
            Some(reported) if !reported.is_empty() => reported,
            _ => &self.kwin_outputs,
        }
    }

    /// Unknown outputs (nobody reported any yet) count as connected.
    fn output_connected(&self, name: &str) -> bool {
        let known = self.known_outputs();
        known.is_empty() || known.iter().any(|o| o.name == name)
    }

    fn wanted_capture(&self) -> HashSet<KeyId> {
        if self.learn.is_some() || self.cfg.settings.unbound_keys == UnboundKeys::Block {
            return KeyId::ALL.into_iter().collect();
        }
        let profile = self.current_profile();
        KeyId::ALL
            .into_iter()
            .filter(|k| render::active_global(&self.cfg, *k).is_some() || profile.keys.get(k).is_some_and(|b| b.r#macro.is_some()))
            .collect()
    }
}

/// How a run is named in error toasts: the binding's label, else the key ("Numpad 4").
fn run_title(label: &str, key: KeyId) -> String {
    let label = label.trim();
    if label.is_empty() { key.display_name() } else { label }.to_string()
}

/// First line of an error, shortened for a toast.
fn summarize_error(e: &str) -> String {
    const MAX: usize = 140;
    let line = e.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.chars().count() <= MAX {
        line.to_string()
    } else {
        format!("{}…", line.chars().take(MAX - 1).collect::<String>())
    }
}
