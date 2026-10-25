//! What an input device can report, queried from evdev in the shape needed to clone it through uinput.

use crate::bits::Bits;
use crate::sys::{self, EV_ABS, EV_KEY, EV_LED, EV_MSC, EV_REL, EV_REP, EV_SW};
use std::io;
use std::os::fd::BorrowedFd;
use streampad_core::KeyId;

/// The keys that make a device "have a numpad": KP0–KP9 and KP Enter.
const NUMPAD_KEYS: [KeyId; 11] = [
    KeyId::KP0,
    KeyId::KP1,
    KeyId::KP2,
    KeyId::KP3,
    KeyId::KP4,
    KeyId::KP5,
    KeyId::KP6,
    KeyId::KP7,
    KeyId::KP8,
    KeyId::KP9,
    KeyId::KPEnter,
];

/// `KEY_ESC ..= KEY_KPDOT`: the main block plus numpad. Devices with none of these (power buttons,
/// media-key-only interfaces, mice) are not keyboards as far as StreamPad is concerned.
const TYPING_KEYS: std::ops::RangeInclusive<u16> = 1..=83;

#[derive(Debug, Clone, Default)]
pub(crate) struct Capabilities {
    /// Supported event types (`EV_*`).
    pub types: Bits,
    pub keys: Bits,
    pub rel: Bits,
    pub abs: Vec<AbsAxis>,
    pub msc: Bits,
    pub led: Bits,
    pub sw: Bits,
    pub props: Bits,
}

#[derive(Clone, Copy)]
pub(crate) struct AbsAxis {
    pub code: u16,
    pub info: libc::input_absinfo,
}

impl std::fmt::Debug for AbsAxis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let i = &self.info;
        write!(
            f,
            "abs {:#x} [{}, {}] fuzz {} flat {} res {}",
            self.code, i.minimum, i.maximum, i.fuzz, i.flat, i.resolution
        )
    }
}

impl Capabilities {
    pub fn query(fd: BorrowedFd<'_>) -> io::Result<Capabilities> {
        let mut types = Bits::new();
        sys::event_bits(fd, 0, types.as_mut_bytes())?;
        let codes = |kind: u16| -> io::Result<Bits> {
            let mut bits = Bits::new();
            if types.contains(kind) {
                sys::event_bits(fd, kind, bits.as_mut_bytes())?;
            }
            Ok(bits)
        };
        let keys = codes(EV_KEY)?;
        let rel = codes(EV_REL)?;
        let abs = codes(EV_ABS)?
            .iter()
            .map(|code| sys::abs_info(fd, code).map(|info| AbsAxis { code, info }))
            .collect::<io::Result<Vec<_>>>()?;
        let msc = codes(EV_MSC)?;
        let led = codes(EV_LED)?;
        let sw = codes(EV_SW)?;
        let mut props = Bits::new();
        sys::property_bits(fd, props.as_mut_bytes())?;
        Ok(Capabilities { types, keys, rel, abs, msc, led, sw, props })
    }

    /// A keyboard with every `KEY_*` code and nothing else — deliberately no `BTN_*` codes, which would
    /// get it classified as a mouse or joystick (and joysticks are ignored by libinput).
    pub fn synthetic_keyboard() -> Capabilities {
        let keys: Bits = (1..=sys::KEY_MAX).filter(|code| streampad_core::combo::key_name(*code).is_some()).collect();
        Capabilities { types: [EV_KEY].into_iter().collect(), keys, ..Capabilities::default() }
    }

    pub fn has_type(&self, kind: u16) -> bool {
        self.types.contains(kind)
    }

    pub fn has_key(&self, code: u16) -> bool {
        self.has_type(EV_KEY) && self.keys.contains(code)
    }

    pub fn has_led(&self, code: u16) -> bool {
        self.has_type(EV_LED) && self.led.contains(code)
    }

    pub fn has_numpad(&self) -> bool {
        NUMPAD_KEYS.iter().all(|key| self.has_key(key.evdev_code()))
    }

    pub fn is_keyboard_like(&self, toggle_key: u16) -> bool {
        TYPING_KEYS.into_iter().any(|code| self.has_key(code)) || self.has_key(toggle_key)
    }

    /// Eligible for grabbing: a real keyboard (auto-repeat) with a numpad or the toggle key.
    pub fn is_eligible(&self, toggle_key: u16) -> bool {
        self.has_type(EV_REP) && (self.has_numpad() || self.has_key(toggle_key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keyboard(keys: &[u16], repeat: bool) -> Capabilities {
        let mut types: Bits = [EV_KEY].into_iter().collect();
        if repeat {
            types.insert(EV_REP);
        }
        Capabilities { types, keys: keys.iter().copied().collect(), ..Capabilities::default() }
    }

    #[test]
    fn eligibility() {
        let numpad: Vec<u16> = NUMPAD_KEYS.iter().map(|k| k.evdev_code()).collect();
        let full = keyboard(&numpad, true);
        assert!(full.has_numpad() && full.is_eligible(70));
        assert!(!keyboard(&numpad, false).is_eligible(70), "no EV_REP: not a keyboard");
        let tkl = keyboard(&[30, 70], true);
        assert!(!tkl.has_numpad() && tkl.is_eligible(70) && !tkl.is_eligible(119));
        let media = keyboard(&[113, 114, 115], true);
        assert!(!media.is_keyboard_like(70) && !media.is_eligible(70));
    }

    #[test]
    fn synthetic_keyboard_has_keys_but_no_buttons() {
        let caps = Capabilities::synthetic_keyboard();
        assert!(caps.has_key(30) && caps.has_key(183) && caps.has_numpad());
        let buttons = (0x100..0x160).chain(0x2c0..0x2e8);
        assert!(buttons.into_iter().all(|btn| !caps.has_key(btn)), "BTN_* codes must stay off");
        assert!(!caps.has_type(EV_REP) && !caps.has_type(EV_LED));
    }
}
