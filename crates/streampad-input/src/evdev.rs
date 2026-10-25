//! A physical (source) input device, opened through its `/dev/input/eventN` node.

use crate::bits::Bits;
use crate::caps::Capabilities;
use crate::sys::{self, InputEvent};
use std::fs::OpenOptions;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub(crate) struct EvdevDevice {
    fd: OwnedFd,
    writable: bool,
}

impl EvdevDevice {
    /// Opens non-blocking, read-write when permitted (needed to set LEDs), else read-only.
    pub fn open(path: &Path) -> io::Result<EvdevDevice> {
        let open = |write: bool| OpenOptions::new().read(true).write(write).custom_flags(libc::O_NONBLOCK).open(path);
        let (file, writable) = match open(true) {
            Ok(file) => (file, true),
            Err(err) if err.kind() == io::ErrorKind::PermissionDenied => (open(false)?, false),
            Err(err) => return Err(err),
        };
        Ok(EvdevDevice { fd: file.into(), writable })
    }

    pub fn writable(&self) -> bool {
        self.writable
    }

    pub fn name(&self) -> io::Result<String> {
        sys::device_name(self.fd.as_fd())
    }

    /// Physical location ("usb-0000:0a:00.3-2/input0"); `None` when the driver doesn't report one.
    pub fn phys(&self) -> Option<String> {
        sys::device_phys(self.fd.as_fd()).ok().filter(|phys| !phys.is_empty())
    }

    pub fn input_id(&self) -> io::Result<libc::input_id> {
        sys::device_id(self.fd.as_fd())
    }

    pub fn capabilities(&self) -> io::Result<Capabilities> {
        Capabilities::query(self.fd.as_fd())
    }

    /// Keys currently held down on the device.
    pub fn pressed_keys(&self) -> io::Result<Bits> {
        let mut keys = Bits::new();
        sys::key_state(self.fd.as_fd(), keys.as_mut_bytes())?;
        Ok(keys)
    }

    pub fn leds(&self) -> io::Result<Bits> {
        let mut leds = Bits::new();
        sys::led_state(self.fd.as_fd(), leds.as_mut_bytes())?;
        Ok(leds)
    }

    pub fn set_grab(&self, grab: bool) -> io::Result<()> {
        sys::set_grab(self.fd.as_fd(), grab)
    }

    pub fn use_monotonic_clock(&self) -> io::Result<()> {
        sys::set_monotonic_clock(self.fd.as_fd())
    }

    pub fn read_events(&self, buf: &mut [InputEvent]) -> io::Result<usize> {
        sys::read_events(self.fd.as_fd(), buf)
    }

    /// Writes output events (LEDs, repeat settings) to the device.
    pub fn write_events(&self, events: &[InputEvent]) -> io::Result<()> {
        sys::write_events(self.fd.as_fd(), events)
    }
}

impl AsFd for EvdevDevice {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// The device node is gone (unplugged): `ENODEV` from an open fd, `ENOENT` when opening.
pub(crate) fn is_vanished(err: &io::Error) -> bool {
    matches!(err.raw_os_error(), Some(libc::ENODEV | libc::ENOENT | libc::ENXIO))
}
