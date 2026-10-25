//! Keeps `streampad-overlay` running (docs/SPEC.md §5): restarts it with exponential backoff and makes sure it
//! never outlives the daemon.

use crate::host::spawn::find_sibling_or_path;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tokio::time::Instant;

pub const BINARY: &str = "streampad-overlay";
const FIRST_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// A run this long resets the backoff.
const STABLE_AFTER: Duration = Duration::from_secs(30);
const TERM_GRACE: Duration = Duration::from_secs(2);

/// Runs until `stop` flips to true, then terminates the overlay.
pub async fn supervise(socket: PathBuf, mut stop: watch::Receiver<bool>) {
    let Some(exe) = find_sibling_or_path(BINARY) else {
        log::warn!("{BINARY} not found next to streampadd or on PATH; running without the on-screen overlay");
        return;
    };
    let mut backoff = FIRST_BACKOFF;
    loop {
        let started = Instant::now();
        match spawn(&exe, &socket) {
            Ok(mut child) => {
                log::info!("started {} (pid {})", exe.display(), child.id().unwrap_or(0));
                tokio::select! {
                    status = child.wait() => {
                        let status = status.map_or_else(|e| e.to_string(), |s| s.to_string());
                        if started.elapsed() >= STABLE_AFTER {
                            backoff = FIRST_BACKOFF;
                        }
                        log::warn!("overlay exited ({status}); restarting in {} s", backoff.as_secs());
                    }
                    _ = stop.changed() => {
                        terminate(&mut child).await;
                        return;
                    }
                }
            }
            Err(e) => log::error!("cannot start {}: {e}; retrying in {} s", exe.display(), backoff.as_secs()),
        }
        tokio::select! {
            () = tokio::time::sleep(backoff) => {}
            _ = stop.changed() => return,
        }
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

fn spawn(exe: &Path, socket: &Path) -> std::io::Result<Child> {
    let parent = std::process::id() as libc::pid_t;
    let mut cmd = Command::new(exe);
    cmd.env("STREAMPAD_SOCKET", socket).stdin(Stdio::null()).kill_on_drop(true);
    // SAFETY: only async-signal-safe calls between fork and exec.
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            // The daemon may have died between fork and prctl.
            if libc::getppid() != parent {
                libc::_exit(0);
            }
            Ok(())
        });
    }
    cmd.spawn()
}

async fn terminate(child: &mut Child) {
    if let Some(pid) = child.id() {
        // SAFETY: plain syscall on our own child.
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
    }
    if tokio::time::timeout(TERM_GRACE, child.wait()).await.is_err() {
        let _ = child.kill().await;
    }
}
