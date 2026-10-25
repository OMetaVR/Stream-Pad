//! The capture state machine (docs/SPEC.md §2.1), free of IO.
//!
//! Every key press is routed either to the system (forwarded to the mirror) or to StreamPad (a captured pad
//! key), and the router remembers that decision until the key is released: repeats and the release always
//! follow their press, whatever happened to the mode, the capture set or the toggle key in between. That one
//! rule is what guarantees there are never stuck keys on either side.

use crate::KeyAction;
use crate::sys::KEY_MAX;
use streampad_core::KeyId;

/// Pad keys captured in macro mode: a bitmask over `KeyId` so it can live in an atomic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CaptureSet(u32);

impl CaptureSet {
    pub const ALL: CaptureSet = CaptureSet((1 << KeyId::ALL.len()) - 1);

    pub fn from_bits(bits: u32) -> CaptureSet {
        CaptureSet(bits & CaptureSet::ALL.0)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, key: KeyId) -> bool {
        self.0 & mask(key) != 0
    }
}

fn mask(key: KeyId) -> u32 {
    1 << key as u32
}

impl FromIterator<KeyId> for CaptureSet {
    fn from_iter<I: IntoIterator<Item = KeyId>>(iter: I) -> Self {
        CaptureSet(iter.into_iter().fold(0, |bits, key| bits | mask(key)))
    }
}

/// The settings that influence routing, sampled per event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Mode {
    pub macro_mode: bool,
    pub capture: CaptureSet,
    pub toggle_key: u16,
    pub pass_toggle_key: bool,
}

/// Where a held key's press went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    System,
    Pad(KeyId),
    /// The macro-mode toggle key; `forwarded` if it was passed through when pressed.
    Toggle {
        forwarded: bool,
    },
}

/// What to do with one key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Verdict {
    /// Write it to the mirror.
    pub forward: bool,
    /// Report it to StreamPad as a pad key.
    pub pad: Option<(KeyId, KeyAction)>,
    /// Flip macro mode (toggle-key press).
    pub toggle: bool,
}

impl Verdict {
    const FORWARD: Verdict = Verdict { forward: true, pad: None, toggle: false };
    const DROP: Verdict = Verdict { forward: false, pad: None, toggle: false };
}

pub(crate) struct KeyRouter {
    /// Indexed by key code; `Some` while the key is held.
    held: Vec<Option<Route>>,
}

impl KeyRouter {
    pub fn new() -> KeyRouter {
        KeyRouter { held: vec![None; usize::from(KEY_MAX) + 1] }
    }

    /// Routes one `EV_KEY` event (`value` 1 = press, 2 = repeat, 0 = release).
    pub fn route(&mut self, code: u16, value: i32, mode: &Mode) -> Verdict {
        let Some(slot) = self.held.get_mut(usize::from(code)) else {
            return Verdict::FORWARD;
        };
        match value {
            1 => {
                // The kernel never repeats a press for a held key; if one shows up anyway, the key keeps
                // its original route so its eventual release can't be orphaned.
                let route = *slot.get_or_insert_with(|| decide(code, mode));
                verdict(route, KeyAction::Press)
            }
            2 => slot.map_or(Verdict::DROP, |route| verdict(route, KeyAction::Repeat)),
            // A release we never saw the press of (pressed during an event overflow) is harmless to
            // forward: the mirror drops releases of keys that aren't down.
            _ => slot.take().map_or(Verdict::FORWARD, |route| verdict(route, KeyAction::Release)),
        }
    }

    /// Releases every held key for which `release` returns true, returning each key's release verdict.
    pub fn release_where(&mut self, mut release: impl FnMut(u16) -> bool) -> Vec<(u16, Verdict)> {
        let mut released = Vec::new();
        for (code, slot) in (0..=KEY_MAX).zip(self.held.iter_mut()) {
            if slot.is_some()
                && release(code)
                && let Some(route) = slot.take()
            {
                released.push((code, verdict(route, KeyAction::Release)));
            }
        }
        released
    }
}

fn decide(code: u16, mode: &Mode) -> Route {
    if code == mode.toggle_key {
        return Route::Toggle { forwarded: mode.pass_toggle_key };
    }
    match KeyId::from_evdev_code(code) {
        Some(key) if mode.macro_mode && mode.capture.contains(key) => Route::Pad(key),
        _ => Route::System,
    }
}

fn verdict(route: Route, action: KeyAction) -> Verdict {
    match route {
        Route::System => Verdict::FORWARD,
        Route::Pad(key) => Verdict { pad: Some((key, action)), ..Verdict::DROP },
        Route::Toggle { forwarded } => Verdict { forward: forwarded, toggle: action == KeyAction::Press, pad: None },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use KeyAction::{Press, Release, Repeat};

    const KP4: u16 = KeyId::KP4.evdev_code();
    const KP5: u16 = KeyId::KP5.evdev_code();
    const SCROLL_LOCK: u16 = 70;
    const KEY_A: u16 = 30;

    fn mode(macro_mode: bool) -> Mode {
        Mode { macro_mode, capture: CaptureSet::ALL, toggle_key: SCROLL_LOCK, pass_toggle_key: false }
    }

    fn pad(key: KeyId, action: KeyAction) -> Verdict {
        Verdict { forward: false, pad: Some((key, action)), toggle: false }
    }

    #[test]
    fn numpad_mode_passes_everything_through() {
        let mut router = KeyRouter::new();
        let m = mode(false);
        for code in [KP4, KP5, KEY_A, KeyId::KPEnter.evdev_code()] {
            assert_eq!(router.route(code, 1, &m), Verdict::FORWARD);
            assert_eq!(router.route(code, 2, &m), Verdict::FORWARD);
            assert_eq!(router.route(code, 0, &m), Verdict::FORWARD);
        }
    }

    #[test]
    fn macro_mode_captures_pad_keys_only() {
        let mut router = KeyRouter::new();
        let m = mode(true);
        assert_eq!(router.route(KP4, 1, &m), pad(KeyId::KP4, Press));
        assert_eq!(router.route(KP4, 0, &m), pad(KeyId::KP4, Release));
        assert_eq!(router.route(KEY_A, 1, &m), Verdict::FORWARD, "non-pad keys always pass through");
        assert_eq!(router.route(KEY_A, 0, &m), Verdict::FORWARD);
    }

    #[test]
    fn repeats_follow_their_press() {
        let mut router = KeyRouter::new();
        assert_eq!(router.route(KP4, 1, &mode(true)), pad(KeyId::KP4, Press));
        assert_eq!(router.route(KP4, 2, &mode(true)), pad(KeyId::KP4, Repeat));
        assert_eq!(router.route(KP4, 2, &mode(false)), pad(KeyId::KP4, Repeat), "mode change doesn't divert repeats");
        assert_eq!(router.route(KP4, 0, &mode(false)), pad(KeyId::KP4, Release));
        assert_eq!(router.route(KP4, 2, &mode(true)), Verdict::DROP, "a repeat without a held press is dropped");
    }

    #[test]
    fn toggle_key_is_consumed_and_flips_once() {
        let mut router = KeyRouter::new();
        let m = mode(false);
        assert_eq!(router.route(SCROLL_LOCK, 1, &m), Verdict { forward: false, pad: None, toggle: true });
        assert_eq!(router.route(SCROLL_LOCK, 2, &m), Verdict::DROP, "auto-repeat must not toggle again");
        assert_eq!(router.route(SCROLL_LOCK, 0, &m), Verdict::DROP);
    }

    #[test]
    fn toggle_key_can_be_passed_through() {
        let mut router = KeyRouter::new();
        let m = Mode { pass_toggle_key: true, ..mode(true) };
        assert_eq!(router.route(SCROLL_LOCK, 1, &m), Verdict { forward: true, pad: None, toggle: true });
        assert_eq!(router.route(SCROLL_LOCK, 2, &m), Verdict::FORWARD);
        assert_eq!(router.route(SCROLL_LOCK, 0, &m), Verdict::FORWARD);
    }

    #[test]
    fn toggle_wins_over_capture_when_it_is_a_pad_key() {
        let mut router = KeyRouter::new();
        let m = Mode { toggle_key: KeyId::NumLock.evdev_code(), ..mode(true) };
        assert!(router.route(KeyId::NumLock.evdev_code(), 1, &m).toggle);
    }

    #[test]
    fn release_follows_press_into_macro_mode() {
        let mut router = KeyRouter::new();
        assert_eq!(router.route(KP4, 1, &mode(false)), Verdict::FORWARD);
        // toggled on while KP4 is held
        assert_eq!(router.route(KP4, 2, &mode(true)), Verdict::FORWARD);
        assert_eq!(router.route(KP4, 0, &mode(true)), Verdict::FORWARD, "release must reach the system");
        assert_eq!(router.route(KP4, 1, &mode(true)), pad(KeyId::KP4, Press), "the next press is captured");
    }

    #[test]
    fn release_follows_press_out_of_macro_mode() {
        let mut router = KeyRouter::new();
        assert_eq!(router.route(KP4, 1, &mode(true)), pad(KeyId::KP4, Press));
        assert_eq!(router.route(KP4, 0, &mode(false)), pad(KeyId::KP4, Release), "release must reach StreamPad");
        assert_eq!(router.route(KP4, 1, &mode(false)), Verdict::FORWARD);
    }

    #[test]
    fn uncaptured_pad_key_passes_through_in_macro_mode() {
        let mut router = KeyRouter::new();
        let m = Mode { capture: [KeyId::KP5].into_iter().collect(), ..mode(true) };
        assert_eq!(router.route(KP4, 1, &m), Verdict::FORWARD);
        assert_eq!(router.route(KP4, 0, &m), Verdict::FORWARD);
        assert_eq!(router.route(KP5, 1, &m), pad(KeyId::KP5, Press));
    }

    #[test]
    fn capture_set_change_while_held() {
        let mut router = KeyRouter::new();
        let only_kp5 = Mode { capture: [KeyId::KP5].into_iter().collect(), ..mode(true) };
        let none = Mode { capture: CaptureSet::from_bits(0), ..mode(true) };
        assert_eq!(router.route(KP4, 1, &only_kp5), Verdict::FORWARD);
        assert_eq!(router.route(KP5, 1, &only_kp5), pad(KeyId::KP5, Press));
        // capture set swapped while both are held
        assert_eq!(router.route(KP4, 0, &mode(true)), Verdict::FORWARD);
        assert_eq!(router.route(KP5, 2, &none), pad(KeyId::KP5, Repeat));
        assert_eq!(router.route(KP5, 0, &none), pad(KeyId::KP5, Release));
    }

    #[test]
    fn toggle_key_change_while_held() {
        let mut router = KeyRouter::new();
        assert!(router.route(SCROLL_LOCK, 1, &mode(false)).toggle);
        let rebound = Mode { toggle_key: 119, ..mode(true) };
        assert_eq!(router.route(SCROLL_LOCK, 0, &rebound), Verdict::DROP, "old toggle press keeps its route");
    }

    #[test]
    fn unknown_release_is_forwarded() {
        let mut router = KeyRouter::new();
        assert_eq!(router.route(KP4, 0, &mode(true)), Verdict::FORWARD);
    }

    #[test]
    fn release_where_reports_held_keys() {
        let mut router = KeyRouter::new();
        router.route(KP4, 1, &mode(true));
        router.route(KEY_A, 1, &mode(true));
        router.route(KP5, 1, &mode(true));
        let released = router.release_where(|code| code != KP5);
        assert_eq!(released, vec![(KEY_A, Verdict::FORWARD), (KP4, pad(KeyId::KP4, Release))]);
        assert_eq!(router.release_where(|_| true), vec![(KP5, pad(KeyId::KP5, Release))]);
        assert!(router.release_where(|_| true).is_empty());
    }

    #[test]
    fn capture_set_bits() {
        assert!(KeyId::ALL.iter().all(|k| CaptureSet::ALL.contains(*k)));
        let set: CaptureSet = [KeyId::NumLock, KeyId::KPDot].into_iter().collect();
        assert_eq!(CaptureSet::from_bits(set.bits()), set);
        assert!(set.contains(KeyId::KPDot) && !set.contains(KeyId::KP0));
        assert_eq!(CaptureSet::from_bits(u32::MAX), CaptureSet::ALL);
    }
}
