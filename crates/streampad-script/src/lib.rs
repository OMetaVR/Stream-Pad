//! StreamPad macro runtime: runs compiled graphs and hand-written scripts in QuickJS with the API from
//! docs/SPEC.md §6 (reference: API.md). All side effects go through the `Host` trait, implemented by the
//! daemon.
//!
//! Each run gets its own worker thread with a fresh QuickJS runtime (64 MB heap limit, bounded stack).
//! Evaluation order: prelude (the JS API, `src/prelude.js`), the shared library, then the macro, whose
//! top level is the press handler. Macro code runs as a function body, so a top-level `return` ends the
//! press handler. If it defines `onRelease`, the run stays alive until [`RunHandle::release`] (or
//! cancel), then calls `onRelease(event)` with `event.type === "release"`.

mod errors;
mod natives;
mod promises;
mod signals;
mod source;
mod worker;

use serde_json::Value;
use signals::RunSignals;
use std::any::Any;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, PoisonError, RwLock};
use std::thread;
use std::time::Duration;
use streampad_core::KeyId;
use streampad_core::compile::CompiledMacro;

/// Implemented by the daemon. `method` / `args` / result shapes are listed in docs/SPEC.md §6
/// ("Host boundary"). Called from worker threads; must be thread-safe. Errors become JS exceptions.
pub trait Host: Send + Sync + 'static {
    fn call(&self, run: RunContext, method: &str, args: Value) -> Result<Value, String>;
}

/// Identifies the run a host call belongs to (for logs, traces, cancellation-aware behaviour).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunContext {
    pub run_id: u64,
    pub key: Option<KeyId>,
}

/// What triggered the run; exposed to JS as the global `event`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub run_id: u64,
    pub key: Option<KeyId>,
    pub repeat: bool,
    pub profile_id: String,
    pub profile_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    Finished,
    Cancelled,
    /// First line: `"<message> (line L:C)"` (`(library line L:C)` when the error is in the shared library
    /// and no macro frame is involved). Further lines, if any: the stack, one `    at …` per frame.
    Failed(String),
}

pub struct ScriptEngine {
    host: Arc<dyn Host>,
    library: RwLock<Arc<str>>,
}

impl ScriptEngine {
    pub fn new(host: Arc<dyn Host>) -> Self {
        ScriptEngine { host, library: RwLock::new(Arc::from("")) }
    }

    /// Shared JS (config `library`) evaluated before every macro. Applies to runs started afterwards.
    pub fn set_library(&self, source: String) {
        *self.library.write().unwrap_or_else(PoisonError::into_inner) = Arc::from(source);
    }

    /// Start a run on a worker thread. `on_done` is called exactly once, from the worker thread, after the
    /// press handler finishes (when there is no `onRelease`) or after `onRelease` finishes. The JS context
    /// is already disposed by then and [`RunHandle::is_finished`] is true. (Should the OS refuse to create
    /// the thread, `on_done(Failed)` is called right away on the calling thread instead.)
    pub fn spawn(
        &self,
        code: Arc<CompiledMacro>,
        inv: Invocation,
        on_done: Box<dyn FnOnce(RunOutcome) + Send + 'static>,
    ) -> RunHandle {
        let signals = Arc::new(RunSignals::default());
        let handle = RunHandle { run_id: inv.run_id, signals: signals.clone() };
        let library = self.library.read().unwrap_or_else(PoisonError::into_inner).clone();
        let job = worker::Job { host: self.host.clone(), library, code, inv, signals: signals.clone() };
        let completion = Completion { signals, on_done: Some(on_done) };
        let spawned = thread::Builder::new()
            .name(format!("macro-{}", handle.run_id))
            .stack_size(worker::WORKER_STACK_SIZE)
            .spawn(move || {
                let run_id = job.inv.run_id;
                let outcome = panic::catch_unwind(AssertUnwindSafe(|| job.run())).unwrap_or_else(|payload| {
                    let message = panic_message(payload.as_ref());
                    log::error!("macro run {run_id} panicked: {message}");
                    RunOutcome::Failed(format!("internal error: {message}"))
                });
                completion.finish(outcome);
            });
        if let Err(e) = spawned {
            log::error!("could not start a thread for macro run {}: {e}", handle.run_id);
        }
        handle
    }

    /// Syntax-check a script without running it (error message includes line:column). Accepts exactly
    /// what a macro may contain, e.g. a top-level `return`. The message has the format of
    /// [`RunOutcome::Failed`].
    pub fn check_syntax(&self, source: &str) -> Result<(), String> {
        worker::check_syntax(source)
    }
}

/// Handle to a running macro. Cheap to clone.
#[derive(Clone)]
pub struct RunHandle {
    run_id: u64,
    signals: Arc<RunSignals>,
}

impl RunHandle {
    pub fn run_id(&self) -> u64 {
        self.run_id
    }

    /// Interrupt the run as soon as possible (interrupts `sleep` immediately; aborts JS via the QuickJS
    /// interrupt handler). `on_done(Cancelled)` follows, unless the run had already finished. A host call in
    /// progress is not interrupted: the run ends as soon as it returns, and no further host calls are made.
    pub fn cancel(&self) {
        self.signals.cancel();
    }

    /// Deliver the key release. If the script defined `onRelease`, it runs now (after the press handler has
    /// finished, if it is still running); otherwise this is a no-op.
    pub fn release(&self) {
        self.signals.release();
    }

    pub fn is_finished(&self) -> bool {
        self.signals.is_finished()
    }

    /// Blocks until the run has finished (`on_done` is being or has been called) or `timeout` elapsed.
    /// Returns whether it finished. Useful for orderly shutdown: cancel every run, then wait.
    pub fn wait(&self, timeout: Duration) -> bool {
        self.signals.wait_finished(timeout)
    }
}

/// Delivers the outcome exactly once. Dropped without `finish` only when the worker thread never started.
struct Completion {
    signals: Arc<RunSignals>,
    on_done: Option<Box<dyn FnOnce(RunOutcome) + Send + 'static>>,
}

impl Completion {
    fn finish(mut self, outcome: RunOutcome) {
        self.complete(outcome);
    }

    fn complete(&mut self, outcome: RunOutcome) {
        if let Some(on_done) = self.on_done.take() {
            self.signals.mark_finished();
            on_done(outcome);
        }
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        self.complete(RunOutcome::Failed("could not start a thread for the macro".into()));
    }
}

fn panic_message(payload: &(dyn Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".into())
}
