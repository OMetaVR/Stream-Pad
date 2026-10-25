//! One thread per grabbed keyboard: it creates the mirror, grabs the source once no key is held, forwards
//! events and LEDs, and tears everything down again. The thread owns both fds, so even a panic releases the
//! grab (closing the source fd) and destroys the mirror (closing the uinput fd).

use crate::bits::Bits;
use crate::discovery::Keyboard;
use crate::evdev::is_vanished;
use crate::forward::{Effect, Forwarder};
use crate::manager::{Command, ManagerLink};
use crate::mirror;
use crate::shared::Shared;
use crate::sys::{self, EV_KEY, EV_LED, EV_REP, InputEvent, LED_SCROLLL};
use crate::uinput::{VirtualDevice, describe_open_error};
use crate::{EngineEvent, KeyAction, ModeSource};
use nix::errno::Errno;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use nix::sys::eventfd::{EfdFlags, EventFd};
use std::convert::Infallible;
use std::io;
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Key-state polling interval while waiting for held keys to be released.
const IDLE_POLL: Duration = Duration::from_millis(20);
/// Waiting longer than this for held keys is logged and shown on the device.
const SLOW_RELEASE: Duration = Duration::from_secs(3);
const READ_BATCH: usize = 64;

pub(crate) const WAITING_FOR_RELEASE: &str = "waiting for held keys to be released";

/// Progress reported to the manager.
#[derive(Debug)]
pub(crate) enum Report {
    Grabbed,
    WaitingForRelease,
    Failed(String),
    Exited { vanished: bool },
}

/// The manager's handle on a device thread. Dropping it stops the thread and waits for it.
pub(crate) struct WorkerHandle {
    pub serial: u64,
    wake: Arc<EventFd>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl WorkerHandle {
    pub fn spawn(serial: u64, keyboard: Keyboard, shared: Arc<Shared>, link: ManagerLink) -> io::Result<WorkerHandle> {
        let wake = Arc::new(EventFd::from_value_and_flags(0, EfdFlags::EFD_CLOEXEC | EfdFlags::EFD_NONBLOCK)?);
        let stop = Arc::new(AtomicBool::new(false));
        let node = keyboard.info.path.rsplit('/').next().unwrap_or("device").to_string();
        let worker = Worker { serial, keyboard, shared, link, wake: wake.clone(), stop: stop.clone() };
        let thread = thread::Builder::new().name(format!("streampad-{node}")).spawn(move || worker.run())?;
        Ok(WorkerHandle { serial, wake, stop, thread: Some(thread) })
    }

    /// Wakes the thread so it re-applies the macro-mode LED.
    pub fn notify(&self) {
        let _ = self.wake.write(1);
    }

    /// Asks the thread to release everything, without waiting.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.notify();
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.request_stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

enum Exit {
    Stopped,
    Vanished,
    Failed(String),
}

impl Exit {
    fn from_io(err: &io::Error, what: &str) -> Exit {
        if is_vanished(err) { Exit::Vanished } else { Exit::Failed(format!("{what}: {err}")) }
    }
}

struct Worker {
    serial: u64,
    keyboard: Keyboard,
    shared: Arc<Shared>,
    link: ManagerLink,
    wake: Arc<EventFd>,
    stop: Arc<AtomicBool>,
}

impl Worker {
    fn run(self) {
        let mut notice = ExitNotice { link: self.link.clone(), serial: self.serial, vanished: false };
        notice.vanished = self.serve_and_report();
    }

    /// Returns whether the device vanished. Takes the worker by value so its fds are closed before the
    /// manager hears about the exit.
    fn serve_and_report(self) -> bool {
        match self.serve() {
            Exit::Stopped => false,
            Exit::Vanished => {
                log::info!("{} disappeared", self.keyboard.info.name);
                true
            }
            Exit::Failed(error) => {
                log::warn!("{} ({}): {error}", self.keyboard.info.name, self.keyboard.info.path);
                self.report(Report::Failed(error));
                false
            }
        }
    }

    fn serve(&self) -> Exit {
        let kb = &self.keyboard;
        if let Err(err) = kb.device.use_monotonic_clock() {
            return Exit::from_io(&err, "cannot configure the device");
        }
        let mirror = match mirror::create(&kb.info.name, kb.input_id, kb.info.phys.as_deref(), &kb.caps) {
            Ok(mirror) => mirror,
            Err(err) => return Exit::Failed(format!("cannot create the mirror device: {}", describe_open_error(&err))),
        };
        let mut settle = mirror::Settle::new(&mirror);
        let backlog = loop {
            match self.wait_until_idle(&mut settle) {
                Ok(true) => {}
                Ok(false) => return Exit::Stopped,
                Err(err) => return Exit::from_io(&err, "cannot read the key state"),
            }
            match self.grab_cleanly() {
                Ok(Some(backlog)) => break backlog,
                Ok(None) => continue,
                Err(err) if err.raw_os_error() == Some(libc::EBUSY) => {
                    return Exit::Failed("another program has grabbed this device".to_string());
                }
                Err(err) => return Exit::from_io(&err, "cannot grab the device"),
            }
        };
        log::info!("grabbed {} ({})", kb.info.name, kb.info.path);
        self.report(Report::Grabbed);
        let mut session = Session::new(self, mirror);
        let exit = session.run(&backlog);
        session.teardown(&exit);
        exit
    }

    /// Waits until the mirror has settled and no key is held on the source; `false` when asked to stop.
    fn wait_until_idle(&self, settle: &mut mirror::Settle) -> io::Result<bool> {
        let started = Instant::now();
        let mut announced = false;
        loop {
            if self.stopping() {
                return Ok(false);
            }
            self.discard_pending()?;
            let idle = self.keyboard.device.pressed_keys()?.is_empty();
            if idle && settle.settled() {
                return Ok(true);
            }
            if !idle && !announced && started.elapsed() >= SLOW_RELEASE {
                log::info!("{}: {WAITING_FOR_RELEASE} before grabbing", self.keyboard.info.name);
                self.report(Report::WaitingForRelease);
                announced = true;
            }
            self.sleep(IDLE_POLL);
        }
    }

    /// Grabs the source so every event is delivered exactly once. Events stamped before the grab completed
    /// may already have reached the system directly, so they are dropped. If one of them pressed a key, the
    /// system saw that press and must also see its release, so the grab is undone and we go back to waiting
    /// (`None`). Otherwise returns the events that arrived after the grab.
    ///
    /// Only a release within microseconds of such a press could still slip through, which no physical key
    /// does.
    fn grab_cleanly(&self) -> io::Result<Option<Vec<InputEvent>>> {
        let device = &self.keyboard.device;
        device.set_grab(true)?;
        let grabbed_at = sys::monotonic_now();
        let mut held_by_system = Bits::new();
        let mut backlog = Vec::new();
        let mut buf = [InputEvent::new(0, 0, 0); READ_BATCH];
        loop {
            let n = match device.read_events(&mut buf) {
                Ok(n) => n,
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(err),
            };
            if n == 0 {
                break;
            }
            for ev in &buf[..n] {
                if ev.time() >= grabbed_at {
                    backlog.push(*ev);
                } else if ev.kind == EV_KEY && ev.value == 1 {
                    held_by_system.insert(ev.code);
                } else if ev.kind == EV_KEY && ev.value == 0 {
                    held_by_system.remove(ev.code);
                }
            }
        }
        if held_by_system.is_empty() {
            return Ok(Some(backlog));
        }
        device.set_grab(false)?;
        Ok(None)
    }

    /// Drops queued events: before the grab they reach the system directly.
    fn discard_pending(&self) -> io::Result<()> {
        let mut buf = [InputEvent::new(0, 0, 0); READ_BATCH];
        loop {
            match self.keyboard.device.read_events(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(_) => {}
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
                Err(err) => return Err(err),
            }
        }
    }

    /// Sleeps up to `timeout`, returning early when woken.
    fn sleep(&self, timeout: Duration) {
        let mut fds = [PollFd::new(self.wake.as_fd(), PollFlags::POLLIN)];
        let _ = poll(&mut fds, sys::poll_timeout(timeout));
        self.drain_wake();
    }

    fn drain_wake(&self) {
        let _ = self.wake.read();
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    fn report(&self, report: Report) {
        self.link.send(Command::Worker { serial: self.serial, report });
    }
}

/// The grabbed state: forwarding between the source and its mirror.
struct Session<'w> {
    worker: &'w Worker,
    mirror: VirtualDevice,
    forwarder: Forwarder,
    /// The source has a Scroll Lock LED we can drive.
    scroll_led: bool,
    /// Macro mode as last shown on the Scroll Lock LED.
    shown: Option<bool>,
    /// The compositor's own Scroll Lock state, restored when the device is released.
    compositor_scroll: bool,
    out: Vec<InputEvent>,
}

impl<'w> Session<'w> {
    fn new(worker: &'w Worker, mirror: VirtualDevice) -> Session<'w> {
        let kb = &worker.keyboard;
        let compositor_scroll = kb.device.leds().is_ok_and(|leds| leds.contains(LED_SCROLLL));
        Session {
            worker,
            mirror,
            forwarder: Forwarder::new(),
            scroll_led: kb.caps.has_led(LED_SCROLLL) && kb.device.writable(),
            shown: None,
            compositor_scroll,
            out: Vec::with_capacity(READ_BATCH + 8),
        }
    }

    fn run(&mut self, backlog: &[InputEvent]) -> Exit {
        self.show_mode();
        let Err(exit) = self.forward_until_exit(backlog);
        exit
    }

    fn forward_until_exit(&mut self, backlog: &[InputEvent]) -> Result<Infallible, Exit> {
        self.forward(backlog)?;
        let mut buf = [InputEvent::new(0, 0, 0); READ_BATCH];
        loop {
            let (source, mirror, woken) = self.wait()?;
            if source {
                self.pump_source(&mut buf)?;
            }
            if mirror {
                self.pump_mirror(&mut buf)?;
            }
            if woken {
                self.worker.drain_wake();
                if self.worker.stopping() {
                    return Err(Exit::Stopped);
                }
                self.show_mode();
            }
        }
    }

    /// Blocks until the source, the mirror or the wake fd is ready.
    fn wait(&self) -> Result<(bool, bool, bool), Exit> {
        let kb = &self.worker.keyboard;
        let mut fds = [
            PollFd::new(kb.device.as_fd(), PollFlags::POLLIN),
            PollFd::new(self.mirror.as_fd(), PollFlags::POLLIN),
            PollFd::new(self.worker.wake.as_fd(), PollFlags::POLLIN),
        ];
        loop {
            match poll(&mut fds, PollTimeout::NONE) {
                Ok(_) => break,
                Err(Errno::EINTR) => continue,
                Err(err) => return Err(Exit::Failed(format!("poll failed: {err}"))),
            }
        }
        // POLLERR / POLLHUP on the source mean it vanished; reading it then reports ENODEV.
        let ready = |fd: &PollFd| fd.revents().is_some_and(|events| !events.is_empty());
        Ok((ready(&fds[0]), ready(&fds[1]), ready(&fds[2])))
    }

    fn pump_source(&mut self, buf: &mut [InputEvent]) -> Result<(), Exit> {
        loop {
            let n = match self.worker.keyboard.device.read_events(buf) {
                Ok(n) => n,
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(Exit::from_io(&err, "reading the device failed")),
            };
            self.forward(&buf[..n])?;
            if n < buf.len() {
                return Ok(());
            }
        }
    }

    fn forward(&mut self, events: &[InputEvent]) -> Result<(), Exit> {
        let shared = &self.worker.shared;
        for ev in events {
            match self.forwarder.push(*ev, &shared.mode(), &mut self.out) {
                None => {}
                Some(Effect::Pad(key, action)) => shared.emit(EngineEvent::PadKey { key, action }),
                Some(Effect::ToggleMode) => shared.toggle_macro_mode(ModeSource::ToggleKey),
                Some(Effect::Resync) => self.resync()?,
                Some(Effect::Led(code, value)) => self.on_source_led(code, value),
            }
        }
        if self.out.is_empty() {
            return Ok(());
        }
        let written = self.mirror.write_events(&self.out);
        self.out.clear();
        written.map_err(|err| Exit::Failed(format!("writing to the mirror failed: {err}")))
    }

    /// The kernel dropped events: release whatever is no longer held according to the device.
    fn resync(&mut self) -> Result<(), Exit> {
        log::debug!("{}: input events were dropped, resynchronizing", self.worker.keyboard.info.name);
        let pressed = self
            .worker
            .keyboard
            .device
            .pressed_keys()
            .map_err(|err| Exit::from_io(&err, "cannot read the key state"))?;
        for (key, action) in self.forwarder.resync(|code| pressed.contains(code), &mut self.out) {
            self.worker.shared.emit(EngineEvent::PadKey { key, action });
        }
        Ok(())
    }

    /// Output events the compositor wrote to the mirror: LEDs and repeat settings go on to the source,
    /// except Scroll Lock, which shows macro mode instead.
    fn pump_mirror(&mut self, buf: &mut [InputEvent]) -> Result<(), Exit> {
        let mut to_source = Vec::new();
        let mut repeat_changed = false;
        loop {
            let n = match self.mirror.read_events(buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                Err(err) => return Err(Exit::Failed(format!("reading the mirror failed: {err}"))),
            };
            for ev in &buf[..n] {
                match ev.kind {
                    EV_LED if ev.code == LED_SCROLLL => self.compositor_scroll = ev.value != 0,
                    EV_LED => to_source.push(*ev),
                    // A client changed the mirror's repeat rate: the source generates the repeats we forward,
                    // so it gets the setting, and the mirror's own software repeat stays off.
                    EV_REP if ev.value != 0 => {
                        to_source.push(*ev);
                        repeat_changed = true;
                    }
                    _ => {}
                }
            }
        }
        if !to_source.is_empty() {
            to_source.push(InputEvent::syn_report());
            self.write_source(&to_source);
        }
        if repeat_changed {
            self.mirror
                .write_events(&mirror::software_repeat_off())
                .map_err(|err| Exit::Failed(format!("writing to the mirror failed: {err}")))?;
        }
        Ok(())
    }

    /// The compositor writes all LEDs to every keyboard, the grabbed one included; when that overwrites
    /// the macro-mode LED, it is set again.
    fn on_source_led(&mut self, code: u16, value: i32) {
        if code == LED_SCROLLL && self.shown.is_some_and(|shown| shown != (value != 0)) {
            self.shown = None;
            self.show_mode();
        }
    }

    fn show_mode(&mut self) {
        let on = self.worker.shared.macro_mode();
        if self.scroll_led && self.shown != Some(on) && self.write_source(&scroll_led(on)) {
            self.shown = Some(on);
        }
    }

    /// Best effort: a failing LED write only matters if the device is gone, which reading reports.
    fn write_source(&self, events: &[InputEvent]) -> bool {
        let kb = &self.worker.keyboard;
        if !kb.device.writable() {
            return false;
        }
        match kb.device.write_events(events) {
            Ok(()) => true,
            Err(err) => {
                log::debug!("{}: cannot set LEDs: {err}", kb.info.name);
                false
            }
        }
    }

    /// Releases captured keys towards StreamPad, hands the Scroll Lock LED back to the compositor, ungrabs
    /// the source and finally destroys the mirror (which releases forwarded keys in the kernel).
    fn teardown(mut self, exit: &Exit) {
        for key in self.forwarder.release_all() {
            self.worker.shared.emit(EngineEvent::PadKey { key, action: KeyAction::Release });
        }
        if !matches!(exit, Exit::Vanished) {
            if self.scroll_led {
                self.write_source(&scroll_led(self.compositor_scroll));
            }
            if let Err(err) = self.worker.keyboard.device.set_grab(false) {
                log::debug!("{}: ungrab failed: {err}", self.worker.keyboard.info.name);
            }
            log::info!("released {} ({})", self.worker.keyboard.info.name, self.worker.keyboard.info.path);
        }
    }
}

fn scroll_led(on: bool) -> [InputEvent; 2] {
    [InputEvent::new(EV_LED, LED_SCROLLL, i32::from(on)), InputEvent::syn_report()]
}

/// Tells the manager the thread is gone — also when it unwinds from a panic.
struct ExitNotice {
    link: ManagerLink,
    serial: u64,
    vanished: bool,
}

impl Drop for ExitNotice {
    fn drop(&mut self) {
        if thread::panicking() {
            let report = Report::Failed("internal error: the device thread panicked".to_string());
            self.link.send(Command::Worker { serial: self.serial, report });
        }
        self.link.send(Command::Worker { serial: self.serial, report: Report::Exited { vanished: self.vanished } });
    }
}
