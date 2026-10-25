//! The native functions the prelude builds on: `__host(method, argsJson)`, `sleep(ms)` and `__node(id)`.

use crate::signals::RunSignals;
use crate::{Host, RunContext};
use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, Function, Value};
use serde_json::json;
use std::sync::Arc;

/// Everything a native call needs to reach the daemon on behalf of one run.
pub(crate) struct Bridge {
    pub(crate) host: Arc<dyn Host>,
    pub(crate) run: RunContext,
    pub(crate) signals: Arc<RunSignals>,
}

impl Bridge {
    /// `__host`: JSON in, JSON out; a host error becomes an `Error` with the host's message.
    fn call(&self, ctx: &Ctx<'_>, method: &str, args_json: Option<&str>) -> rquickjs::Result<String> {
        self.ensure_running(ctx)?;
        let args = match args_json.map(str::trim) {
            None | Some("") => json!({}),
            Some(text) => serde_json::from_str(text).map_err(|e| {
                Exception::throw_type(ctx, &format!("__host({method}): arguments are not valid JSON: {e}"))
            })?,
        };
        let result = self.host.call(self.run.clone(), method, args);
        // The host may have blocked (e.g. `run(…, { wait: true })`) while the run was cancelled.
        self.ensure_running(ctx)?;
        match result {
            Ok(value) => Ok(value.to_string()),
            Err(message) => Err(Exception::throw_message(ctx, &message)),
        }
    }

    /// Once cancelled, no further side effects: every native call throws. The interrupt handler then
    /// stops the script even if it catches these.
    fn ensure_running(&self, ctx: &Ctx<'_>) -> rquickjs::Result<()> {
        if self.signals.is_cancelled() { Err(Exception::throw_internal(ctx, "macro cancelled")) } else { Ok(()) }
    }
}

pub(crate) fn install<'js>(ctx: &Ctx<'js>, bridge: Arc<Bridge>) -> rquickjs::Result<()> {
    let globals = ctx.globals();

    let b = bridge.clone();
    let host = Function::new(ctx.clone(), move |ctx: Ctx<'js>, method: String, args: Opt<String>| {
        b.call(&ctx, &method, args.0.as_deref())
    })?
    .with_name("__host")?;
    globals.set("__host", host)?;

    let b = bridge.clone();
    let sleep = Function::new(ctx.clone(), move |ctx: Ctx<'js>, ms: Opt<Value<'js>>| -> rquickjs::Result<()> {
        let ms = ms.0.as_ref().and_then(Value::as_number).filter(|ms| !ms.is_nan());
        let Some(ms) = ms else {
            return Err(Exception::throw_type(&ctx, "sleep: expected a number of milliseconds, e.g. sleep(100)"));
        };
        b.ensure_running(&ctx)?;
        if b.signals.sleep(ms) { Ok(()) } else { b.ensure_running(&ctx) }
    })?
    .with_name("sleep")?;
    globals.set("sleep", sleep)?;

    let b = bridge;
    let node = Function::new(ctx.clone(), move |ctx: Ctx<'js>, id: String| -> rquickjs::Result<()> {
        b.ensure_running(&ctx)?;
        // Tracing is best effort: a failed trace must never break the macro.
        if let Err(e) = b.host.call(b.run.clone(), "trace", json!({ "nodeId": id })) {
            log::debug!("run {}: trace {id} failed: {e}", b.run.run_id);
        }
        Ok(())
    })?
    .with_name("__node")?;
    globals.set("__node", node)?;
    Ok(())
}
