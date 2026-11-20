//! Permission-free device identification from sysfs (`/sys/class/input/eventN/device`). Discovery uses it
//! to skip non-keyboards without opening them (opening can wake an autosuspended device) and to describe
//! keyboards it isn't allowed to open.

use crate::bits::Bits;
use crate::caps::Capabilities;
use crate::sys::BUS_VIRTUAL;
use std::fs;
use std::path::Path;

pub(crate) struct SysfsDevice {
    pub name: String,
    pub phys: Option<String>,
    pub vendor: u16,
    pub product: u16,
    pub is_virtual: bool,
    /// Event types and key codes only.
    pub caps: Capabilities,
}

/// True when a canonical `/sys/class/input/eventN/device` path belongs to `/dev/uinput`.
///
/// Only `/sys/devices/virtual/input` matches uinput; BLE HID keyboards live under
/// `/sys/devices/virtual/misc/uhid/...` and remain classified as physical devices.
pub(crate) fn is_virtual_sysfs_path(canonical: &Path) -> bool {
    canonical.starts_with("/sys/devices/virtual/input")
}

/// True when `/sys/class/input/<eventN>/device` resolves under `/sys/devices/virtual/input`.
pub(crate) fn is_virtual_node(node: &Path) -> bool {
    let Some(file_name) = node.file_name() else {
        return false;
    };
    let dir = Path::new("/sys/class/input").join(file_name).join("device");
    fs::canonicalize(&dir).ok().is_some_and(|path| is_virtual_sysfs_path(&path))
}

pub(crate) fn read(node: &Path) -> Option<SysfsDevice> {
    let dir = Path::new("/sys/class/input").join(node.file_name()?).join("device");
    let read = |file: &str| fs::read_to_string(dir.join(file)).ok().map(|s| s.trim_end_matches('\n').to_string());
    let hex = |file: &str| read(file).and_then(|s| u16::from_str_radix(s.trim(), 16).ok());
    let caps = Capabilities {
        types: parse_bitmap(&read("capabilities/ev")?)?,
        keys: parse_bitmap(&read("capabilities/key")?)?,
        ..Capabilities::default()
    };
    let bustype = hex("id/bustype")?;
    let is_virtual =
        bustype == BUS_VIRTUAL || fs::canonicalize(&dir).ok().is_some_and(|path| is_virtual_sysfs_path(&path));
    Some(SysfsDevice {
        name: read("name")?,
        phys: read("phys").filter(|phys| !phys.is_empty()),
        vendor: hex("id/vendor")?,
        product: hex("id/product")?,
        is_virtual,
        caps,
    })
}

/// Parses a sysfs capability bitmap: hex words of the kernel's `long` width, most significant first.
fn parse_bitmap(text: &str) -> Option<Bits> {
    let mut bits = Bits::new();
    for (index, word) in text.split_whitespace().rev().enumerate() {
        let value = u64::from_str_radix(word, 16).ok()?;
        for bit in (0..usize::BITS).filter(|bit| value >> bit & 1 == 1) {
            if let Ok(code) = u16::try_from(index as u64 * u64::from(usize::BITS) + u64::from(bit)) {
                bits.insert(code);
            }
        }
    }
    Some(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn parses_multi_word_bitmaps() {
        // A real keyboard's key bitmap: KEY_ESC.. in the lowest word, KP Enter (96) in the second one.
        let bits = parse_bitmap("1000000000007 ff9f207ac14057ff febeffdfffefffff fffffffffffffffe\n").expect("valid");
        assert!(bits.contains(1) && bits.contains(30) && bits.contains(70) && bits.contains(96));
        assert!(!bits.contains(0));
        assert!(bits.contains(3 * 64 + 48), "highest word is the most significant");
        assert_eq!(parse_bitmap("0").expect("valid").iter().count(), 0);
        assert!(parse_bitmap("zz").is_none());
    }

    #[test]
    fn distinguishes_uinput_from_physical_and_ble_uhid_paths() {
        assert!(is_virtual_sysfs_path(Path::new("/sys/devices/virtual/input/input29")));
        assert!(
            !is_virtual_sysfs_path(Path::new("/sys/devices/virtual/misc/uhid/0005:1234:5678.0001/input/input12")),
            "BLE HID keyboards via uhid are physical"
        );
        assert!(!is_virtual_sysfs_path(Path::new(
            "/sys/devices/pci0000:00/0000:00:01.3/usb1/1-2/1-2:1.0/0003:1532:023F.0001/input/input2"
        )));
        assert!(!is_virtual_sysfs_path(Path::new("/sys/devices/platform/i8042/serio0/input/input3")));
    }
}
