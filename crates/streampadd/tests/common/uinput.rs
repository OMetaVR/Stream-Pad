//! Fake keyboards (uinput) and grabbed readers for the daemon's output devices.
//!
//! Safety rules: the daemon only sees our fake keyboard (`--only-device`), and the test EVIOCGRABs the
//! keyboard's mirror and the daemon's virtual keyboard *before* injecting anything, so no event reaches the
//! desktop. Nothing is injected before the daemon has grabbed the fake keyboard.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::mem::size_of;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
const EV_MSC: u16 = 4;
const EV_LED: u16 = 0x11;
const EV_REP: u16 = 0x14;
const MSC_SCAN: u16 = 4;

pub const SETTLE: Duration = Duration::from_secs(5);

const fn iow(kind: u8, nr: u8, size: usize) -> libc::c_ulong {
    ((1 << 30) | (size << 16) | ((kind as usize) << 8) | nr as usize) as libc::c_ulong
}

const fn io(kind: u8, nr: u8) -> libc::c_ulong {
    (((kind as usize) << 8) | nr as usize) as libc::c_ulong
}

const UI_DEV_CREATE: libc::c_ulong = io(b'U', 1);
const UI_DEV_DESTROY: libc::c_ulong = io(b'U', 2);
const UI_DEV_SETUP: libc::c_ulong = iow(b'U', 3, size_of::<libc::uinput_setup>());
const UI_SET_EVBIT: libc::c_ulong = iow(b'U', 100, size_of::<libc::c_int>());
const UI_SET_KEYBIT: libc::c_ulong = iow(b'U', 101, size_of::<libc::c_int>());
const UI_SET_MSCBIT: libc::c_ulong = iow(b'U', 104, size_of::<libc::c_int>());
const UI_SET_LEDBIT: libc::c_ulong = iow(b'U', 105, size_of::<libc::c_int>());
const EVIOCGRAB: libc::c_ulong = iow(b'E', 0x90, size_of::<libc::c_int>());

fn ioctl_int(file: &File, request: libc::c_ulong, arg: libc::c_int) -> io::Result<()> {
    // SAFETY: integer-argument ioctl on an fd we own.
    if unsafe { libc::ioctl(file.as_raw_fd(), request, arg) } < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
}

fn write_events(file: &File, events: &[(u16, u16, i32)]) {
    let mut bytes = Vec::with_capacity(events.len() * size_of::<libc::input_event>());
    for &(kind, code, value) in events {
        let ev = libc::input_event { time: libc::timeval { tv_sec: 0, tv_usec: 0 }, type_: kind, code, value };
        // SAFETY: input_event is plain old data.
        let raw = unsafe { std::slice::from_raw_parts((&ev as *const libc::input_event).cast::<u8>(), size_of::<libc::input_event>()) };
        bytes.extend_from_slice(raw);
    }
    (&*file).write_all(&bytes).expect("writing input events");
}

/// A fake USB keyboard with a numpad, LEDs and auto-repeat.
pub struct FakeKeyboard {
    file: File,
    pub name: String,
}

impl FakeKeyboard {
    pub fn create(name: &str) -> FakeKeyboard {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open("/dev/uinput")
            .expect("this test needs read/write access to /dev/uinput");
        for kind in [EV_KEY, EV_MSC, EV_LED, EV_REP] {
            ioctl_int(&file, UI_SET_EVBIT, kind.into()).expect("UI_SET_EVBIT");
        }
        for code in 1..=248 {
            ioctl_int(&file, UI_SET_KEYBIT, code).expect("UI_SET_KEYBIT");
        }
        ioctl_int(&file, UI_SET_MSCBIT, MSC_SCAN.into()).expect("UI_SET_MSCBIT");
        for led in 0..3 {
            ioctl_int(&file, UI_SET_LEDBIT, led).expect("UI_SET_LEDBIT");
        }
        let mut setup = libc::uinput_setup {
            id: libc::input_id { bustype: 0x03, vendor: 0x1209, product: 0x5351, version: 1 },
            name: [0; libc::UINPUT_MAX_NAME_SIZE],
            ff_effects_max: 0,
        };
        for (dst, src) in setup.name.iter_mut().zip(name.bytes().take(libc::UINPUT_MAX_NAME_SIZE - 1)) {
            *dst = src as libc::c_char;
        }
        // SAFETY: valid uinput_setup matching the request size; argument-less create.
        unsafe {
            assert!(libc::ioctl(file.as_raw_fd(), UI_DEV_SETUP, &setup) >= 0, "UI_DEV_SETUP");
            assert!(libc::ioctl(file.as_raw_fd(), UI_DEV_CREATE) >= 0, "UI_DEV_CREATE");
        }
        FakeKeyboard { file, name: name.to_string() }
    }

    /// Framed like a USB keyboard: MSC_SCAN, EV_KEY, SYN_REPORT.
    pub fn key(&self, code: u16, value: i32) {
        write_events(&self.file, &[(EV_MSC, MSC_SCAN, 0x70000 + i32::from(code)), (EV_KEY, code, value), (EV_SYN, 0, 0)]);
    }

    pub fn tap(&self, code: u16) {
        self.key(code, 1);
        self.key(code, 0);
    }
}

impl Drop for FakeKeyboard {
    fn drop(&mut self) {
        // SAFETY: argument-less ioctl on our uinput fd.
        unsafe { libc::ioctl(self.file.as_raw_fd(), UI_DEV_DESTROY) };
    }
}

/// An evdev node EVIOCGRAB'ed by the test: its events reach only us.
pub struct Grabbed {
    file: File,
}

impl Grabbed {
    pub fn open(path: &Path) -> Grabbed {
        let deadline = Instant::now() + SETTLE;
        let file = loop {
            match OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path) {
                Ok(f) => break f,
                // udev may not have applied the group permissions yet.
                Err(e) if e.kind() == io::ErrorKind::PermissionDenied && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => panic!("cannot open {}: {e}", path.display()),
            }
        };
        ioctl_int(&file, EVIOCGRAB, 1).unwrap_or_else(|e| panic!("EVIOCGRAB {} failed: {e}; refusing to inject anything", path.display()));
        Grabbed { file }
    }

    /// Key events (code, value) arriving within `window`, ignoring auto-repeat.
    pub fn keys(&self, window: Duration) -> Vec<(u16, i32)> {
        let deadline = Instant::now() + window;
        let mut keys = Vec::new();
        let mut buf = [0u8; 64 * size_of::<libc::input_event>()];
        while Instant::now() < deadline {
            match (&self.file).read(&mut buf) {
                Ok(n) => {
                    for chunk in buf[..n].chunks_exact(size_of::<libc::input_event>()) {
                        // SAFETY: one input_event per chunk; read_unaligned copes with the byte buffer.
                        let ev: libc::input_event = unsafe { std::ptr::read_unaligned(chunk.as_ptr().cast()) };
                        if ev.type_ == EV_KEY && ev.value != 2 {
                            keys.push((ev.code, ev.value));
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(5)),
                Err(e) => panic!("read: {e}"),
            }
        }
        keys
    }
}

fn sysfs_name(node: &Path) -> Option<String> {
    let path = Path::new("/sys/class/input").join(node.file_name()?).join("device/name");
    std::fs::read_to_string(path).ok().map(|s| s.trim_end().to_string())
}

/// Event nodes of every input device called `name`.
pub fn nodes_named(name: &str) -> HashSet<PathBuf> {
    std::fs::read_dir("/dev/input")
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.file_name().is_some_and(|f| f.to_string_lossy().starts_with("event")))
                .filter(|p| sysfs_name(p).as_deref() == Some(name))
                .collect()
        })
        .unwrap_or_default()
}

/// The one node called `name` that is not in `before` (devices other daemons created are left alone).
pub fn new_node_named(name: &str, before: &HashSet<PathBuf>) -> PathBuf {
    let deadline = Instant::now() + SETTLE;
    loop {
        let fresh: Vec<PathBuf> = nodes_named(name).difference(before).cloned().collect();
        match fresh.as_slice() {
            [one] => return one.clone(),
            [] if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            [] => panic!("no new input device named {name:?}"),
            many => panic!("several new devices named {name:?} ({many:?}); refusing to guess which one is ours"),
        }
    }
}
