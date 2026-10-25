//! Test harness: fake source keyboards (uinput) and grabbed readers for the engine's output devices.
//!
//! Safety rules for every test: the engine only ever sees our fake devices (`DeviceFilter::only`), and the
//! test grabs the mirror and the virtual keyboard *before* injecting anything, so no event can reach the
//! desktop. Engines get a grab timeout as a safety valve in case a test hangs.
//!
//! Deliberately independent of the crate's own uinput/evdev code.

#![allow(dead_code)]

use crossbeam_channel::Receiver;
use nix::{request_code_none, request_code_read, request_code_write};
use std::collections::VecDeque;
use std::ffi::{CStr, c_int};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem::size_of;
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{Duration, Instant};
use streampad_input::{DeviceFilter, Engine, EngineConfig, EngineEvent};

pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
pub const EV_MSC: u16 = 4;
pub const EV_LED: u16 = 0x11;
pub const EV_REP: u16 = 0x14;
pub const MSC_SCAN: u16 = 4;
pub const LED_NUML: u16 = 0;
pub const LED_CAPSL: u16 = 1;
pub const LED_SCROLLL: u16 = 2;

pub const KEY_A: u16 = 30;
pub const KEY_LEFTSHIFT: u16 = 42;
pub const KEY_SCROLLLOCK: u16 = 70;
pub const KEY_KP4: u16 = 75;
pub const KEY_KP5: u16 = 76;

/// Hotplug and mirror creation take a few hundred ms; be generous so a loaded machine doesn't flake.
pub const SETTLE: Duration = Duration::from_secs(5);
/// How long to watch for events that must *not* arrive.
pub const QUIET: Duration = Duration::from_millis(250);

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct RawEvent {
    sec: libc::c_long,
    usec: libc::c_long,
    kind: u16,
    code: u16,
    value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
    /// Kernel timestamp (CLOCK_MONOTONIC). uinput passes injected timestamps on, so for mirror events
    /// this is when the *source* event happened.
    pub time: Duration,
    /// When the test read the event (CLOCK_MONOTONIC).
    pub received: Duration,
}

/// Tests run in parallel, except timing-sensitive ones, which hold this exclusively.
static TIMING: RwLock<()> = RwLock::new(());

pub fn run_shared() -> RwLockReadGuard<'static, ()> {
    TIMING.read().unwrap_or_else(PoisonError::into_inner)
}

pub fn run_exclusive() -> RwLockWriteGuard<'static, ()> {
    TIMING.write().unwrap_or_else(PoisonError::into_inner)
}

pub fn monotonic_now() -> Duration {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: valid out-pointer.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

fn ioctl_int(file: &File, request: nix::sys::ioctl::ioctl_num_type, arg: c_int) -> io::Result<()> {
    // SAFETY: by-value integer ioctl.
    let ret = unsafe { libc::ioctl(file.as_raw_fd(), request, arg) };
    if ret < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

fn write_raw(file: &File, events: &[(u16, u16, i32)]) {
    let raw: Vec<RawEvent> =
        events.iter().map(|&(kind, code, value)| RawEvent { sec: 0, usec: 0, kind, code, value }).collect();
    // SAFETY: RawEvent is plain old data.
    let bytes = unsafe { std::slice::from_raw_parts(raw.as_ptr().cast::<u8>(), raw.len() * size_of::<RawEvent>()) };
    (&*file).write_all(bytes).expect("writing input events");
}

/// Reads one batch of events, waiting up to `timeout`.
fn read_raw(file: &File, timeout: Duration) -> io::Result<Vec<Event>> {
    let mut fds = [nix::poll::PollFd::new(file.as_fd(), nix::poll::PollFlags::POLLIN)];
    let millis = timeout.as_millis().clamp(1, 60_000) as u16;
    if nix::poll::poll(&mut fds, millis).map_err(io::Error::from)? == 0 {
        return Ok(Vec::new());
    }
    let mut buf = [0u8; 64 * size_of::<RawEvent>()];
    let n = match (&*file).read(&mut buf) {
        Ok(n) => n,
        Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let received = monotonic_now();
    Ok(buf[..n]
        .chunks_exact(size_of::<RawEvent>())
        .map(|chunk| {
            // SAFETY: chunk is exactly one RawEvent; read_unaligned copes with the byte buffer's alignment.
            let raw: RawEvent = unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
            Event {
                kind: raw.kind,
                code: raw.code,
                value: raw.value,
                time: Duration::new(raw.sec as u64, (raw.usec * 1000) as u32),
                received,
            }
        })
        .collect())
}

pub fn random_name(prefix: &str) -> String {
    format!("SPTEST {prefix} {:08x}", fastrand::u32(..))
}

/// A fake physical keyboard: full key set incl. numpad, auto-repeat, scan codes and three LEDs.
pub struct FakeKeyboard {
    file: File,
    pub name: String,
    /// LED writes read but not yet consumed.
    leds: Mutex<VecDeque<Event>>,
}

impl FakeKeyboard {
    pub fn create(name: &str) -> FakeKeyboard {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")
            .expect("these tests need read/write access to /dev/uinput");
        let set =
            |nr: u8, bit: u16| ioctl_int(&file, request_code_write!(b'U', nr, size_of::<c_int>()), c_int::from(bit));
        let (evbit, keybit, mscbit, ledbit) = (100, 101, 104, 105);
        for kind in [EV_KEY, EV_MSC, EV_LED, EV_REP] {
            set(evbit, kind).expect("UI_SET_EVBIT");
        }
        for code in 1..=248 {
            set(keybit, code).expect("UI_SET_KEYBIT");
        }
        set(mscbit, MSC_SCAN).expect("UI_SET_MSCBIT");
        for led in [LED_NUML, LED_CAPSL, LED_SCROLLL] {
            set(ledbit, led).expect("UI_SET_LEDBIT");
        }
        let mut setup = libc::uinput_setup {
            id: libc::input_id { bustype: 0x03, vendor: 0x1209, product: 0x5350, version: 1 },
            name: [0; libc::UINPUT_MAX_NAME_SIZE],
            ff_effects_max: 0,
        };
        for (dst, src) in setup.name.iter_mut().zip(name.bytes().take(libc::UINPUT_MAX_NAME_SIZE - 1)) {
            *dst = src as libc::c_char;
        }
        let request = request_code_write!(b'U', 3, size_of::<libc::uinput_setup>());
        // SAFETY: valid uinput_setup pointer matching the request size.
        assert!(unsafe { libc::ioctl(file.as_raw_fd(), request, &setup) } >= 0, "UI_DEV_SETUP");
        // SAFETY: argument-less ioctl.
        assert!(unsafe { libc::ioctl(file.as_raw_fd(), request_code_none!(b'U', 1)) } >= 0, "UI_DEV_CREATE");
        FakeKeyboard { file, name: name.to_string(), leds: Mutex::new(VecDeque::new()) }
    }

    pub fn devnode(&self) -> PathBuf {
        let mut buf = [0u8; 64];
        let request = request_code_read!(b'U', 44, buf.len());
        // SAFETY: the request encodes the buffer length.
        assert!(unsafe { libc::ioctl(self.file.as_raw_fd(), request, buf.as_mut_ptr()) } >= 0, "UI_GET_SYSNAME");
        let sysname = CStr::from_bytes_until_nul(&buf).expect("sysname").to_string_lossy().into_owned();
        let dir = Path::new("/sys/devices/virtual/input").join(sysname);
        let deadline = Instant::now() + SETTLE;
        loop {
            let node = std::fs::read_dir(&dir).ok().and_then(|entries| {
                entries.filter_map(Result::ok).map(|e| e.file_name()).find(|n| n.to_string_lossy().starts_with("event"))
            });
            if let Some(node) = node {
                return Path::new("/dev/input").join(node);
            }
            assert!(Instant::now() < deadline, "no event node for {}", self.name);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// A key event framed like a USB keyboard reports it: MSC_SCAN, EV_KEY, SYN_REPORT.
    pub fn key(&self, code: u16, value: i32) {
        write_raw(&self.file, &[(EV_MSC, MSC_SCAN, 0x70000 + i32::from(code)), (EV_KEY, code, value), (EV_SYN, 0, 0)]);
    }

    pub fn press(&self, code: u16) {
        self.key(code, 1);
    }

    pub fn release(&self, code: u16) {
        self.key(code, 0);
    }

    /// Waits for an LED write (from the engine) matching `code`/`value`, consuming the writes before it.
    pub fn wait_led(&self, code: u16, value: i32, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut leds = self.leds.lock().expect("lock");
        loop {
            while let Some(event) = leds.pop_front() {
                if event.kind == EV_LED && event.code == code && event.value == value {
                    return true;
                }
            }
            let Some(left) = deadline.checked_duration_since(Instant::now()) else { return false };
            leds.extend(read_raw(&self.file, left).expect("reading LED writes"));
        }
    }

    /// Discards LED writes received so far.
    pub fn drain_leds(&self) {
        let mut leds = self.leds.lock().expect("lock");
        leds.clear();
        while !read_raw(&self.file, Duration::from_millis(1)).expect("reading LED writes").is_empty() {}
    }
}

impl Drop for FakeKeyboard {
    fn drop(&mut self) {
        // SAFETY: argument-less ioctl.
        unsafe { libc::ioctl(self.file.as_raw_fd(), request_code_none!(b'U', 2)) };
    }
}

/// An evdev node opened and EVIOCGRAB'ed by the test, so its events reach only us.
pub struct Grabbed {
    file: File,
    pending: Mutex<VecDeque<Event>>,
}

impl Grabbed {
    pub fn open(path: &Path) -> Grabbed {
        let deadline = Instant::now() + SETTLE;
        let file = loop {
            match OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK).open(path) {
                Ok(file) => break file,
                // udev may not have applied the group permissions yet
                Err(err) if err.kind() == io::ErrorKind::PermissionDenied && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(err) => panic!("cannot open {}: {err}", path.display()),
            }
        };
        ioctl_int(&file, request_code_write!(b'E', 0x90, size_of::<c_int>()), 1)
            .unwrap_or_else(|err| panic!("EVIOCGRAB {} failed: {err} — refusing to inject anything", path.display()));
        let clock: c_int = libc::CLOCK_MONOTONIC;
        let request = request_code_write!(b'E', 0xa0, size_of::<c_int>());
        // SAFETY: EVIOCSCLOCKID reads one int through the pointer.
        assert!(unsafe { libc::ioctl(file.as_raw_fd(), request, &clock) } >= 0, "EVIOCSCLOCKID");
        Grabbed { file, pending: Mutex::new(VecDeque::new()) }
    }

    pub fn next(&self, timeout: Duration) -> Option<Event> {
        let deadline = Instant::now() + timeout;
        let mut pending = self.pending.lock().expect("lock");
        loop {
            if let Some(event) = pending.pop_front() {
                return Some(event);
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            pending.extend(read_raw(&self.file, left).ok()?);
        }
    }

    /// Next key event as (code, value), skipping everything else.
    pub fn next_key(&self, timeout: Duration) -> Option<(u16, i32)> {
        self.next_key_event(timeout).map(|e| (e.code, e.value))
    }

    pub fn next_key_event(&self, timeout: Duration) -> Option<Event> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.checked_duration_since(Instant::now())?;
            let event = self.next(left)?;
            if event.kind == EV_KEY {
                return Some(event);
            }
        }
    }

    /// The next complete frame (events up to and including SYN_REPORT).
    pub fn next_frame(&self, timeout: Duration) -> Vec<(u16, u16, i32)> {
        let deadline = Instant::now() + timeout;
        let mut frame = Vec::new();
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            let Some(event) = self.next(left) else { break };
            frame.push((event.kind, event.code, event.value));
            if event.kind == EV_SYN && event.code == 0 {
                break;
            }
        }
        frame
    }

    /// Fails if a key event arrives within `window`.
    pub fn assert_no_keys(&self, window: Duration, what: &str) {
        if let Some(event) = self.next_key_event(window) {
            panic!("{what}: unexpected key event {event:?}");
        }
    }

    /// Writes events into the device as a client would (e.g. the compositor setting LEDs).
    pub fn write(&self, events: &[(u16, u16, i32)]) {
        write_raw(&self.file, events);
    }
}

/// Writes events to an evdev node without grabbing it, the way the compositor sets LEDs.
pub fn write_to_node(node: &Path, events: &[(u16, u16, i32)]) {
    let file = OpenOptions::new().write(true).open(node).unwrap_or_else(|err| panic!("open {}: {err}", node.display()));
    write_raw(&file, events);
}

/// Finds the event node of the device called `name`.
pub fn find_node(name: &str) -> PathBuf {
    let deadline = Instant::now() + SETTLE;
    loop {
        for entry in std::fs::read_dir("/dev/input").expect("/dev/input").filter_map(Result::ok) {
            if !entry.file_name().to_string_lossy().starts_with("event") {
                continue;
            }
            if sysfs_name(&entry.path()).as_deref() == Some(name) {
                return entry.path();
            }
        }
        assert!(Instant::now() < deadline, "no input device named {name:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn node_exists_named(name: &str) -> bool {
    std::fs::read_dir("/dev/input")
        .expect("/dev/input")
        .filter_map(Result::ok)
        .any(|entry| sysfs_name(&entry.path()).as_deref() == Some(name))
}

fn sysfs_name(node: &Path) -> Option<String> {
    let path = Path::new("/sys/class/input").join(node.file_name()?).join("device/name");
    std::fs::read_to_string(path).ok().map(|s| s.trim_end_matches('\n').to_string())
}

pub fn mirror_name(source: &str) -> String {
    format!("{}{source}", streampad_input::MIRROR_NAME_PREFIX)
}

pub fn config_for(only: &str) -> EngineConfig {
    EngineConfig {
        devices: DeviceFilter { auto: true, only: Some(only.to_string()), ..Default::default() },
        grab_timeout: Some(Duration::from_secs(60)),
        ..EngineConfig::default()
    }
}

pub fn start_engine(cfg: EngineConfig) -> (Engine, Receiver<EngineEvent>) {
    let (tx, rx) = crossbeam_channel::unbounded();
    let engine = Engine::start(cfg, tx).expect("engine start");
    (engine, rx)
}

/// Waits for an engine event matching `pred`, skipping others.
pub fn wait_event(
    rx: &Receiver<EngineEvent>,
    timeout: Duration,
    pred: impl Fn(&EngineEvent) -> bool,
) -> Option<EngineEvent> {
    let deadline = Instant::now() + timeout;
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        match rx.recv_timeout(left) {
            Ok(event) if pred(&event) => return Some(event),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
}

/// Fails if an event matching `pred` arrives within `window`.
pub fn assert_no_event(rx: &Receiver<EngineEvent>, window: Duration, pred: impl Fn(&EngineEvent) -> bool) {
    if let Some(event) = wait_event(rx, window, pred) {
        panic!("unexpected engine event {event:?}");
    }
}

pub fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    condition()
}

pub fn is_grabbed(engine: &Engine, name: &str) -> bool {
    engine.devices().iter().any(|d| d.name == name && d.grabbed)
}

/// An engine serving one new fake keyboard, with the keyboard's mirror grabbed by the test: the standard
/// setup for tests that inject keys.
pub struct Rig {
    pub engine: Engine,
    pub events: Receiver<EngineEvent>,
    pub mirror: Grabbed,
    pub keyboard: FakeKeyboard,
}

impl Rig {
    pub fn new(label: &str) -> Rig {
        Rig::with_config(label, |_| {})
    }

    pub fn with_config(label: &str, adjust: impl FnOnce(&mut EngineConfig)) -> Rig {
        let keyboard = FakeKeyboard::create(&random_name(label));
        let mut cfg = config_for(&keyboard.name);
        adjust(&mut cfg);
        let (engine, events) = start_engine(cfg);
        assert!(wait_until(SETTLE, || is_grabbed(&engine, &keyboard.name)), "{} was not grabbed", keyboard.name);
        let mirror = Grabbed::open(&find_node(&mirror_name(&keyboard.name)));
        Rig { engine, events, mirror, keyboard }
    }
}
