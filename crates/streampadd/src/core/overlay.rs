//! Overlay visibility model (docs/SPEC.md §2.3): `visible = macroMode && (pinned || peeking) || preview`.

use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayAction {
    Show,
    Hide,
    Toggle,
    Peek,
}

impl OverlayAction {
    pub fn parse(s: &str) -> Option<OverlayAction> {
        match s {
            "show" => Some(OverlayAction::Show),
            "hide" => Some(OverlayAction::Hide),
            "toggle" => Some(OverlayAction::Toggle),
            "peek" => Some(OverlayAction::Peek),
            _ => None,
        }
    }
}

/// How long the editor's preview lasts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Preview {
    UntilHidden,
    Until(Instant),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OverlayModel {
    pub pinned: bool,
    peek_until: Option<Instant>,
    preview: Option<Preview>,
}

impl OverlayModel {
    pub fn new(pinned: bool) -> Self {
        OverlayModel { pinned, ..Default::default() }
    }

    pub fn wanted(&self, macro_mode: bool, now: Instant) -> bool {
        let peeking = self.peek_until.is_some_and(|t| t > now);
        self.preview_active(now) || (macro_mode && (self.pinned || peeking))
    }

    pub fn preview_active(&self, now: Instant) -> bool {
        match self.preview {
            Some(Preview::UntilHidden) => true,
            Some(Preview::Until(t)) => t > now,
            None => false,
        }
    }

    /// Apply an action from a global key, a macro or IPC. `peek_until` is used by `Peek`.
    pub fn apply(&mut self, action: OverlayAction, peek_until: Instant) {
        match action {
            OverlayAction::Show => self.pinned = true,
            OverlayAction::Hide => self.hide(),
            OverlayAction::Toggle if self.pinned => self.hide(),
            OverlayAction::Toggle => self.pinned = true,
            OverlayAction::Peek => self.peek_until = Some(peek_until),
        }
    }

    /// The editor's "Preview overlay": shows regardless of macro mode and leaves `pinned` alone.
    pub fn apply_preview(&mut self, action: OverlayAction, peek_until: Instant, now: Instant) {
        self.preview = match action {
            OverlayAction::Show => Some(Preview::UntilHidden),
            OverlayAction::Hide => None,
            OverlayAction::Toggle if self.preview_active(now) => None,
            OverlayAction::Toggle => Some(Preview::UntilHidden),
            OverlayAction::Peek => Some(Preview::Until(peek_until)),
        };
    }

    pub fn peek(&mut self, until: Instant) {
        self.peek_until = Some(until);
    }

    pub fn stop_peek(&mut self) {
        self.peek_until = None;
    }

    fn hide(&mut self) {
        self.pinned = false;
        self.peek_until = None;
        self.preview = None;
    }

    /// Drop deadlines that have passed.
    pub fn expire(&mut self, now: Instant) {
        if self.peek_until.is_some_and(|t| t <= now) {
            self.peek_until = None;
        }
        if matches!(self.preview, Some(Preview::Until(t)) if t <= now) {
            self.preview = None;
        }
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        let preview = match self.preview {
            Some(Preview::Until(t)) => Some(t),
            _ => None,
        };
        [self.peek_until, preview].into_iter().flatten().min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn pinned_shows_only_in_macro_mode_and_is_remembered() {
        let t0 = Instant::now();
        let mut m = OverlayModel::new(false);
        m.apply(OverlayAction::Toggle, t0);
        assert!(m.pinned);
        assert!(m.wanted(true, t0));
        assert!(!m.wanted(false, t0), "leaving macro mode hides");
        assert!(m.wanted(true, t0), "re-entering with pinned shows again");
        m.apply(OverlayAction::Toggle, t0);
        assert!(!m.pinned && !m.wanted(true, t0));
    }

    #[test]
    fn peek_expires() {
        let t0 = Instant::now();
        let mut m = OverlayModel::new(false);
        m.peek(t0 + ms(1600));
        assert!(m.wanted(true, t0 + ms(1599)));
        assert!(!m.wanted(true, t0 + ms(1600)));
        assert_eq!(m.next_deadline(), Some(t0 + ms(1600)));
        m.expire(t0 + ms(1600));
        assert_eq!(m.next_deadline(), None);
    }

    #[test]
    fn toggle_during_peek_pins() {
        let t0 = Instant::now();
        let mut m = OverlayModel::new(false);
        m.peek(t0 + ms(1600));
        m.apply(OverlayAction::Toggle, t0);
        assert!(m.pinned);
        assert!(m.wanted(true, t0 + ms(5000)));
    }

    #[test]
    fn hide_clears_peek_and_pin() {
        let t0 = Instant::now();
        let mut m = OverlayModel::new(true);
        m.peek(t0 + ms(1000));
        m.apply(OverlayAction::Hide, t0);
        assert!(!m.wanted(true, t0));
    }

    #[test]
    fn preview_ignores_macro_mode_and_keeps_pin() {
        let t0 = Instant::now();
        let mut m = OverlayModel::new(true);
        m.apply_preview(OverlayAction::Toggle, t0, t0);
        assert!(m.wanted(false, t0));
        m.apply_preview(OverlayAction::Toggle, t0, t0);
        assert!(!m.wanted(false, t0));
        assert!(m.pinned, "preview never changes the pin");

        m.apply_preview(OverlayAction::Peek, t0 + ms(800), t0);
        assert!(m.wanted(false, t0 + ms(799)));
        assert!(!m.wanted(false, t0 + ms(800)));
        assert_eq!(m.next_deadline(), Some(t0 + ms(800)));
    }
}
