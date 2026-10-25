//! The synthetic "StreamPad Virtual Keyboard" that macros type with.

use crate::UntypableText;
use crate::caps::Capabilities;
use crate::sys::{BUS_VIRTUAL, EV_KEY, InputEvent};
use crate::uinput::{VirtualDevice, VirtualSpec, describe_open_error};
use anyhow::Context;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;
use streampad_core::combo::{Combo, KEY_LEFTSHIFT, char_to_key};

pub const VIRTUAL_KEYBOARD_NAME: &str = "StreamPad Virtual Keyboard";

/// How long `tap` holds a combo: long enough for compositors and apps that sample key state to see it.
const TAP_HOLD: Duration = Duration::from_millis(2);

/// Synthetic keyboard for macro output. Cheap to clone, `Send + Sync`, usable from any thread; each call
/// holds the device for its whole duration, so concurrent macros never interleave inside one combo.
#[derive(Clone)]
pub struct KeyOutput {
    keyboard: Arc<Mutex<SyntheticKeyboard>>,
}

impl KeyOutput {
    /// Creates the virtual keyboard. The compositor needs a moment (typically well under 100 ms) to pick up
    /// a new device; output sent before that is lost.
    pub fn create() -> anyhow::Result<KeyOutput> {
        let spec = VirtualSpec {
            name: VIRTUAL_KEYBOARD_NAME,
            id: libc::input_id { bustype: BUS_VIRTUAL, vendor: 0x5350, product: 0x0001, version: 1 },
            phys: "streampad/virtual-keyboard",
            caps: &Capabilities::synthetic_keyboard(),
        };
        let device = VirtualDevice::create(&spec).map_err(|err| anyhow::anyhow!(describe_open_error(&err)))?;
        Ok(KeyOutput { keyboard: Arc::new(Mutex::new(SyntheticKeyboard { device, held: Vec::new() })) })
    }

    /// Press all keys in order, then release in reverse order (with SYN_REPORTs and a ~2 ms gap so
    /// compositors register it). Keys already held through [`key_down`](Self::key_down) stay held.
    pub fn tap(&self, combo: &Combo) -> anyhow::Result<()> {
        self.lock().tap(&combo.keys).context("sending a shortcut failed")
    }

    pub fn key_down(&self, combo: &Combo) -> anyhow::Result<()> {
        let mut keyboard = self.lock();
        combo.keys.iter().try_for_each(|key| keyboard.press(*key)).context("sending a key press failed")
    }

    pub fn key_up(&self, combo: &Combo) -> anyhow::Result<()> {
        let mut keyboard = self.lock();
        combo.keys.iter().rev().try_for_each(|key| keyboard.release(*key)).context("sending a key release failed")
    }

    /// Type text using the US layout (`streampad_core::combo::char_to_key`), holding Shift for uppercase
    /// letters and symbols and waiting `delay` between characters. Returns `Err(UntypableText)` *before
    /// typing anything* if some character cannot be typed, so the caller can fall back to a clipboard paste.
    pub fn type_text(&self, text: &str, delay: Duration) -> anyhow::Result<()> {
        let strokes = plan_text(text)?;
        for (index, (code, shift)) in strokes.into_iter().enumerate() {
            if index > 0 && !delay.is_zero() {
                thread::sleep(delay);
            }
            let keys: &[u16] = if shift { &[KEY_LEFTSHIFT, code] } else { &[code] };
            self.lock().tap(keys).context("typing text failed")?;
        }
        Ok(())
    }

    /// Release every key this output currently holds down (used on cancel / shutdown).
    pub fn release_all(&self) {
        self.lock().release_all();
    }

    /// The virtual keyboard's `/dev/input/eventN` node.
    pub fn devnode(&self) -> Option<PathBuf> {
        self.lock().device.devnode()
    }

    fn lock(&self) -> MutexGuard<'_, SyntheticKeyboard> {
        // A panic mid-combo leaves `held` accurate (it is updated per written event), so poisoning is ignored.
        self.keyboard.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct SyntheticKeyboard {
    device: VirtualDevice,
    /// Keys currently held down, in press order.
    held: Vec<u16>,
}

impl SyntheticKeyboard {
    fn send(&mut self, code: u16, down: bool) -> io::Result<()> {
        self.device.write_events(&[InputEvent::new(EV_KEY, code, i32::from(down)), InputEvent::syn_report()])?;
        if down {
            self.held.push(code);
        } else {
            self.held.retain(|held| *held != code);
        }
        Ok(())
    }

    fn press(&mut self, code: u16) -> io::Result<()> {
        if self.held.contains(&code) { Ok(()) } else { self.send(code, true) }
    }

    fn release(&mut self, code: u16) -> io::Result<()> {
        if self.held.contains(&code) { self.send(code, false) } else { Ok(()) }
    }

    fn tap(&mut self, keys: &[u16]) -> io::Result<()> {
        let fresh: Vec<u16> = keys.iter().copied().filter(|key| !self.held.contains(key)).collect();
        let mut pressed = Vec::with_capacity(fresh.len());
        let mut result = Ok(());
        for key in &fresh {
            result = self.send(*key, true);
            if result.is_err() {
                break;
            }
            pressed.push(*key);
        }
        if result.is_ok() {
            thread::sleep(TAP_HOLD);
        }
        // Release what was pressed even if a later press failed, so a failed tap never leaves keys down.
        for key in pressed.iter().rev() {
            let released = self.send(*key, false);
            result = result.and(released);
        }
        result
    }

    fn release_all(&mut self) {
        for code in std::mem::take(&mut self.held).into_iter().rev() {
            if let Err(err) = self.device.write_events(&[InputEvent::new(EV_KEY, code, 0), InputEvent::syn_report()]) {
                log::warn!("releasing synthetic key {code} failed: {err}");
            }
        }
    }
}

impl Drop for SyntheticKeyboard {
    fn drop(&mut self) {
        self.release_all();
    }
}

/// Maps text to (key code, needs shift) strokes, or lists the characters that can't be typed.
fn plan_text(text: &str) -> Result<Vec<(u16, bool)>, UntypableText> {
    let text = text.replace("\r\n", "\n");
    let mut untypable = String::new();
    let strokes = text
        .chars()
        .filter_map(|c| {
            let stroke = char_to_key(c);
            if stroke.is_none() && !untypable.contains(c) {
                untypable.push(c);
            }
            stroke
        })
        .collect();
    if untypable.is_empty() { Ok(strokes) } else { Err(UntypableText(untypable)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_shifted_and_plain_strokes() {
        assert_eq!(plan_text("aA!").unwrap(), vec![(30, false), (30, true), (2, true)]);
        assert_eq!(plan_text("a\r\nb").unwrap(), vec![(30, false), (28, false), (48, false)]);
        assert!(plan_text("").unwrap().is_empty());
    }

    #[test]
    fn rejects_untypable_text_listing_each_character_once() {
        let err = plan_text("héllo wörld é").unwrap_err();
        assert_eq!(err.0, "éö");
        let anyhow_err: anyhow::Error = err.into();
        assert!(anyhow_err.downcast_ref::<UntypableText>().is_some(), "callers detect it by downcasting");
    }
}
