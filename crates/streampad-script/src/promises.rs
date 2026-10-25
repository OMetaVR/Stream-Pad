//! Promises under a synchronous API: queued jobs (`.then` callbacks, `async` continuations) run to
//! completion after each handler, and a rejection nobody handles fails the run like an exception would.

use crate::errors::describe_thrown;
use rquickjs::{Ctx, Runtime, Value};
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use streampad_core::compile::CompiledMacro;

/// Rejected promises that have no handler (yet), by promise identity, with the formatted reason.
pub(crate) struct Rejections(Rc<RefCell<Vec<(u64, String)>>>);

impl Rejections {
    /// Starts tracking the rejections of `runtime`'s promises; `code` maps error positions back to the macro.
    pub(crate) fn track(runtime: &Runtime, code: Arc<CompiledMacro>) -> Rejections {
        let pending: Rc<RefCell<Vec<(u64, String)>>> = Rc::default();
        let tracked = pending.clone();
        runtime.set_host_promise_rejection_tracker(Some(Box::new(move |ctx, promise, reason, handled| {
            let id = identity(&promise);
            // Formatting can run JS (getters on the reason), so finish it before borrowing.
            let reason = (!handled).then(|| describe_thrown(&ctx, reason, &code.code));
            let mut tracked = tracked.borrow_mut();
            match reason {
                Some(reason) => tracked.push((id, reason)),
                None => tracked.retain(|(p, _)| *p != id),
            }
        })));
        Rejections(pending)
    }

    /// Runs queued jobs until there are none left or `cancelled()`; returns the reason of the first
    /// rejection that is still unhandled.
    pub(crate) fn settle(&self, ctx: &Ctx<'_>, cancelled: impl Fn() -> bool) -> Option<String> {
        while !cancelled() && ctx.execute_pending_job() {}
        self.0.borrow().first().map(|(_, reason)| reason.clone())
    }
}

/// Identity of a JS object (its tag and pointer), without keeping a reference to it.
fn identity(v: &Value<'_>) -> u64 {
    let mut hasher = DefaultHasher::new();
    v.hash(&mut hasher);
    hasher.finish()
}
