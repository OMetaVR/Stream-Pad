//! The mirror: a uinput clone of a grabbed keyboard that carries its events on to the compositor.
//!
//! Identity: the mirror copies bus type, vendor, product and version from the source. libinput quirks and
//! udev hwdb entries match on exactly those fields, and libinput decides from the bus type whether a keyboard
//! is internal (which pairs it with the touchpad for disable-while-typing on laptops) — so the mirror is
//! classified and configured exactly like the physical keyboard. A virtual bus type would silently lose all
//! of that. The name is `"StreamPad: <original name>"` (our prefix, so the mirror itself is never grabbed) and
//! phys gets a "streampad/" prefix so tools that group devices by phys keep the two apart.

use crate::caps::Capabilities;
use crate::sys::{EV_REP, InputEvent, REP_DELAY, REP_PERIOD};
use crate::uinput::{VirtualDevice, VirtualSpec};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const MIRROR_NAME_PREFIX: &str = "StreamPad: ";

pub(crate) fn create(
    name: &str,
    id: libc::input_id,
    phys: Option<&str>,
    caps: &Capabilities,
) -> io::Result<VirtualDevice> {
    let name = format!("{MIRROR_NAME_PREFIX}{name}");
    let phys = format!("streampad/{}", phys.unwrap_or("mirror"));
    let mirror = VirtualDevice::create(&VirtualSpec { name: &name, id, phys: &phys, caps })?;
    if caps.has_type(EV_REP) {
        // The source's own auto-repeat events are forwarded as they are; the kernel's software repeat on
        // the mirror would double them.
        mirror.write_events(&software_repeat_off())?;
    }
    Ok(mirror)
}

pub(crate) fn software_repeat_off() -> [InputEvent; 3] {
    [InputEvent::new(EV_REP, REP_DELAY, 0), InputEvent::new(EV_REP, REP_PERIOD, 0), InputEvent::syn_report()]
}

const UDEV_DATABASE: &str = "/run/udev/data";
/// The compositor learns about a new device from udev, which needs ~100–150 ms (more on a busy system).
const SETTLE_MIN: Duration = Duration::from_millis(250);
/// udev writes its database entry slightly before announcing the device; then the compositor opens it.
const UDEV_TO_COMPOSITOR: Duration = Duration::from_millis(150);
const SETTLE_MAX: Duration = Duration::from_secs(2);

/// Tracks whether the compositor has had time to open a new mirror. Grabbing the source earlier would
/// lose the first keystrokes (events written to a device nobody has opened go nowhere).
pub(crate) struct Settle {
    created: Instant,
    /// udev's database entry for the mirror's event node; `None` without udev.
    udev_entry: Option<PathBuf>,
    processed: Option<Instant>,
}

impl Settle {
    pub fn new(mirror: &VirtualDevice) -> Settle {
        let udev_entry = Path::new(UDEV_DATABASE).is_dir().then(|| udev_entry(mirror)).flatten();
        Settle { created: Instant::now(), udev_entry, processed: None }
    }

    pub fn settled(&mut self) -> bool {
        let age = self.created.elapsed();
        let Some(entry) = &self.udev_entry else { return age >= SETTLE_MIN };
        if self.processed.is_none() && entry.exists() {
            self.processed = Some(Instant::now());
        }
        let announced = self.processed.is_some_and(|at| at.elapsed() >= UDEV_TO_COMPOSITOR);
        age >= SETTLE_MAX || (age >= SETTLE_MIN && announced)
    }
}

/// `/run/udev/data/c<major>:<minor>` for the mirror's event node.
fn udev_entry(mirror: &VirtualDevice) -> Option<PathBuf> {
    let node = mirror.devnode()?;
    let dev = fs::read_to_string(Path::new("/sys/class/input").join(node.file_name()?).join("dev")).ok()?;
    Some(Path::new(UDEV_DATABASE).join(format!("c{}", dev.trim())))
}
