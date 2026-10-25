//! uinput virtual devices: the per-keyboard mirrors and the synthetic output keyboard.

use crate::caps::Capabilities;
use crate::sys::{self, EV_ABS, EV_KEY, EV_LED, EV_MSC, EV_REL, EV_REP, EV_SW, InputEvent, UinputBit};
use std::ffi::{CString, c_char};
use std::fs::{self, OpenOptions};
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const UINPUT_PATH: &str = "/dev/uinput";

pub(crate) struct VirtualSpec<'a> {
    pub name: &'a str,
    pub id: libc::input_id,
    pub phys: &'a str,
    pub caps: &'a Capabilities,
}

/// A live uinput device; destroyed when dropped.
pub(crate) struct VirtualDevice {
    fd: OwnedFd,
}

impl VirtualDevice {
    /// Creates the device with exactly the capabilities in `spec.caps`. Force feedback and sound are never
    /// advertised: uinput would have to service FF uploads, and clients block on them otherwise.
    pub fn create(spec: &VirtualSpec<'_>) -> io::Result<VirtualDevice> {
        let file = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK).open(UINPUT_PATH)?;
        let device = VirtualDevice { fd: file.into() };
        device.configure(spec)?;
        sys::ui_dev_create(device.fd.as_fd())?;
        Ok(device)
    }

    fn configure(&self, spec: &VirtualSpec<'_>) -> io::Result<()> {
        let fd = self.fd.as_fd();
        let caps = spec.caps;
        let typed = [
            (EV_KEY, UinputBit::Key, &caps.keys),
            (EV_REL, UinputBit::Rel, &caps.rel),
            (EV_MSC, UinputBit::Msc, &caps.msc),
            (EV_LED, UinputBit::Led, &caps.led),
            (EV_SW, UinputBit::Switch, &caps.sw),
        ];
        for (kind, which, codes) in typed {
            if caps.has_type(kind) {
                sys::ui_set_bit(fd, UinputBit::Event, kind)?;
                codes.iter().try_for_each(|code| sys::ui_set_bit(fd, which, code))?;
            }
        }
        if caps.has_type(EV_ABS) {
            sys::ui_set_bit(fd, UinputBit::Event, EV_ABS)?;
            for axis in &caps.abs {
                sys::ui_set_bit(fd, UinputBit::Abs, axis.code)?;
                sys::ui_abs_setup(fd, &libc::uinput_abs_setup { code: axis.code, absinfo: axis.info })?;
            }
        }
        if caps.has_type(EV_REP) {
            sys::ui_set_bit(fd, UinputBit::Event, EV_REP)?;
        }
        caps.props.iter().try_for_each(|prop| sys::ui_set_bit(fd, UinputBit::Property, prop))?;

        let phys = CString::new(spec.phys).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        sys::ui_set_phys(fd, &phys)?;
        sys::ui_dev_setup(fd, &libc::uinput_setup { id: spec.id, name: c_name(spec.name), ff_effects_max: 0 })
    }

    pub fn write_events(&self, events: &[InputEvent]) -> io::Result<()> {
        sys::write_events(self.fd.as_fd(), events)
    }

    /// Reads events written *to* the device by its clients (LEDs, repeat settings).
    pub fn read_events(&self, buf: &mut [InputEvent]) -> io::Result<usize> {
        sys::read_events(self.fd.as_fd(), buf)
    }

    /// The device's `/dev/input/eventN` node.
    pub fn devnode(&self) -> Option<PathBuf> {
        let sysname = sys::ui_sysname(self.fd.as_fd()).ok()?;
        let entries = fs::read_dir(Path::new("/sys/devices/virtual/input").join(sysname)).ok()?;
        entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .find(|name| name.to_string_lossy().starts_with("event"))
            .map(|name| Path::new("/dev/input").join(name))
    }
}

impl AsFd for VirtualDevice {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl Drop for VirtualDevice {
    fn drop(&mut self) {
        // Closing the fd destroys the device too; destroying explicitly first makes it synchronous.
        let _ = sys::ui_dev_destroy(self.fd.as_fd());
    }
}

/// `name` as a NUL-terminated `uinput_setup.name`, truncated on a character boundary.
fn c_name(name: &str) -> [c_char; libc::UINPUT_MAX_NAME_SIZE] {
    let mut out = [0 as c_char; libc::UINPUT_MAX_NAME_SIZE];
    let mut len = name.len().min(libc::UINPUT_MAX_NAME_SIZE - 1);
    while !name.is_char_boundary(len) {
        len -= 1;
    }
    for (dst, src) in out.iter_mut().zip(name[..len].bytes().filter(|b| *b != 0)) {
        *dst = src as c_char;
    }
    out
}

/// Explains the two common ways opening `/dev/uinput` fails.
pub(crate) fn describe_open_error(err: &io::Error) -> String {
    match err.kind() {
        io::ErrorKind::PermissionDenied => format!(
            "cannot open {UINPUT_PATH}: permission denied (install the StreamPad udev rule or add your user to the \"input\" group, then log in again)"
        ),
        io::ErrorKind::NotFound => {
            format!("{UINPUT_PATH} does not exist (load the uinput kernel module: modprobe uinput)")
        }
        _ => format!("cannot create a uinput device: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(raw: &[c_char]) -> String {
        let bytes: Vec<u8> = raw.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
        String::from_utf8(bytes).expect("truncation must keep UTF-8 valid")
    }

    #[test]
    fn names_are_truncated_on_char_boundaries() {
        assert_eq!(decode(&c_name("StreamPad: Razer Cynosa")), "StreamPad: Razer Cynosa");
        let long = format!("StreamPad: {}", "é".repeat(60));
        let name = decode(&c_name(&long));
        assert!(name.len() < libc::UINPUT_MAX_NAME_SIZE && long.starts_with(&name));
        assert_eq!(c_name(&long)[libc::UINPUT_MAX_NAME_SIZE - 1], 0);
    }
}
