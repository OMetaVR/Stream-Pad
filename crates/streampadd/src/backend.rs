//! Seams between the daemon core and the input engine / macro runtime. Each trait has the real implementation
//! here; `--no-input` uses [`NoInput`] and the core's unit tests use in-process fakes.

use crossbeam_channel::Sender;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use streampad_core::compile::CompiledMacro;
use streampad_core::ipc::DeviceInfo;
use streampad_core::{DeviceMode, KeyId, Settings, combo};
use streampad_input::{DeviceFilter, Engine, EngineConfig, EngineEvent, KeyOutput};
use streampad_script::{Host, Invocation, RunHandle, RunOutcome, ScriptEngine};

/// What the core needs from the input engine.
pub trait InputBackend: Send + Sync {
    fn set_macro_mode(&self, on: bool);
    fn set_capture(&self, keys: HashSet<KeyId>);
    fn reconfigure(&self, cfg: EngineConfig);
    fn devices(&self) -> Vec<DeviceInfo>;
    /// Release every grab. Idempotent.
    fn shutdown(&self);
}

/// Options that come from the command line rather than config.json.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputOverrides {
    pub only_device: Option<String>,
    pub grab_timeout: Option<Duration>,
}

/// Engine configuration for the given settings. An unknown toggle key falls back to Scroll Lock
/// (config validation reports it before it gets here).
pub fn engine_config(settings: &Settings, overrides: &InputOverrides, macro_mode: bool) -> EngineConfig {
    let devices = &settings.devices;
    EngineConfig {
        toggle_key: combo::key_code(&settings.toggle_key).unwrap_or(streampad_core::keys::KEY_SCROLLLOCK),
        pass_toggle_key: false,
        devices: DeviceFilter {
            auto: devices.mode == DeviceMode::Auto,
            include: devices.include.clone(),
            exclude: devices.exclude.clone(),
            only: overrides.only_device.clone(),
        },
        grab_timeout: overrides.grab_timeout,
        macro_mode,
    }
}

/// The real evdev/uinput engine.
pub struct EngineInput {
    engine: Mutex<Option<Engine>>,
}

impl EngineInput {
    pub fn start(cfg: EngineConfig, events: Sender<EngineEvent>) -> anyhow::Result<(EngineInput, KeyOutput)> {
        let engine = Engine::start(cfg, events)?;
        let output = engine.output();
        Ok((EngineInput { engine: Mutex::new(Some(engine)) }, output))
    }

    fn with<R>(&self, f: impl FnOnce(&Engine) -> R) -> Option<R> {
        self.engine.lock().unwrap_or_else(PoisonError::into_inner).as_ref().map(f)
    }
}

impl InputBackend for EngineInput {
    fn set_macro_mode(&self, on: bool) {
        self.with(|e| e.set_macro_mode(on));
    }
    fn set_capture(&self, keys: HashSet<KeyId>) {
        self.with(|e| e.set_capture(keys));
    }
    fn reconfigure(&self, cfg: EngineConfig) {
        self.with(|e| e.reconfigure(cfg));
    }
    fn devices(&self) -> Vec<DeviceInfo> {
        self.with(Engine::devices).unwrap_or_default()
    }
    fn shutdown(&self) {
        if let Some(engine) = self.engine.lock().unwrap_or_else(PoisonError::into_inner).take() {
            engine.shutdown();
        }
    }
}

/// `--no-input`: no device is touched; macro mode lives only in the daemon and pad keys arrive via
/// `simulateKey`.
pub struct NoInput;

impl InputBackend for NoInput {
    fn set_macro_mode(&self, _on: bool) {}
    fn set_capture(&self, _keys: HashSet<KeyId>) {}
    fn reconfigure(&self, _cfg: EngineConfig) {}
    fn devices(&self) -> Vec<DeviceInfo> {
        Vec::new()
    }
    fn shutdown(&self) {}
}

/// Completion callback of a run; called exactly once, from any thread.
pub type OnDone = Box<dyn FnOnce(RunOutcome) + Send + 'static>;

/// What the core needs from the macro runtime.
pub trait MacroRunner: Send + Sync {
    fn set_library(&self, source: String);
    fn spawn(&self, code: Arc<CompiledMacro>, inv: Invocation, on_done: OnDone) -> Box<dyn RunControl>;
    fn check_syntax(&self, source: &str) -> Result<(), String>;
}

/// Control over one running macro.
pub trait RunControl: Send {
    fn cancel(&self);
    /// Deliver the key release (runs `onRelease` if the macro defines one).
    fn release(&self);
}

/// Live runs and whether each has been asked to stop, so blocking host calls (`run(cmd, {wait: true})`) can
/// give up early.
#[derive(Default)]
pub struct Cancellations {
    runs: Mutex<HashMap<u64, bool>>,
}

impl Cancellations {
    pub fn is_cancelled(&self, run_id: u64) -> bool {
        self.lock().get(&run_id).copied().unwrap_or(false)
    }

    fn begin(&self, run_id: u64) {
        self.lock().insert(run_id, false);
    }

    fn cancel(&self, run_id: u64) {
        if let Some(cancelled) = self.lock().get_mut(&run_id) {
            *cancelled = true;
        }
    }

    fn end(&self, run_id: u64) {
        self.lock().remove(&run_id);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, bool>> {
        self.runs.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The QuickJS runtime.
pub struct ScriptRunner {
    engine: ScriptEngine,
    cancellations: Arc<Cancellations>,
}

impl ScriptRunner {
    pub fn new(host: Arc<dyn Host>, cancellations: Arc<Cancellations>) -> Self {
        ScriptRunner { engine: ScriptEngine::new(host), cancellations }
    }
}

impl MacroRunner for ScriptRunner {
    fn set_library(&self, source: String) {
        self.engine.set_library(source);
    }

    fn spawn(&self, code: Arc<CompiledMacro>, inv: Invocation, on_done: OnDone) -> Box<dyn RunControl> {
        let run_id = inv.run_id;
        self.cancellations.begin(run_id);
        let cancellations = self.cancellations.clone();
        let on_done: OnDone = Box::new(move |outcome| {
            cancellations.end(run_id);
            on_done(outcome);
        });
        let handle = self.engine.spawn(code, inv, on_done);
        Box::new(ScriptRun { handle, run_id, cancellations: self.cancellations.clone() })
    }

    fn check_syntax(&self, source: &str) -> Result<(), String> {
        self.engine.check_syntax(source)
    }
}

struct ScriptRun {
    handle: RunHandle,
    run_id: u64,
    cancellations: Arc<Cancellations>,
}

impl RunControl for ScriptRun {
    fn cancel(&self) {
        self.cancellations.cancel(self.run_id);
        self.handle.cancel();
    }

    fn release(&self) {
        self.handle.release();
    }
}
