//! Registry of running macros: concurrency decisions, release delivery and cancellation.

use crate::backend::RunControl;
use std::collections::BTreeMap;
use streampad_core::{Concurrency, KeyId};

/// Upper bound on simultaneously running macros, so a held `repeat` + `parallel` key can't pile up workers.
pub const MAX_RUNS: usize = 32;

/// The binding a run was started from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BindingRef {
    pub profile_id: String,
    pub key: KeyId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProfileHook {
    Enter,
    Leave,
}

impl ProfileHook {
    pub fn label(self) -> &'static str {
        match self {
            ProfileHook::Enter => "on enter",
            ProfileHook::Leave => "on leave",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RunOrigin {
    Binding(BindingRef),
    ProfileHook { profile_id: String, hook: ProfileHook },
}

pub struct RunEntry {
    pub key: Option<KeyId>,
    pub origin: Option<RunOrigin>,
    /// Human name for error toasts ("Game Mode", "Numpad 4", "Test run").
    pub title: String,
    control: Box<dyn RunControl>,
    cancelling: bool,
}

/// What to do with a new trigger of a binding.
#[derive(Debug, PartialEq, Eq)]
pub enum Admission {
    Start,
    /// `ignore` concurrency and the binding is still running.
    Busy,
    /// `restart` concurrency: cancel these, then start.
    Replace(Vec<u64>),
    TooMany,
}

#[derive(Default)]
pub struct RunRegistry {
    runs: BTreeMap<u64, RunEntry>,
    last_id: u64,
}

impl RunRegistry {
    pub fn allocate_id(&mut self) -> u64 {
        self.last_id += 1;
        self.last_id
    }

    pub fn admit(&self, origin: &BindingRef, concurrency: Concurrency) -> Admission {
        let active: Vec<u64> = self.active_for(origin).collect();
        match concurrency {
            Concurrency::Ignore if !active.is_empty() => Admission::Busy,
            Concurrency::Restart if !active.is_empty() => Admission::Replace(active),
            _ if self.runs.len() >= MAX_RUNS => Admission::TooMany,
            _ => Admission::Start,
        }
    }

    pub fn has_capacity(&self) -> bool {
        self.runs.len() < MAX_RUNS
    }

    pub fn insert(
        &mut self,
        id: u64,
        key: Option<KeyId>,
        origin: Option<RunOrigin>,
        title: String,
        control: Box<dyn RunControl>,
    ) {
        self.runs.insert(id, RunEntry { key, origin, title, control, cancelling: false });
    }

    /// Runs of `origin` that are not already being cancelled.
    fn active_for<'a>(&'a self, origin: &'a BindingRef) -> impl Iterator<Item = u64> + 'a {
        self.runs
            .iter()
            .filter(move |(_, r)| !r.cancelling && matches!(&r.origin, Some(RunOrigin::Binding(b)) if b == origin))
            .map(|(id, _)| *id)
    }

    /// Active profile lifecycle hook runs for `profile_id` (both enter and leave).
    pub fn active_profile_hooks(&self, profile_id: &str) -> Vec<u64> {
        self.runs
            .iter()
            .filter(|(_, r)| !r.cancelling && matches!(&r.origin, Some(RunOrigin::ProfileHook { profile_id: id, .. }) if id == profile_id))
            .map(|(id, _)| *id)
            .collect()
    }

    pub fn cancel(&mut self, id: u64) -> bool {
        match self.runs.get_mut(&id) {
            Some(run) => {
                run.cancelling = true;
                run.control.cancel();
                true
            }
            None => false,
        }
    }

    pub fn cancel_all(&mut self) {
        let ids: Vec<u64> = self.runs.keys().copied().collect();
        for id in ids {
            self.cancel(id);
        }
    }

    pub fn release(&self, id: u64) {
        if let Some(run) = self.runs.get(&id).filter(|r| !r.cancelling) {
            run.control.release();
        }
    }

    pub fn finish(&mut self, id: u64) -> Option<RunEntry> {
        self.runs.remove(&id)
    }
}
