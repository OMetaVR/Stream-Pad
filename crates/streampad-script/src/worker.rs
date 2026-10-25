//! One run, start to finish, on the calling (worker) thread: fresh QuickJS runtime + context, prelude,
//! library, press handler, optional wait for the key release and `onRelease`, then disposal. Promise jobs
//! queued by a handler run right after it (src/promises.rs).

use crate::errors::describe;
use crate::natives::{self, Bridge};
use crate::promises::Rejections;
use crate::signals::RunSignals;
use crate::source::{self, LIBRARY_FILE, MACRO_FILE, PRELUDE_FILE};
use crate::{Host, Invocation, RunContext, RunOutcome};
use rquickjs::context::EvalOptions;
use rquickjs::{Context, Ctx, FromJs, Function, Object, Runtime};
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use streampad_core::compile::CompiledMacro;

/// Heap limit of one run's QuickJS runtime.
pub(crate) const MEMORY_LIMIT: usize = 64 << 20;
/// JS stack budget; deeper recursion throws a RangeError.
const MAX_STACK_SIZE: usize = 1 << 20;
/// Native stack of a worker thread: the JS budget plus headroom for host calls.
pub(crate) const WORKER_STACK_SIZE: usize = 4 << 20;
/// `check_syntax` only parses, but code that closes the wrapper early can run; bound it.
const SYNTAX_CHECK_BUDGET: Duration = Duration::from_millis(250);
const SYNTAX_CHECK_MEMORY: usize = 16 << 20;

const PRELUDE: &str = include_str!("prelude.js");

pub(crate) struct Job {
    pub(crate) host: Arc<dyn Host>,
    pub(crate) library: Arc<str>,
    pub(crate) code: Arc<CompiledMacro>,
    pub(crate) inv: Invocation,
    pub(crate) signals: Arc<RunSignals>,
}

impl Job {
    pub(crate) fn run(self) -> RunOutcome {
        let signals = self.signals.clone();
        let runtime = match new_runtime(MEMORY_LIMIT, move || signals.is_cancelled()) {
            Ok(rt) => rt,
            Err(e) => return RunOutcome::Failed(e),
        };
        let rejections = Rejections::track(&runtime, self.code.clone());
        let context = match Context::full(&runtime) {
            Ok(ctx) => ctx,
            Err(e) => return RunOutcome::Failed(format!("could not create a JavaScript context: {e}")),
        };
        context.with(|ctx| match self.press_then_release(&ctx, &rejections) {
            Ok(outcome) => outcome,
            Err(_) if self.signals.is_cancelled() => RunOutcome::Cancelled,
            Err(e) => RunOutcome::Failed(describe(&ctx, e, &self.code.code)),
        })
    }

    fn press_then_release<'js>(&self, ctx: &Ctx<'js>, rejections: &Rejections) -> rquickjs::Result<RunOutcome> {
        let run = RunContext { run_id: self.inv.run_id, key: self.inv.key };
        natives::install(ctx, Arc::new(Bridge { host: self.host.clone(), run, signals: self.signals.clone() }))?;
        let init: Function = eval(ctx, PRELUDE, PRELUDE_FILE)?;
        let internals: Object = init.call((source::PREFIX.len(),))?;
        let call = |name: &str| internals.get::<_, Function>(name);

        call("setEvent")?.call::<_, ()>((self.event_json(),))?;
        if !self.library.trim().is_empty() {
            eval::<()>(ctx, &self.library, LIBRARY_FILE)?;
        }
        let main: Function = eval(ctx, &source::wrap(&self.code.code), MACRO_FILE)?;
        let on_release: Option<Function> = call("runPress")?.call((main,))?;
        if let Some(outcome) = self.settle(ctx, rejections) {
            return Ok(outcome);
        }

        let Some(on_release) = on_release else {
            return Ok(RunOutcome::Finished);
        };
        if !self.signals.wait_release() {
            return Ok(RunOutcome::Cancelled);
        }
        call("runRelease")?.call::<_, ()>((on_release,))?;
        Ok(self.settle(ctx, rejections).unwrap_or(RunOutcome::Finished))
    }

    /// Runs the promise jobs a handler queued; `Some` when that ends the run.
    fn settle(&self, ctx: &Ctx<'_>, rejections: &Rejections) -> Option<RunOutcome> {
        let unhandled = rejections.settle(ctx, || self.signals.is_cancelled());
        if self.signals.is_cancelled() { Some(RunOutcome::Cancelled) } else { unhandled.map(RunOutcome::Failed) }
    }

    /// The JS `event` global for the press handler.
    fn event_json(&self) -> String {
        let inv = &self.inv;
        json!({
            "key": inv.key,
            "type": "press",
            "repeat": inv.repeat,
            "profile": { "id": inv.profile_id, "name": inv.profile_name },
        })
        .to_string()
    }
}

/// Compiles `source` as macro code without running it.
pub(crate) fn check_syntax(source: &str) -> Result<(), String> {
    let deadline = Instant::now() + SYNTAX_CHECK_BUDGET;
    let runtime = new_runtime(SYNTAX_CHECK_MEMORY, move || Instant::now() > deadline)?;
    let context = Context::full(&runtime).map_err(|e| format!("could not create a JavaScript context: {e}"))?;
    context.with(|ctx| {
        eval::<rquickjs::Value>(&ctx, &source::wrap(source), MACRO_FILE)
            .map(drop)
            .map_err(|e| describe(&ctx, e, source))
    })
}

fn new_runtime(memory_limit: usize, interrupt: impl FnMut() -> bool + 'static) -> Result<Runtime, String> {
    let runtime = Runtime::new().map_err(|e| format!("could not start the JavaScript engine: {e}"))?;
    runtime.set_memory_limit(memory_limit);
    runtime.set_max_stack_size(MAX_STACK_SIZE);
    runtime.set_interrupt_handler(Some(Box::new(interrupt)));
    Ok(runtime)
}

fn eval<'js, T: FromJs<'js>>(ctx: &Ctx<'js>, source: &str, file: &str) -> rquickjs::Result<T> {
    let mut opts = EvalOptions::default();
    opts.strict = true;
    opts.filename = Some(file.to_string());
    ctx.eval_with_options(source, opts)
}
