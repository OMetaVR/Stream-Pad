//! The manager thread: owns the device registry, follows hotplug (inotify on `/dev/input`), applies
//! reconfiguration, relays LED updates to the device threads and enforces the grab timeout.

use crate::caps::Capabilities;
use crate::discovery::{self, INPUT_DIR, Keyboard, Probe, describe_open_error};
use crate::shared::Shared;
use crate::sys;
use crate::worker::{Report, WAITING_FOR_RELEASE, WorkerHandle};
use crate::{DeviceFilter, EngineEvent, KeyOutput};
use crossbeam_channel::{Receiver, Sender};
use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use nix::sys::eventfd::{EfdFlags, EventFd};
use nix::sys::inotify::{AddWatchFlags, InitFlags, Inotify};
use std::collections::{HashMap, HashSet};
use std::io;
use std::os::fd::AsFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use streampad_core::ipc::DeviceInfo;

/// A device appearing produces a burst of node and attribute changes; they are handled together.
const HOTPLUG_SETTLE: Duration = Duration::from_millis(100);
/// How long a new node may stay unreadable (udev still applying permissions) before that is an error.
const PERMISSION_SETTLE: Duration = Duration::from_secs(3);
const PERMISSION_RETRY: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(crate) enum Command {
    Reconfigure(DeviceFilter),
    SyncLeds,
    Worker { serial: u64, report: Report },
    Shutdown,
}

/// Sends commands to the manager thread and wakes it.
#[derive(Clone)]
pub(crate) struct ManagerLink {
    commands: Sender<Command>,
    wake: Arc<EventFd>,
}

impl ManagerLink {
    pub fn new() -> io::Result<(ManagerLink, Receiver<Command>)> {
        let (commands, receiver) = crossbeam_channel::unbounded();
        let wake = Arc::new(EventFd::from_value_and_flags(0, EfdFlags::EFD_CLOEXEC | EfdFlags::EFD_NONBLOCK)?);
        Ok((ManagerLink { commands, wake }, receiver))
    }

    /// Silently does nothing once the manager has stopped.
    pub fn send(&self, command: Command) {
        if self.commands.send(command).is_ok() {
            let _ = self.wake.write(1);
        }
    }
}

struct Entry {
    info: DeviceInfo,
    /// Set while a device thread serves this device.
    running: Option<Running>,
}

struct Running {
    worker: WorkerHandle,
    caps: Capabilities,
}

pub(crate) struct Manager {
    shared: Arc<Shared>,
    link: ManagerLink,
    commands: Receiver<Command>,
    inotify: Option<Inotify>,
    filter: DeviceFilter,
    entries: Vec<Entry>,
    next_serial: u64,
    /// Devices whose grab failed, by node: (device id, error). Not retried until reconfigured.
    failures: HashMap<PathBuf, (String, String)>,
    /// Nodes created recently, which udev may not have given their final permissions yet.
    fresh: HashMap<PathBuf, Instant>,
    /// Unreadable nodes already warned about.
    warned: HashSet<PathBuf>,
    rescan_at: Option<Instant>,
    grab_deadline: Option<(Instant, Duration)>,
    output: KeyOutput,
}

impl Manager {
    pub fn new(
        shared: Arc<Shared>,
        link: ManagerLink,
        commands: Receiver<Command>,
        filter: DeviceFilter,
        grab_timeout: Option<Duration>,
        output: KeyOutput,
    ) -> Manager {
        let inotify = watch_input_dir()
            .inspect_err(|err| {
                let message = format!("cannot watch {INPUT_DIR} for new devices ({err}); hotplug is disabled");
                log::warn!("{message}");
                shared.emit(EngineEvent::Warning(message));
            })
            .ok();
        Manager {
            shared,
            link,
            commands,
            inotify,
            filter,
            entries: Vec::new(),
            next_serial: 0,
            failures: HashMap::new(),
            fresh: HashMap::new(),
            warned: HashSet::new(),
            rescan_at: None,
            grab_deadline: grab_timeout.map(|timeout| (Instant::now() + timeout, timeout)),
            output,
        }
    }

    pub fn run(mut self) {
        loop {
            let timeout = self
                .next_deadline()
                .map_or(PollTimeout::NONE, |at| sys::poll_timeout(at.saturating_duration_since(Instant::now())));
            let hotplug = match self.wait(timeout) {
                Ok(hotplug) => hotplug,
                Err(err) => return self.stop(format!("the input manager failed to poll: {err}")),
            };
            if hotplug {
                self.read_hotplug();
            }
            let _ = self.link.wake.read();
            while let Ok(command) = self.commands.try_recv() {
                if !self.handle(command) {
                    return;
                }
            }
            let now = Instant::now();
            if let Some((deadline, timeout)) = self.grab_deadline
                && now >= deadline
            {
                return self.stop(format!("grab timeout of {} s reached", timeout.as_secs_f64()));
            }
            if self.rescan_at.is_some_and(|at| now >= at) {
                self.rescan();
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        [self.rescan_at, self.grab_deadline.map(|(at, _)| at)].into_iter().flatten().min()
    }

    /// Blocks until a command or hotplug event arrives or `timeout` passes; returns whether inotify is ready.
    fn wait(&self, timeout: PollTimeout) -> nix::Result<bool> {
        let mut fds = vec![PollFd::new(self.link.wake.as_fd(), PollFlags::POLLIN)];
        if let Some(inotify) = &self.inotify {
            fds.push(PollFd::new(inotify.as_fd(), PollFlags::POLLIN));
        }
        loop {
            match poll(&mut fds, timeout) {
                Ok(_) => break,
                Err(Errno::EINTR) => continue,
                Err(err) => return Err(err),
            }
        }
        Ok(fds.get(1).and_then(PollFd::revents).is_some_and(|events| !events.is_empty()))
    }

    /// Returns false when the manager should exit.
    fn handle(&mut self, command: Command) -> bool {
        match command {
            Command::Reconfigure(filter) => self.reconfigure(filter),
            Command::SyncLeds => self.running().for_each(|running| running.worker.notify()),
            Command::Worker { serial, report } => self.on_report(serial, report),
            Command::Shutdown => return false,
        }
        true
    }

    fn running(&self) -> impl Iterator<Item = &Running> {
        self.entries.iter().filter_map(|entry| entry.running.as_ref())
    }

    fn read_hotplug(&mut self) {
        let Some(inotify) = &self.inotify else { return };
        let events = match inotify.read_events() {
            Ok(events) => events,
            Err(Errno::EAGAIN) => return,
            Err(err) => {
                log::warn!("reading hotplug events failed: {err}");
                return;
            }
        };
        let mut changed = false;
        for event in events {
            if event.mask.contains(AddWatchFlags::IN_Q_OVERFLOW) {
                changed = true;
                continue;
            }
            let Some(name) = event.name.as_ref().and_then(|name| name.to_str()) else { continue };
            if discovery::event_number(name).is_none() {
                continue;
            }
            changed = true;
            if event.mask.contains(AddWatchFlags::IN_CREATE) {
                self.fresh.insert(Path::new(INPUT_DIR).join(name), Instant::now());
            }
        }
        if changed {
            self.schedule_rescan(HOTPLUG_SETTLE);
        }
    }

    fn schedule_rescan(&mut self, delay: Duration) {
        let at = Instant::now() + delay;
        self.rescan_at = Some(self.rescan_at.map_or(at, |current| current.min(at)));
    }

    /// Re-reads `/dev/input`. Devices with a running thread are left alone (the thread notices removal
    /// itself); every other entry is rebuilt from a fresh probe.
    pub fn rescan(&mut self) {
        self.rescan_at = None;
        let toggle_key = self.shared.toggle_key();
        let now = Instant::now();
        self.fresh.retain(|_, created| now.duration_since(*created) < PERMISSION_SETTLE);
        let nodes = discovery::event_nodes();
        self.entries.retain(|entry| entry.running.is_some());
        self.failures.retain(|path, _| nodes.contains(path));
        self.warned.retain(|path| nodes.contains(path));

        let mut unreadable = Vec::new();
        for path in nodes {
            if self.entries.iter().any(|entry| Path::new(&entry.info.path) == path) {
                continue;
            }
            match discovery::probe(&path, toggle_key) {
                Probe::Keyboard(keyboard) => self.adopt(*keyboard),
                Probe::Inaccessible { .. } if self.fresh.contains_key(&path) => self.schedule_rescan(PERMISSION_RETRY),
                Probe::Inaccessible { info: Some(info), error } if self.filter.considers(&info) => {
                    if self.warned.insert(path.clone()) {
                        unreadable.push(format!("{} ({})", info.name, info.path));
                    }
                    self.entries.push(Entry {
                        info: DeviceInfo { error: Some(describe_open_error(&error)), ..info },
                        running: None,
                    });
                }
                Probe::Inaccessible { .. } | Probe::Ignored => {}
            }
        }
        if !unreadable.is_empty() {
            self.shared.emit(EngineEvent::Warning(format!(
                "Cannot open {}: permission denied. Add your user to the \"input\" group and log in again.",
                unreadable.join(", ")
            )));
        }
        self.publish();
    }

    /// Registers a probed keyboard, starting a device thread if the filter selects it.
    fn adopt(&mut self, keyboard: Keyboard) {
        if !self.filter.considers(&keyboard.info) {
            return;
        }
        let path = PathBuf::from(&keyboard.info.path);
        if let Some((_, error)) = self.failures.get(&path).filter(|(id, _)| *id == keyboard.info.id) {
            let info = DeviceInfo { error: Some(error.clone()), ..keyboard.info };
            self.entries.push(Entry { info, running: None });
            return;
        }
        if !self.filter.selects(&keyboard.info) {
            self.entries.push(Entry { info: keyboard.info, running: None });
            return;
        }
        let mut info = keyboard.info.clone();
        let caps = keyboard.caps.clone();
        let serial = self.next_serial;
        self.next_serial += 1;
        match WorkerHandle::spawn(serial, keyboard, self.shared.clone(), self.link.clone()) {
            Ok(worker) => self.entries.push(Entry { info, running: Some(Running { worker, caps }) }),
            Err(err) => {
                info.error = Some(format!("cannot start a device thread: {err}"));
                self.entries.push(Entry { info, running: None });
            }
        }
    }

    fn on_report(&mut self, serial: u64, report: Report) {
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.running.as_ref().is_some_and(|running| running.worker.serial == serial))
        else {
            return; // a thread we already stopped
        };
        let entry = &mut self.entries[index];
        match report {
            Report::Grabbed => {
                entry.info.grabbed = true;
                entry.info.error = None;
            }
            Report::WaitingForRelease => entry.info.error = Some(WAITING_FOR_RELEASE.to_string()),
            Report::Failed(error) => {
                entry.info.grabbed = false;
                entry.info.error = Some(error.clone());
                self.shared.emit(EngineEvent::Warning(format!("Cannot use {}: {error}", entry.info.name)));
                self.failures.insert(PathBuf::from(&entry.info.path), (entry.info.id.clone(), error));
            }
            Report::Exited { vanished } => {
                entry.running = None; // joins the (already finishing) thread
                entry.info.grabbed = false;
                if vanished {
                    self.entries.remove(index);
                }
                self.schedule_rescan(HOTPLUG_SETTLE);
            }
        }
        self.publish();
    }

    fn reconfigure(&mut self, filter: DeviceFilter) {
        self.filter = filter.clone();
        self.failures.clear();
        let toggle_key = self.shared.toggle_key();
        // Stop threads whose device is no longer selected; their entries are rebuilt by the rescan.
        self.stop_where(|info, caps| {
            info.eligible = caps.is_eligible(toggle_key);
            !filter.selects(info)
        });
        self.rescan();
    }

    /// Stops the device threads for which `stop` returns true (all at once, then waits for each) and drops
    /// their entries.
    fn stop_where(&mut self, mut stop: impl FnMut(&mut DeviceInfo, &Capabilities) -> bool) {
        let mut stopping = Vec::new();
        for mut entry in std::mem::take(&mut self.entries) {
            let stops = match &entry.running {
                Some(running) => stop(&mut entry.info, &running.caps),
                None => false,
            };
            if stops {
                if let Some(running) = &entry.running {
                    running.worker.request_stop();
                }
                stopping.push(entry);
            } else {
                self.entries.push(entry);
            }
        }
        drop(stopping);
    }

    /// Releases every device and tells the owner the engine stopped.
    fn stop(mut self, reason: String) {
        log::warn!("input engine stopped: {reason}");
        self.stop_where(|_, _| true);
        self.entries.clear();
        self.publish();
        self.output.release_all();
        self.shared.emit(EngineEvent::Stopped { reason });
    }

    fn publish(&mut self) {
        self.entries.sort_by_key(|entry| node_number(&entry.info.path));
        self.shared.publish_devices(self.entries.iter().map(|entry| entry.info.clone()).collect());
    }
}

impl Drop for Manager {
    fn drop(&mut self) {
        self.stop_where(|_, _| true);
    }
}

fn watch_input_dir() -> nix::Result<Inotify> {
    let inotify = Inotify::init(InitFlags::IN_CLOEXEC | InitFlags::IN_NONBLOCK)?;
    inotify.add_watch(INPUT_DIR, AddWatchFlags::IN_CREATE | AddWatchFlags::IN_DELETE | AddWatchFlags::IN_ATTRIB)?;
    Ok(inotify)
}

fn node_number(path: &str) -> u32 {
    path.rsplit('/').next().and_then(discovery::event_number).unwrap_or(u32::MAX)
}
