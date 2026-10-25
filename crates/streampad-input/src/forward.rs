//! Frame-level forwarding: turns the raw event stream of a grabbed device into the frames written to its
//! mirror, plus the effects StreamPad acts on. Pure (no IO), so the framing rules are unit-tested.

use crate::KeyAction;
use crate::capture::{KeyRouter, Mode, Verdict};
use crate::sys::{
    EV_FF, EV_FF_STATUS, EV_KEY, EV_LED, EV_MSC, EV_PWR, EV_REP, EV_SND, EV_SYN, InputEvent, MSC_SCAN, SYN_DROPPED,
    SYN_REPORT,
};
use streampad_core::KeyId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    Pad(KeyId, KeyAction),
    ToggleMode,
    /// The kernel dropped events (`SYN_DROPPED`); call [`Forwarder::resync`] with the device's key state.
    Resync,
    /// Someone (we, the compositor) set an LED on the device. A grab doesn't stop other clients from
    /// writing LEDs, so this is how an overwritten macro-mode LED is noticed.
    Led(u16, i32),
}

pub(crate) struct Forwarder {
    router: KeyRouter,
    /// Events of the current frame that go to the mirror.
    frame: Vec<InputEvent>,
    /// A `MSC_SCAN` waiting for the key event it describes: it is dropped together with a captured key.
    scan: Option<InputEvent>,
    /// Inside a `SYN_DROPPED` gap: everything up to the next `SYN_REPORT` is incomplete and discarded.
    dropping: bool,
}

impl Forwarder {
    pub fn new() -> Forwarder {
        Forwarder { router: KeyRouter::new(), frame: Vec::with_capacity(16), scan: None, dropping: false }
    }

    /// Feeds one event read from the device; completed frames are appended to `out`.
    pub fn push(&mut self, ev: InputEvent, mode: &Mode, out: &mut Vec<InputEvent>) -> Option<Effect> {
        if self.dropping {
            if ev.is(EV_SYN, SYN_REPORT) {
                self.dropping = false;
                return Some(Effect::Resync);
            }
            return None;
        }
        match ev.kind {
            EV_SYN if ev.code == SYN_DROPPED => {
                self.frame.clear();
                self.scan = None;
                self.dropping = true;
            }
            EV_SYN if ev.code == SYN_REPORT => {
                self.flush_scan();
                self.end_frame(ev, out);
            }
            EV_MSC if ev.code == MSC_SCAN => {
                self.flush_scan();
                self.scan = Some(ev);
            }
            EV_KEY => {
                let scan = self.scan.take();
                let verdict = self.router.route(ev.code, ev.value, mode);
                if verdict.forward {
                    self.frame.extend(scan);
                    self.frame.push(ev);
                }
                return effect(verdict);
            }
            // LED, repeat-rate, sound and force-feedback events coming *from* a device are echoes of what
            // clients wrote to it, not input; the mirror's clients manage their own.
            EV_LED => return Some(Effect::Led(ev.code, ev.value)),
            EV_REP | EV_SND | EV_FF | EV_PWR | EV_FF_STATUS => {}
            _ => {
                self.flush_scan();
                self.frame.push(ev);
            }
        }
        None
    }

    /// After a `SYN_DROPPED` gap: releases every held key that is no longer down on the device. Forwarded
    /// releases are appended to `out` as one frame; captured ones are returned.
    pub fn resync(&mut self, is_down: impl Fn(u16) -> bool, out: &mut Vec<InputEvent>) -> Vec<(KeyId, KeyAction)> {
        let released = self.router.release_where(|code| !is_down(code));
        self.release(released, out)
    }

    /// Releases every held key (teardown). Forwarded keys need no event: destroying the mirror releases
    /// them in the kernel. Returns the captured keys that were still held.
    pub fn release_all(&mut self) -> Vec<KeyId> {
        let released = self.router.release_where(|_| true);
        let mut ignored = Vec::new();
        self.release(released, &mut ignored).into_iter().map(|(key, _)| key).collect()
    }

    fn release(&mut self, released: Vec<(u16, Verdict)>, out: &mut Vec<InputEvent>) -> Vec<(KeyId, KeyAction)> {
        let mut pad = Vec::new();
        for (code, verdict) in released {
            if verdict.forward {
                self.frame.push(InputEvent::new(EV_KEY, code, 0));
            }
            pad.extend(verdict.pad);
        }
        self.end_frame(InputEvent::syn_report(), out);
        pad
    }

    fn flush_scan(&mut self) {
        self.frame.extend(self.scan.take());
    }

    /// Events keep their timestamps: uinput passes them on, so the compositor sees the original timing.
    fn end_frame(&mut self, syn: InputEvent, out: &mut Vec<InputEvent>) {
        if !self.frame.is_empty() {
            out.append(&mut self.frame);
            out.push(syn);
        }
    }
}

fn effect(verdict: Verdict) -> Option<Effect> {
    if verdict.toggle { Some(Effect::ToggleMode) } else { verdict.pad.map(|(key, action)| Effect::Pad(key, action)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CaptureSet;
    use crate::sys::{EV_REL, LED_SCROLLL};

    const KP4: u16 = KeyId::KP4.evdev_code();
    const KEY_A: u16 = 30;

    fn mode(macro_mode: bool) -> Mode {
        Mode { macro_mode, capture: CaptureSet::ALL, toggle_key: 70, pass_toggle_key: false }
    }

    fn ev(kind: u16, code: u16, value: i32) -> InputEvent {
        InputEvent::new(kind, code, value)
    }

    fn key(code: u16, value: i32) -> [InputEvent; 3] {
        [ev(EV_MSC, MSC_SCAN, 0x70000 + i32::from(code)), ev(EV_KEY, code, value), InputEvent::syn_report()]
    }

    fn feed(fwd: &mut Forwarder, events: &[InputEvent], m: &Mode) -> (Vec<InputEvent>, Vec<Effect>) {
        let mut out = Vec::new();
        let effects = events.iter().filter_map(|e| fwd.push(*e, m, &mut out)).collect();
        (out, effects)
    }

    #[test]
    fn passthrough_keeps_frames_intact() {
        let mut fwd = Forwarder::new();
        let input = key(KP4, 1);
        let (out, effects) = feed(&mut fwd, &input, &mode(false));
        assert_eq!(out, input.to_vec());
        assert!(effects.is_empty());
    }

    #[test]
    fn frames_keep_their_timestamps() {
        let mut fwd = Forwarder::new();
        let input = key(KEY_A, 1).map(|e| InputEvent { sec: 12, usec: 345, ..e });
        let (out, _) = feed(&mut fwd, &input, &mode(false));
        assert_eq!(out, input.to_vec());
    }

    #[test]
    fn captured_key_drops_the_whole_frame_including_its_scan_code() {
        let mut fwd = Forwarder::new();
        let (out, effects) = feed(&mut fwd, &key(KP4, 1), &mode(true));
        assert!(out.is_empty(), "no empty SYN_REPORT for a fully captured frame");
        assert_eq!(effects, vec![Effect::Pad(KeyId::KP4, KeyAction::Press)]);
    }

    #[test]
    fn mixed_frame_keeps_only_forwarded_events() {
        let mut fwd = Forwarder::new();
        let input = [
            ev(EV_MSC, MSC_SCAN, 1),
            ev(EV_KEY, KEY_A, 1),
            ev(EV_MSC, MSC_SCAN, 2),
            ev(EV_KEY, KP4, 1),
            ev(EV_REL, 0, 5),
            InputEvent::syn_report(),
        ];
        let (out, effects) = feed(&mut fwd, &input, &mode(true));
        assert_eq!(out, vec![input[0], input[1], input[4], InputEvent::syn_report()]);
        assert_eq!(effects, vec![Effect::Pad(KeyId::KP4, KeyAction::Press)]);
    }

    #[test]
    fn toggle_key_produces_effect_and_no_output() {
        let mut fwd = Forwarder::new();
        let (out, effects) = feed(&mut fwd, &key(70, 1), &mode(false));
        assert!(out.is_empty());
        assert_eq!(effects, vec![Effect::ToggleMode]);
    }

    #[test]
    fn echoed_led_and_repeat_events_are_not_forwarded() {
        let mut fwd = Forwarder::new();
        let input = [ev(EV_LED, LED_SCROLLL, 1), ev(EV_REP, 0, 250), InputEvent::syn_report()];
        let (out, effects) = feed(&mut fwd, &input, &mode(false));
        assert!(out.is_empty());
        assert_eq!(effects, vec![Effect::Led(LED_SCROLLL, 1)]);
    }

    #[test]
    fn syn_dropped_discards_partial_frame_and_resyncs() {
        let mut fwd = Forwarder::new();
        let mut out = Vec::new();
        let m = mode(true);
        for e in key(KP4, 1).into_iter().chain(key(KEY_A, 1)) {
            fwd.push(e, &m, &mut out);
        }
        out.clear();
        assert_eq!(fwd.push(ev(EV_KEY, 31, 1), &m, &mut out), None);
        assert_eq!(fwd.push(ev(EV_SYN, SYN_DROPPED, 0), &m, &mut out), None);
        assert_eq!(fwd.push(ev(EV_KEY, KEY_A, 0), &m, &mut out), None, "events in the gap are discarded");
        assert_eq!(fwd.push(InputEvent::syn_report(), &m, &mut out), Some(Effect::Resync));
        assert!(out.is_empty(), "the partial frame before SYN_DROPPED is discarded");

        // No key is down any more: each is released on the side its press went to.
        let pad = fwd.resync(|_| false, &mut out);
        assert_eq!(pad, vec![(KeyId::KP4, KeyAction::Release)]);
        assert_eq!(out, vec![ev(EV_KEY, KEY_A, 0), ev(EV_KEY, 31, 0), InputEvent::syn_report()]);
    }

    #[test]
    fn release_all_reports_captured_keys() {
        let mut fwd = Forwarder::new();
        let m = mode(true);
        let mut out = Vec::new();
        for e in key(KP4, 1).into_iter().chain(key(KEY_A, 1)) {
            fwd.push(e, &m, &mut out);
        }
        assert_eq!(fwd.release_all(), vec![KeyId::KP4]);
        assert!(fwd.release_all().is_empty());
    }
}
