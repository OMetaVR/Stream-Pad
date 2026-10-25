//! StreamPad input engine (docs/SPEC.md §2.1): grabs keyboards via evdev, mirrors them losslessly through
//! uinput, captures numpad keys while macro mode is on, and provides a synthetic keyboard for macro output.
//!
//! Architecture:
//! * A **manager thread** owns the device registry: it discovers `/dev/input/event*`, follows hotplug via
//!   inotify, applies [`Engine::reconfigure`], relays LED updates and enforces the grab timeout.
//! * One **device thread** per grabbed keyboard creates its mirror (`"StreamPad: <name>"`), waits until no key
//!   is held, grabs the source and forwards every event frame to the mirror — except captured pad keys in
//!   macro mode and the toggle key — and forwards the compositor's LED writes back to the keyboard
//!   (Scroll Lock always shows macro mode). The thread owns both fds, so a panic still releases the grab.
//! * The routing decision is a pure state machine (`capture`) that remembers where each press went, so
//!   repeats and releases always follow their press and keys never get stuck on either side.
//! * The input path never blocks on anything but the device: settings are atomics, events go out with
//!   `try_send`.
//!
//! [`KeyOutput`] drives the separate "StreamPad Virtual Keyboard" used by macros.

mod bits;
mod caps;
mod capture;
mod discovery;
mod evdev;
mod forward;
mod manager;
mod mirror;
mod output;
mod shared;
mod sys;
mod sysfs;
mod uinput;
mod worker;

use crossbeam_channel::Sender;
use manager::{Command, Manager, ManagerLink};
use shared::Shared;
use std::collections::HashSet;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use streampad_core::KeyId;
use streampad_core::ipc::DeviceInfo;

pub use discovery::OWN_DEVICE_PREFIX;
pub use mirror::MIRROR_NAME_PREFIX;
pub use output::{KeyOutput, VIRTUAL_KEYBOARD_NAME};

/// Which physical devices to grab.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceFilter {
    /// `true` = grab every eligible keyboard except `exclude`; `false` = grab only `include`.
    pub auto: bool,
    /// Device ids (`"<vendor hex4>:<product hex4>:<name>"`).
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// Testing / `--only-device`: if set, ONLY devices whose id, `/dev/input/eventN` path, or name equals
    /// this string are considered at all (overrides everything else).
    pub only: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// evdev code of the macro-mode toggle key (default 70 = Scroll Lock).
    pub toggle_key: u16,
    /// Pass the toggle key through to the system as well (default false = consume it).
    pub pass_toggle_key: bool,
    pub devices: DeviceFilter,
    /// Development safety valve: release all grabs and stop after this long. Only read by `Engine::start`.
    pub grab_timeout: Option<Duration>,
    /// Initial macro mode. Only read by `Engine::start`.
    pub macro_mode: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            toggle_key: streampad_core::keys::KEY_SCROLLLOCK,
            pass_toggle_key: false,
            devices: DeviceFilter { auto: true, ..Default::default() },
            grab_timeout: None,
            macro_mode: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Press,
    Release,
    /// Kernel auto-repeat while held.
    Repeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeSource {
    /// The toggle key was pressed on a device.
    ToggleKey,
    /// `Engine::set_macro_mode` was called.
    Api,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    ModeChanged {
        macro_mode: bool,
        source: ModeSource,
    },
    /// A captured pad key event (only emitted for captured keys; never for passed-through ones). Every
    /// `Press` is eventually followed by a `Release`, also when the device is unplugged or released.
    PadKey {
        key: KeyId,
        action: KeyAction,
    },
    DevicesChanged(Vec<DeviceInfo>),
    /// Non-fatal problem worth surfacing (e.g. permission denied on a device).
    Warning(String),
    /// The engine stopped (grab timeout, fatal error). All grabs are released.
    Stopped {
        reason: String,
    },
}

/// Running engine. Dropping it (or calling `shutdown`) releases every grab and destroys the mirrors.
pub struct Engine {
    shared: Arc<Shared>,
    link: ManagerLink,
    manager: Option<JoinHandle<()>>,
    output: KeyOutput,
}

impl Engine {
    /// Discover devices, create mirrors, grab (waiting until no keys are held on a device before grabbing it),
    /// start hotplug watching, and begin forwarding. Events are sent on `events` (never blocks the input path:
    /// use `try_send` / unbounded channel).
    ///
    /// Returns once discovery is done; [`devices`](Self::devices) then lists the keyboards found, and each
    /// selected one is grabbed by its device thread shortly after (`DevicesChanged` reports it). Fails only
    /// if the synthetic keyboard can't be created (no access to `/dev/uinput`).
    pub fn start(cfg: EngineConfig, events: Sender<EngineEvent>) -> anyhow::Result<Engine> {
        let output = KeyOutput::create()?;
        let (link, commands) = ManagerLink::new()?;
        let shared = Arc::new(Shared::new(&cfg, events, link.clone()));
        let mut manager =
            Manager::new(shared.clone(), link.clone(), commands, cfg.devices, cfg.grab_timeout, output.clone());
        manager.rescan();
        let manager = thread::Builder::new().name("streampad-input".into()).spawn(move || manager.run())?;
        Ok(Engine { shared, link, manager: Some(manager), output })
    }

    pub fn macro_mode(&self) -> bool {
        self.shared.macro_mode()
    }

    /// Set macro mode (updates Scroll Lock LEDs; emits `ModeChanged { source: Api }` if it changed).
    pub fn set_macro_mode(&self, on: bool) {
        self.shared.set_macro_mode(on, ModeSource::Api);
    }

    /// Which pad keys are captured while macro mode is on. Keys not in the set pass through even in macro
    /// mode. The daemon sets this to every bound/global key (or all 17 keys when `unboundKeys = "block"`).
    /// Until the first call, all 17 keys are captured. Keys held while the set changes keep their routing.
    pub fn set_capture(&self, keys: HashSet<KeyId>) {
        self.shared.set_capture(keys.into_iter().collect());
    }

    /// Apply a new config (device filter / toggle key). Re-evaluates devices, grabbing/releasing as needed.
    /// Devices whose grab failed are retried.
    pub fn reconfigure(&self, cfg: EngineConfig) {
        self.shared.set_toggle_key(cfg.toggle_key, cfg.pass_toggle_key);
        self.link.send(Command::Reconfigure(cfg.devices));
    }

    pub fn devices(&self) -> Vec<DeviceInfo> {
        self.shared.devices()
    }

    /// Handle to the synthetic "StreamPad Virtual Keyboard".
    pub fn output(&self) -> KeyOutput {
        self.output.clone()
    }

    pub fn shutdown(self) {
        drop(self);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.link.send(Command::Shutdown);
        if let Some(manager) = self.manager.take()
            && manager.join().is_err()
        {
            log::error!("the input manager thread panicked; its devices were released while unwinding");
        }
        self.output.release_all();
    }
}

#[derive(Debug, thiserror::Error)]
#[error("text contains characters that cannot be typed with the US layout: {0:?}")]
pub struct UntypableText(pub String);

/// Enumerate input devices without grabbing anything (for `streampadd list-devices`).
///
/// Lists keyboard-like devices (not StreamPad's own), restricted by `filter.only` when set. `eligible`
/// reflects capabilities only (has EV_REP, and the numpad keys or the toggle key); `grabbed` is always
/// false; devices that can't be opened are described from sysfs with `error` set.
pub fn list_devices(filter: &DeviceFilter, toggle_key: u16) -> Vec<DeviceInfo> {
    discovery::list_devices(filter, toggle_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_are_thread_safe() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Engine>();
        assert_send_sync::<KeyOutput>();
    }
}
