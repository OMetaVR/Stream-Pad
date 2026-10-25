//! State shared by the engine handle, the manager and every device thread. The input path only does atomic
//! loads here; the mutexes guard rare changes (mode flips, device list updates).

use crate::capture::{CaptureSet, Mode};
use crate::manager::{Command, ManagerLink};
use crate::{EngineConfig, EngineEvent, ModeSource};
use crossbeam_channel::{Sender, TrySendError};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use streampad_core::ipc::DeviceInfo;

pub(crate) struct Shared {
    macro_mode: AtomicBool,
    capture: AtomicU32,
    toggle_key: AtomicU16,
    pass_toggle_key: AtomicBool,
    /// Serializes mode changes so `ModeChanged` events go out in the order the changes happened.
    mode_change: Mutex<()>,
    devices: Mutex<Vec<DeviceInfo>>,
    events: Sender<EngineEvent>,
    manager: ManagerLink,
}

impl Shared {
    pub fn new(cfg: &EngineConfig, events: Sender<EngineEvent>, manager: ManagerLink) -> Shared {
        Shared {
            macro_mode: AtomicBool::new(cfg.macro_mode),
            capture: AtomicU32::new(CaptureSet::ALL.bits()),
            toggle_key: AtomicU16::new(cfg.toggle_key),
            pass_toggle_key: AtomicBool::new(cfg.pass_toggle_key),
            mode_change: Mutex::new(()),
            devices: Mutex::new(Vec::new()),
            events,
            manager,
        }
    }

    pub fn mode(&self) -> Mode {
        Mode {
            macro_mode: self.macro_mode.load(Ordering::Acquire),
            capture: CaptureSet::from_bits(self.capture.load(Ordering::Acquire)),
            toggle_key: self.toggle_key.load(Ordering::Acquire),
            pass_toggle_key: self.pass_toggle_key.load(Ordering::Acquire),
        }
    }

    pub fn macro_mode(&self) -> bool {
        self.macro_mode.load(Ordering::Acquire)
    }

    pub fn toggle_key(&self) -> u16 {
        self.toggle_key.load(Ordering::Acquire)
    }

    pub fn set_macro_mode(&self, on: bool, source: ModeSource) {
        let _order = lock(&self.mode_change);
        if self.macro_mode.swap(on, Ordering::AcqRel) != on {
            self.mode_changed(on, source);
        }
    }

    pub fn toggle_macro_mode(&self, source: ModeSource) {
        let _order = lock(&self.mode_change);
        let on = !self.macro_mode.fetch_xor(true, Ordering::AcqRel);
        self.mode_changed(on, source);
    }

    fn mode_changed(&self, macro_mode: bool, source: ModeSource) {
        self.emit(EngineEvent::ModeChanged { macro_mode, source });
        self.manager.send(Command::SyncLeds);
    }

    pub fn set_capture(&self, capture: CaptureSet) {
        self.capture.store(capture.bits(), Ordering::Release);
    }

    pub fn set_toggle_key(&self, code: u16, pass_through: bool) {
        self.toggle_key.store(code, Ordering::Release);
        self.pass_toggle_key.store(pass_through, Ordering::Release);
    }

    /// Never blocks: with a bounded channel that is full, the event is dropped (and logged).
    pub fn emit(&self, event: EngineEvent) {
        if let Err(TrySendError::Full(event)) = self.events.try_send(event) {
            log::warn!("engine event channel is full, dropping {event:?}");
        }
    }

    pub fn devices(&self) -> Vec<DeviceInfo> {
        lock(&self.devices).clone()
    }

    /// Stores the device list and emits `DevicesChanged` if it differs from the previous one.
    pub fn publish_devices(&self, devices: Vec<DeviceInfo>) {
        let mut current = lock(&self.devices);
        if *current != devices {
            *current = devices.clone();
            drop(current);
            self.emit(EngineEvent::DevicesChanged(devices));
        }
    }
}

/// The data behind these locks stays consistent even if a holder panicked, so poisoning is ignored.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
