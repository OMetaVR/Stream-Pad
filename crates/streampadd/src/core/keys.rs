//! Captured pad keys: globals with tap/hold (docs/SPEC.md §2.2), macro triggers with repeat/concurrency/release
//! (§2.4) and learn mode (§2.5).

use super::render::active_global;
use super::runs::{Admission, BindingRef};
use super::{Core, Held};
use std::sync::Arc;
use std::time::{Duration, Instant};
use streampad_core::compile::CompiledMacro;
use streampad_core::ipc::{KeyEvent, KeyEventKind, RunEvent, RunStatus, events};
use streampad_core::{GlobalAction, KeyId};
use streampad_input::KeyAction;
use streampad_script::Invocation;

impl Held {
    fn kind(&self) -> KeyEventKind {
        match self {
            Held::GlobalPending { .. } | Held::GlobalDone => KeyEventKind::Global,
            Held::Macro { .. } => KeyEventKind::Macro,
            Held::Empty => KeyEventKind::Empty,
            Held::Learn => KeyEventKind::Learn,
        }
    }
}

impl Core {
    /// A captured pad key event (from the engine or `simulateKey`).
    pub fn pad_key(&mut self, key: KeyId, action: KeyAction, now: Instant) {
        match action {
            KeyAction::Press if self.held.contains_key(&key) => self.key_repeat(key),
            KeyAction::Press => self.key_press(key, now),
            KeyAction::Repeat => self.key_repeat(key),
            KeyAction::Release => self.key_release(key, now),
        }
        self.settle(now);
    }

    fn key_press(&mut self, key: KeyId, now: Instant) {
        if !self.macro_mode {
            return;
        }
        if self.learn.is_some() {
            self.emit_key(key, true, KeyEventKind::Learn);
            self.emit(events::LEARN, serde_json::json!({ "key": key }));
            self.held.insert(key, Held::Learn);
            return;
        }
        if let Some(global) = active_global(&self.cfg, key) {
            self.emit_key(key, true, KeyEventKind::Global);
            let held = match global.hold.filter(|h| *h != GlobalAction::None) {
                Some(hold) => {
                    let deadline = now + Duration::from_millis(self.cfg.settings.hold_ms.into());
                    Held::GlobalPending { tap: global.tap, hold, deadline }
                }
                None => {
                    self.run_global(global.tap, now);
                    Held::GlobalDone
                }
            };
            self.held.insert(key, held);
            return;
        }
        let origin = BindingRef { profile_id: self.current_profile().id.clone(), key };
        if self.binding(&origin).is_some_and(|b| b.r#macro.is_some()) {
            self.emit_key(key, true, KeyEventKind::Macro);
            let runs = self.trigger(&origin, false).into_iter().collect();
            self.held.insert(key, Held::Macro { origin, runs });
        } else {
            self.emit_key(key, true, KeyEventKind::Empty);
            self.held.insert(key, Held::Empty);
        }
    }

    /// Kernel auto-repeat: re-triggers `repeat: true` bindings; ignored for everything else.
    fn key_repeat(&mut self, key: KeyId) {
        if !self.macro_mode {
            return;
        }
        let Some(Held::Macro { origin, .. }) = self.held.get(&key) else { return };
        let origin = origin.clone();
        if !self.binding(&origin).is_some_and(|b| b.repeat) {
            return;
        }
        if let Some(run_id) = self.trigger(&origin, true)
            && let Some(Held::Macro { runs, .. }) = self.held.get_mut(&key)
        {
            runs.push(run_id);
        }
    }

    fn key_release(&mut self, key: KeyId, now: Instant) {
        let Some(held) = self.held.remove(&key) else { return };
        self.emit_key(key, false, held.kind());
        match held {
            Held::GlobalPending { tap, .. } if self.macro_mode => self.run_global(tap, now),
            Held::Macro { runs, .. } => runs.into_iter().for_each(|id| self.runs.release(id)),
            _ => {}
        }
    }

    pub(super) fn fire_due_holds(&mut self, now: Instant) {
        let due: Vec<(KeyId, GlobalAction)> = self
            .held
            .iter()
            .filter_map(|(k, h)| match h {
                Held::GlobalPending { hold, deadline, .. } if *deadline <= now => Some((*k, *hold)),
                _ => None,
            })
            .collect();
        for (key, hold) in due {
            self.held.insert(key, Held::GlobalDone);
            self.run_global(hold, now);
        }
    }

    /// Leaving macro mode drops taps/holds that have not fired yet.
    pub(super) fn cancel_pending_globals(&mut self) {
        for held in self.held.values_mut() {
            if matches!(held, Held::GlobalPending { .. }) {
                *held = Held::GlobalDone;
            }
        }
    }

    pub(super) fn run_global(&mut self, action: GlobalAction, now: Instant) {
        use super::overlay::OverlayAction;
        let peek_until = now + self.peek_duration();
        match action {
            GlobalAction::ToggleOverlay => self.overlay.apply(OverlayAction::Toggle, peek_until),
            GlobalAction::ShowOverlay => self.overlay.apply(OverlayAction::Show, peek_until),
            GlobalAction::HideOverlay => self.overlay.apply(OverlayAction::Hide, peek_until),
            GlobalAction::NextProfile => self.step_profile(1, now),
            GlobalAction::PrevProfile => self.step_profile(-1, now),
            GlobalAction::FirstProfile => self.select_profile(0, -1, now),
            GlobalAction::ToggleMacroMode => self.set_macro_mode(!self.macro_mode, now),
            GlobalAction::OpenEditor => self.effects.push(super::Effect::OpenEditor),
            GlobalAction::None => {}
        }
    }

    /// Start the binding's macro subject to its concurrency mode. Returns the new run id.
    fn trigger(&mut self, origin: &BindingRef, repeat: bool) -> Option<u64> {
        let binding = self.binding(origin)?;
        let title = super::run_title(&binding.label, origin.key);
        match self.runs.admit(origin, binding.concurrency) {
            Admission::Start => {}
            Admission::Busy => return None,
            Admission::TooMany => {
                log::warn!("too many macros running; ignoring {}", origin.key);
                return None;
            }
            Admission::Replace(ids) => ids.into_iter().for_each(|id| {
                self.runs.cancel(id);
            }),
        }
        match self.compiled_for(origin)? {
            Ok(code) => {
                let profile = self.profile_by_id(&origin.profile_id)?;
                let (profile_id, profile_name) = (profile.id.clone(), profile.name.clone());
                let inv = Invocation { run_id: 0, key: Some(origin.key), repeat, profile_id, profile_name };
                Some(self.start_run(code, inv, Some(origin.clone()), title))
            }
            Err(e) => {
                let message = match &e.node_id {
                    Some(node) => format!("{e} (node {node})"),
                    None => e.to_string(),
                };
                self.report_failed_start(Some(origin.key), &title, message);
                None
            }
        }
    }

    /// Spawn a run (`inv.run_id` is assigned here) and announce it.
    pub(super) fn start_run(
        &mut self,
        code: Arc<CompiledMacro>,
        mut inv: Invocation,
        origin: Option<BindingRef>,
        title: String,
    ) -> u64 {
        let run_id = self.runs.allocate_id();
        inv.run_id = run_id;
        let key = inv.key;
        let on_done = self.deps.on_run_done.clone();
        let control = self.deps.runner.spawn(code, inv, Box::new(move |outcome| on_done(run_id, outcome)));
        self.runs.insert(run_id, key, origin, title, control);
        self.emit(events::RUN, RunEvent { run_id, key, status: RunStatus::Started, error: None });
        run_id
    }

    /// A macro that could not even start (compile error): reported like a failed run.
    fn report_failed_start(&mut self, key: Option<KeyId>, title: &str, message: String) {
        let run_id = self.runs.allocate_id();
        if self.cfg.settings.toast_on_error {
            self.toast(format!("{title}: {}", super::summarize_error(&message)), streampad_core::ipc::ToastKind::Error, 4000);
        }
        self.emit(events::RUN, RunEvent { run_id, key, status: RunStatus::Failed, error: Some(message) });
    }

    fn emit_key(&mut self, key: KeyId, pressed: bool, kind: KeyEventKind) {
        self.emit(events::KEY, KeyEvent { key, pressed, kind });
    }

    fn profile_by_id(&self, id: &str) -> Option<&streampad_core::Profile> {
        self.cfg.profiles.iter().find(|p| p.id == id)
    }
}
