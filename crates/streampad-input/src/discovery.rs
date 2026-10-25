//! Finding keyboards: enumerating `/dev/input`, identifying devices and applying the [`DeviceFilter`].

use crate::DeviceFilter;
use crate::caps::Capabilities;
use crate::evdev::{EvdevDevice, is_vanished};
use crate::sysfs::{self, SysfsDevice};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use streampad_core::ipc::DeviceInfo;

pub(crate) const INPUT_DIR: &str = "/dev/input";

/// Every device StreamPad creates has a name starting with this; such devices are never grabbed.
pub const OWN_DEVICE_PREFIX: &str = "StreamPad";

/// A keyboard-like device that could be opened.
pub(crate) struct Keyboard {
    pub info: DeviceInfo,
    pub device: EvdevDevice,
    pub caps: Capabilities,
    pub input_id: libc::input_id,
}

pub(crate) enum Probe {
    Keyboard(Box<Keyboard>),
    /// The node exists but can't be opened (usually permissions — possibly udev hasn't applied them
    /// yet). `info` comes from sysfs; `None` when sysfs couldn't describe it either.
    Inaccessible {
        info: Option<DeviceInfo>,
        error: io::Error,
    },
    /// Not a keyboard, one of our own devices, or gone.
    Ignored,
}

/// `/dev/input/event*`, in numeric order.
pub(crate) fn event_nodes() -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(INPUT_DIR) else {
        return Vec::new();
    };
    let mut nodes: Vec<(u32, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| Some((event_number(&entry.file_name().to_string_lossy())?, entry.path())))
        .collect();
    nodes.sort_unstable();
    nodes.into_iter().map(|(_, path)| path).collect()
}

/// `"event12"` → 12.
pub(crate) fn event_number(file_name: &str) -> Option<u32> {
    file_name.strip_prefix("event")?.parse().ok()
}

pub(crate) fn device_id(vendor: u16, product: u16, name: &str) -> String {
    format!("{vendor:04x}:{product:04x}:{name}")
}

pub(crate) fn probe(path: &Path, toggle_key: u16) -> Probe {
    let summary = sysfs::read(path);
    if let Some(summary) = &summary
        && (summary.name.starts_with(OWN_DEVICE_PREFIX) || !summary.caps.is_keyboard_like(toggle_key))
    {
        return Probe::Ignored;
    }
    let device = match EvdevDevice::open(path) {
        Ok(device) => device,
        Err(err) if is_vanished(&err) => return Probe::Ignored,
        Err(error) => return Probe::Inaccessible { info: summary.map(|s| sysfs_info(&s, path, toggle_key)), error },
    };
    let (Ok(name), Ok(input_id), Ok(caps)) = (device.name(), device.input_id(), device.capabilities()) else {
        return Probe::Ignored;
    };
    if name.starts_with(OWN_DEVICE_PREFIX) || !caps.is_keyboard_like(toggle_key) {
        return Probe::Ignored;
    }
    let info = DeviceInfo {
        id: device_id(input_id.vendor, input_id.product, &name),
        name,
        path: path.to_string_lossy().into_owned(),
        phys: device.phys(),
        has_numpad: caps.has_numpad(),
        eligible: caps.is_eligible(toggle_key),
        grabbed: false,
        error: None,
    };
    Probe::Keyboard(Box::new(Keyboard { info, device, caps, input_id }))
}

fn sysfs_info(summary: &SysfsDevice, path: &Path, toggle_key: u16) -> DeviceInfo {
    DeviceInfo {
        id: device_id(summary.vendor, summary.product, &summary.name),
        name: summary.name.clone(),
        path: path.to_string_lossy().into_owned(),
        phys: summary.phys.clone(),
        has_numpad: summary.caps.has_numpad(),
        eligible: summary.caps.is_eligible(toggle_key),
        grabbed: false,
        error: None,
    }
}

pub(crate) fn describe_open_error(err: &io::Error) -> String {
    if err.kind() == io::ErrorKind::PermissionDenied {
        "permission denied (add your user to the \"input\" group and log in again)".to_string()
    } else {
        format!("cannot open: {err}")
    }
}

impl DeviceFilter {
    /// `only` restricts which devices are looked at at all.
    pub(crate) fn considers(&self, info: &DeviceInfo) -> bool {
        self.only.as_deref().is_none_or(|only| only == info.id || only == info.path || only == info.name)
    }

    /// Whether the device should be grabbed.
    pub(crate) fn selects(&self, info: &DeviceInfo) -> bool {
        let wanted = if self.only.is_some() {
            true
        } else if self.auto {
            !self.exclude.contains(&info.id)
        } else {
            self.include.contains(&info.id)
        };
        info.eligible && wanted && self.considers(info)
    }
}

pub(crate) fn list_devices(filter: &DeviceFilter, toggle_key: u16) -> Vec<DeviceInfo> {
    event_nodes()
        .iter()
        .filter_map(|path| match probe(path, toggle_key) {
            Probe::Keyboard(keyboard) => Some(keyboard.info),
            Probe::Inaccessible { info: Some(info), error } => {
                Some(DeviceInfo { error: Some(describe_open_error(&error)), ..info })
            }
            Probe::Inaccessible { info: None, .. } | Probe::Ignored => None,
        })
        .filter(|info| filter.considers(info))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(name: &str, path: &str, eligible: bool) -> DeviceInfo {
        DeviceInfo {
            id: device_id(0x1532, 0x023f, name),
            name: name.into(),
            path: path.into(),
            phys: None,
            has_numpad: true,
            eligible,
            grabbed: false,
            error: None,
        }
    }

    #[test]
    fn ids_use_lowercase_hex4() {
        assert_eq!(device_id(0x1532, 0x23f, "Razer Razer Cynosa Lite"), "1532:023f:Razer Razer Cynosa Lite");
        assert_eq!(event_number("event12"), Some(12));
        assert_eq!(event_number("mouse0"), None);
        assert_eq!(event_number("event"), None);
    }

    #[test]
    fn auto_mode_grabs_eligible_devices_except_excluded() {
        let kb = info("Keyboard", "/dev/input/event3", true);
        let mut filter = DeviceFilter { auto: true, ..Default::default() };
        assert!(filter.selects(&kb));
        assert!(!filter.selects(&info("Keyboard", "/dev/input/event3", false)), "never grab ineligible devices");
        filter.exclude.push(kb.id.clone());
        assert!(!filter.selects(&kb) && filter.considers(&kb));
    }

    #[test]
    fn manual_mode_grabs_only_included_devices() {
        let kb = info("Keyboard", "/dev/input/event3", true);
        let mut filter = DeviceFilter { auto: false, ..Default::default() };
        assert!(!filter.selects(&kb));
        filter.include.push(kb.id.clone());
        assert!(filter.selects(&kb));
    }

    #[test]
    fn only_matches_id_path_or_name_and_overrides_the_rest() {
        let kb = info("SPTEST Keyboard", "/dev/input/event30", true);
        for only in [kb.id.clone(), kb.path.clone(), kb.name.clone()] {
            let filter =
                DeviceFilter { auto: false, exclude: vec![kb.id.clone()], only: Some(only), ..Default::default() };
            assert!(filter.considers(&kb) && filter.selects(&kb));
        }
        let other = DeviceFilter { auto: true, only: Some("something else".into()), ..Default::default() };
        assert!(!other.considers(&kb) && !other.selects(&kb));
    }
}
