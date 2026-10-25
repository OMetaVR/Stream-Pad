//! Single instance: an exclusive lock next to the socket, then a fresh socket bound with mode 0600.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum InstanceError {
    AlreadyRunning(PathBuf),
    Io(PathBuf, io::Error),
}

impl std::fmt::Display for InstanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstanceError::AlreadyRunning(socket) => write!(f, "streampadd is already running (socket {})", socket.display()),
            InstanceError::Io(path, e) => write!(f, "{}: {e}", path.display()),
        }
    }
}

impl std::error::Error for InstanceError {}

/// Held for the daemon's lifetime; removes the socket on drop.
pub struct Instance {
    _lock: File,
    socket: PathBuf,
    listener: Option<UnixListener>,
}

impl Instance {
    pub fn acquire(socket: &Path) -> Result<Instance, InstanceError> {
        let io_err = |p: &Path| {
            let p = p.to_path_buf();
            move |e| InstanceError::Io(p, e)
        };
        if let Some(dir) = socket.parent() {
            fs::DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(io_err(dir))?;
        }
        let lock_path = socket.with_extension("lock");
        let lock = OpenOptions::new().create(true).truncate(false).write(true).mode(0o600).open(&lock_path).map_err(io_err(&lock_path))?;
        // SAFETY: plain syscall on a file descriptor we own.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let e = io::Error::last_os_error();
            return Err(if e.kind() == io::ErrorKind::WouldBlock {
                InstanceError::AlreadyRunning(socket.to_path_buf())
            } else {
                InstanceError::Io(lock_path, e)
            });
        }
        if socket.exists() {
            if UnixStream::connect(socket).is_ok() {
                return Err(InstanceError::AlreadyRunning(socket.to_path_buf()));
            }
            fs::remove_file(socket).map_err(io_err(socket))?;
        }
        // Create the socket owner-only from the start (no window where others could connect). umask is
        // process-wide, which is why the daemon acquires its instance first thing, before spawning threads.
        // SAFETY: umask has no memory-safety preconditions.
        let old_mask = unsafe { libc::umask(0o177) };
        let bound = UnixListener::bind(socket);
        // SAFETY: as above.
        unsafe { libc::umask(old_mask) };
        let listener = bound.map_err(io_err(socket))?;
        fs::set_permissions(socket, fs::Permissions::from_mode(0o600)).map_err(io_err(socket))?;
        Ok(Instance { _lock: lock, socket: socket.to_path_buf(), listener: Some(listener) })
    }

    /// The bound listener (taken once by the IPC server).
    pub fn take_listener(&mut self) -> Option<UnixListener> {
        self.listener.take()
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.socket);
    }
}
