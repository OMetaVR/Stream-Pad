//! Kernel input ABI: event codes, the `input_event` record and the evdev / uinput ioctls the engine uses.
//! Every `unsafe` block of the crate lives here, behind safe wrappers taking borrowed fds.

use nix::errno::Errno;
use nix::{request_code_none, request_code_read, request_code_write};
use std::ffi::{CStr, CString, c_int, c_long, c_void};
use std::io;
use std::mem::{size_of, size_of_val};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::Duration;

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_REL: u16 = 0x02;
pub const EV_ABS: u16 = 0x03;
pub const EV_MSC: u16 = 0x04;
pub const EV_SW: u16 = 0x05;
pub const EV_LED: u16 = 0x11;
pub const EV_SND: u16 = 0x12;
pub const EV_REP: u16 = 0x14;
pub const EV_FF: u16 = 0x15;
pub const EV_PWR: u16 = 0x16;
pub const EV_FF_STATUS: u16 = 0x17;

pub const SYN_REPORT: u16 = 0;
pub const SYN_DROPPED: u16 = 3;
pub const MSC_SCAN: u16 = 4;
pub const LED_SCROLLL: u16 = 2;
pub const REP_DELAY: u16 = 0;
pub const REP_PERIOD: u16 = 1;
pub const KEY_MAX: u16 = 0x2ff;

pub const BUS_VIRTUAL: u16 = 0x06;

/// `struct input_event` as read from evdev nodes and written to uinput (the timestamp is ignored on write).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub sec: c_long,
    pub usec: c_long,
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

const _: () = assert!(size_of::<InputEvent>() == size_of::<libc::input_event>());

impl InputEvent {
    pub const fn new(kind: u16, code: u16, value: i32) -> InputEvent {
        InputEvent { sec: 0, usec: 0, kind, code, value }
    }

    pub const fn syn_report() -> InputEvent {
        InputEvent::new(EV_SYN, SYN_REPORT, 0)
    }

    pub fn is(&self, kind: u16, code: u16) -> bool {
        self.kind == kind && self.code == code
    }

    /// Kernel timestamp (CLOCK_MONOTONIC once [`set_monotonic_clock`] was applied to the fd).
    pub fn time(&self) -> Duration {
        Duration::new(self.sec.max(0) as u64, (self.usec.clamp(0, 999_999) as u32) * 1000)
    }
}

/// Current CLOCK_MONOTONIC time, comparable with event timestamps.
pub fn monotonic_now() -> Duration {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid out-pointer; CLOCK_MONOTONIC always exists on Linux.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.clamp(0, 999_999_999) as u32)
}

/// `poll(2)` timeout for `wait`, rounded up to whole milliseconds so a wait never ends early.
pub fn poll_timeout(wait: Duration) -> nix::poll::PollTimeout {
    let millis = wait.as_micros().div_ceil(1000).min(i32::MAX as u128);
    nix::poll::PollTimeout::try_from(millis).unwrap_or(nix::poll::PollTimeout::MAX)
}

fn check(ret: c_int) -> io::Result<c_int> {
    Errno::result(ret).map_err(io::Error::from)
}

/// Reads as many events as fit into `buf`; returns how many were read (`WouldBlock` when none are queued).
pub fn read_events(fd: BorrowedFd<'_>, buf: &mut [InputEvent]) -> io::Result<usize> {
    // SAFETY: `buf` is valid for `size_of_val(buf)` bytes and `InputEvent` is plain old data.
    let n = unsafe { libc::read(fd.as_raw_fd(), buf.as_mut_ptr().cast::<c_void>(), size_of_val(buf)) };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(n as usize / size_of::<InputEvent>())
}

pub fn write_events(fd: BorrowedFd<'_>, events: &[InputEvent]) -> io::Result<()> {
    let total = size_of_val(events);
    let base = events.as_ptr().cast::<u8>();
    let mut written = 0;
    while written < total {
        // SAFETY: `base + written .. base + total` lies inside `events`.
        let n = unsafe { libc::write(fd.as_raw_fd(), base.add(written).cast::<c_void>(), total - written) };
        if n < 0 {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err);
        }
        if n == 0 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        written += n as usize;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------
// evdev

/// Variable-length evdev query (`EVIOCGNAME`, `EVIOCGBIT`, `EVIOCGKEY`, …) into `buf`.
fn evdev_query(fd: BorrowedFd<'_>, nr: u16, buf: &mut [u8]) -> io::Result<usize> {
    let request = request_code_read!(b'E', nr, buf.len());
    // SAFETY: the request encodes `buf.len()`, so the kernel writes at most that many bytes into `buf`.
    let n = check(unsafe { libc::ioctl(fd.as_raw_fd(), request, buf.as_mut_ptr()) })?;
    Ok(n as usize)
}

fn evdev_string(fd: BorrowedFd<'_>, nr: u16) -> io::Result<String> {
    let mut buf = [0u8; 256];
    let n = evdev_query(fd, nr, &mut buf)?.min(buf.len());
    let bytes = &buf[..n];
    let text = CStr::from_bytes_until_nul(bytes).map(CStr::to_bytes).unwrap_or(bytes);
    Ok(String::from_utf8_lossy(text).into_owned())
}

pub fn device_name(fd: BorrowedFd<'_>) -> io::Result<String> {
    evdev_string(fd, 0x06)
}

pub fn device_phys(fd: BorrowedFd<'_>) -> io::Result<String> {
    evdev_string(fd, 0x07)
}

pub fn device_id(fd: BorrowedFd<'_>) -> io::Result<libc::input_id> {
    let mut id = libc::input_id { bustype: 0, vendor: 0, product: 0, version: 0 };
    let request = request_code_read!(b'E', 0x02, size_of::<libc::input_id>());
    // SAFETY: `id` is a valid `input_id` out-pointer matching the request size.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, &mut id) })?;
    Ok(id)
}

/// `EVIOCGBIT(kind)`: supported codes of an event type; `kind == 0` yields the supported event types.
pub fn event_bits(fd: BorrowedFd<'_>, kind: u16, buf: &mut [u8]) -> io::Result<()> {
    evdev_query(fd, 0x20 + kind, buf).map(drop)
}

pub fn property_bits(fd: BorrowedFd<'_>, buf: &mut [u8]) -> io::Result<()> {
    evdev_query(fd, 0x09, buf).map(drop)
}

pub fn key_state(fd: BorrowedFd<'_>, buf: &mut [u8]) -> io::Result<()> {
    evdev_query(fd, 0x18, buf).map(drop)
}

pub fn led_state(fd: BorrowedFd<'_>, buf: &mut [u8]) -> io::Result<()> {
    evdev_query(fd, 0x19, buf).map(drop)
}

pub fn abs_info(fd: BorrowedFd<'_>, axis: u16) -> io::Result<libc::input_absinfo> {
    let mut info = libc::input_absinfo { value: 0, minimum: 0, maximum: 0, fuzz: 0, flat: 0, resolution: 0 };
    let request = request_code_read!(b'E', 0x40 + axis, size_of::<libc::input_absinfo>());
    // SAFETY: `info` is a valid `input_absinfo` out-pointer matching the request size.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, &mut info) })?;
    Ok(info)
}

/// `EVIOCGRAB`: while grabbed, events reach only this fd. Closing the fd releases the grab.
pub fn set_grab(fd: BorrowedFd<'_>, grab: bool) -> io::Result<()> {
    let request = request_code_write!(b'E', 0x90, size_of::<c_int>());
    // SAFETY: EVIOCGRAB takes its argument by value.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, c_int::from(grab)) }).map(drop)
}

/// `EVIOCSCLOCKID(CLOCK_MONOTONIC)`: timestamp this fd's events with the monotonic clock.
pub fn set_monotonic_clock(fd: BorrowedFd<'_>) -> io::Result<()> {
    let clock: c_int = libc::CLOCK_MONOTONIC;
    let request = request_code_write!(b'E', 0xa0, size_of::<c_int>());
    // SAFETY: EVIOCSCLOCKID reads one int through the pointer.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, &clock) }).map(drop)
}

// ---------------------------------------------------------------------------------------------------------
// uinput

/// `UI_SET_*BIT` ioctl numbers.
#[derive(Debug, Clone, Copy)]
pub enum UinputBit {
    Event = 100,
    Key = 101,
    Rel = 102,
    Abs = 103,
    Msc = 104,
    Led = 105,
    Switch = 109,
    Property = 110,
}

pub fn ui_set_bit(fd: BorrowedFd<'_>, which: UinputBit, bit: u16) -> io::Result<()> {
    let request = request_code_write!(b'U', which as u8, size_of::<c_int>());
    // SAFETY: UI_SET_*BIT take their argument by value.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, c_int::from(bit)) }).map(drop)
}

pub fn ui_set_phys(fd: BorrowedFd<'_>, phys: &CString) -> io::Result<()> {
    let request = request_code_write!(b'U', 108, size_of::<*const libc::c_char>());
    // SAFETY: UI_SET_PHYS reads a NUL-terminated string through the pointer; `phys` outlives the call.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, phys.as_ptr()) }).map(drop)
}

pub fn ui_abs_setup(fd: BorrowedFd<'_>, setup: &libc::uinput_abs_setup) -> io::Result<()> {
    let request = request_code_write!(b'U', 4, size_of::<libc::uinput_abs_setup>());
    // SAFETY: `setup` is a valid `uinput_abs_setup` matching the request size.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, setup as *const libc::uinput_abs_setup) }).map(drop)
}

pub fn ui_dev_setup(fd: BorrowedFd<'_>, setup: &libc::uinput_setup) -> io::Result<()> {
    let request = request_code_write!(b'U', 3, size_of::<libc::uinput_setup>());
    // SAFETY: `setup` is a valid `uinput_setup` matching the request size.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, setup as *const libc::uinput_setup) }).map(drop)
}

pub fn ui_dev_create(fd: BorrowedFd<'_>) -> io::Result<()> {
    // SAFETY: UI_DEV_CREATE takes no argument.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request_code_none!(b'U', 1)) }).map(drop)
}

pub fn ui_dev_destroy(fd: BorrowedFd<'_>) -> io::Result<()> {
    // SAFETY: UI_DEV_DESTROY takes no argument.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request_code_none!(b'U', 2)) }).map(drop)
}

/// `UI_GET_SYSNAME`: the created device's sysfs name, e.g. `"input42"`.
pub fn ui_sysname(fd: BorrowedFd<'_>) -> io::Result<String> {
    let mut buf = [0u8; 64];
    let request = request_code_read!(b'U', 44, buf.len());
    // SAFETY: the request encodes `buf.len()`, so the kernel writes at most that many bytes into `buf`.
    check(unsafe { libc::ioctl(fd.as_raw_fd(), request, buf.as_mut_ptr()) })?;
    let name = CStr::from_bytes_until_nul(&buf).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    Ok(name.to_string_lossy().into_owned())
}
