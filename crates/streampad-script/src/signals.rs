//! Cross-thread control of one run: cancellation, key release and completion, plus the interruptible
//! sleep behind JS `sleep()`. All waits share one condvar so `cancel()` wakes whatever the worker is
//! blocked on immediately.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(crate) struct RunSignals {
    /// Mirror of `Flags::cancelled` for the QuickJS interrupt handler, which polls it without locking.
    cancelled: AtomicBool,
    flags: Mutex<Flags>,
    changed: Condvar,
}

#[derive(Default)]
struct Flags {
    cancelled: bool,
    released: bool,
    finished: bool,
}

impl RunSignals {
    pub(crate) fn cancel(&self) {
        // Atomic first: a worker woken by the condvar must already see the run as cancelled.
        self.cancelled.store(true, Ordering::SeqCst);
        self.update(|f| f.cancelled = true);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Remembered until the worker asks for it, so a release that arrives while the press handler is
    /// still running is not lost.
    pub(crate) fn release(&self) {
        self.update(|f| f.released = true);
    }

    pub(crate) fn mark_finished(&self) {
        self.update(|f| f.finished = true);
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.lock().finished
    }

    /// Blocks until the key is released (`true`) or the run is cancelled (`false`).
    pub(crate) fn wait_release(&self) -> bool {
        let flags = self.wait_until(None, |f| f.released || f.cancelled);
        !flags.cancelled
    }

    /// Sleeps for `ms` milliseconds (non-finite or huge values: until cancelled). Returns `false` when the
    /// sleep was cut short by cancellation.
    pub(crate) fn sleep(&self, ms: f64) -> bool {
        let deadline = if ms <= 0.0 {
            Some(Instant::now())
        } else {
            Duration::try_from_secs_f64(ms / 1000.0).ok().and_then(|d| Instant::now().checked_add(d))
        };
        !self.wait_until(deadline, |f| f.cancelled).cancelled
    }

    /// Blocks until the run has finished or `timeout` passed; returns whether it finished.
    pub(crate) fn wait_finished(&self, timeout: Duration) -> bool {
        self.wait_until(Instant::now().checked_add(timeout), |f| f.finished).finished
    }

    fn wait_until(&self, deadline: Option<Instant>, done: impl Fn(&Flags) -> bool) -> MutexGuard<'_, Flags> {
        let mut flags = self.lock();
        while !done(&flags) {
            flags = match deadline {
                None => self.changed.wait(flags).unwrap_or_else(PoisonError::into_inner),
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }
                    self.changed.wait_timeout(flags, deadline - now).unwrap_or_else(PoisonError::into_inner).0
                }
            };
        }
        flags
    }

    fn update(&self, f: impl FnOnce(&mut Flags)) {
        f(&mut self.lock());
        self.changed.notify_all();
    }

    fn lock(&self) -> MutexGuard<'_, Flags> {
        self.flags.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn sleep_runs_to_its_deadline() {
        let s = RunSignals::default();
        let start = Instant::now();
        assert!(s.sleep(30.0));
        assert!(start.elapsed() >= Duration::from_millis(30));
        assert!(s.sleep(0.0) && s.sleep(-5.0));
    }

    #[test]
    fn cancel_cuts_sleep_and_release_wait_short() {
        let s = Arc::new(RunSignals::default());
        let canceller = {
            let s = s.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(20));
                s.cancel();
            })
        };
        let start = Instant::now();
        assert!(!s.sleep(f64::INFINITY));
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(!s.wait_release());
        assert!(!s.sleep(10_000.0));
        canceller.join().unwrap();
    }

    #[test]
    fn early_release_is_remembered() {
        let s = RunSignals::default();
        s.release();
        assert!(s.wait_release());
        assert!(!s.wait_finished(Duration::from_millis(1)));
        s.mark_finished();
        assert!(s.is_finished() && s.wait_finished(Duration::ZERO));
    }
}
