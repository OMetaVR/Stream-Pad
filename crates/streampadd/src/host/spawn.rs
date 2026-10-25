//! Starting processes: fully detached launches (apps, URLs, fire-and-forget commands) and captured commands.

use std::io::{self, Read};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, mpsc};
use std::time::{Duration, Instant};

/// `run(cmd, {wait: true})` limits.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
pub const OUTPUT_CAP: usize = 1 << 20;

/// Environment the daemon has (from systemd or our own tooling) that launched apps must not inherit.
const PRIVATE_ENV: &[&str] = &[
    "RUST_LOG",
    "RUST_BACKTRACE",
    "INVOCATION_ID",
    "JOURNAL_STREAM",
    "NOTIFY_SOCKET",
    "MANAGERPID",
    "SYSTEMD_EXEC_PID",
    "LISTEN_PID",
    "LISTEN_FDS",
    "LISTEN_FDNAMES",
    "WATCHDOG_PID",
    "WATCHDOG_USEC",
];

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

/// First executable called `name` on `$PATH`.
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|dir| dir.join(name)).find(|p| is_executable(p))
}

/// `name` next to our own executable, else on `$PATH`.
pub fn find_sibling_or_path(name: &str) -> Option<PathBuf> {
    let sibling = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join(name)));
    sibling.filter(|p| is_executable(p)).or_else(|| find_in_path(name))
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Launches programs so they outlive the daemon: a new session, double-forked so they are reparented to the
/// user's subreaper, and (when the user manager is reachable) in their own transient systemd scope, so
/// restarting or stopping `streampad.service` never takes user apps with it.
#[derive(Default)]
pub struct Spawner {
    scopes: OnceLock<bool>,
}

impl Spawner {
    /// Start `argv` detached. `app_id` names the scope after the app (XDG `app-<launcher>-<id>-<random>`).
    pub fn detached(&self, argv: &[String], app_id: Option<&str>, cwd: Option<&Path>) -> Result<(), String> {
        let Some((program, rest)) = argv.split_first() else { return Err("nothing to launch".into()) };
        let mut cmd = if self.scopes_available() {
            let mut c = Command::new("systemd-run");
            c.args(["--user", "--scope", "--quiet", "--collect", "--slice=app.slice"]);
            if let Some(id) = app_id {
                c.arg(format!("--unit=app-streampad-{}-{:08x}.scope", escape_unit_part(id), fastrand::u32(..)));
            }
            c.arg("--").arg(program);
            c
        } else {
            Command::new(program)
        };
        cmd.args(rest).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        cmd.current_dir(cwd.filter(|d| d.is_dir()).map_or_else(home_dir, Path::to_path_buf));
        for var in PRIVATE_ENV {
            cmd.env_remove(var);
        }
        // SAFETY: only async-signal-safe calls (setsid, fork, _exit) between fork and exec.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(io::Error::last_os_error());
                }
                match libc::fork() {
                    -1 => Err(io::Error::last_os_error()),
                    0 => Ok(()),
                    _ => libc::_exit(0),
                }
            });
        }
        // `spawn` returns once the grandchild has exec'd (or reports its exec error); we only reap the
        // intermediate process, which exits immediately.
        let mut intermediate = cmd.spawn().map_err(|e| format!("cannot start {program}: {e}"))?;
        intermediate.wait().map_err(|e| format!("cannot start {program}: {e}"))?;
        Ok(())
    }

    fn scopes_available(&self) -> bool {
        *self.scopes.get_or_init(|| {
            let probe = Command::new("systemd-run")
                .args(["--user", "--scope", "--quiet", "--collect", "--", "true"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let ok = probe.is_ok_and(|s| s.success());
            log::info!("launching apps {}", if ok { "in transient systemd scopes" } else { "without systemd scopes" });
            ok
        })
    }
}

/// systemd unit-name escaping for one name component (`-` separates components, so it is escaped too).
fn escape_unit_part(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'.' | b':' => (b as char).to_string(),
            _ => format!("\\x{b:02x}"),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// `sh -c command`, capturing output (each stream capped at `OUTPUT_CAP`). Killed, with its whole process group,
/// after `COMMAND_TIMEOUT` or as soon as `cancelled()` turns true.
pub fn run_captured(command: &str, cancelled: impl Fn() -> bool) -> Result<CommandOutput, String> {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .current_dir(home_dir())
        .env_remove("RUST_LOG")
        .spawn()
        .map_err(|e| format!("cannot run command: {e}"))?;
    let stdout = child.stdout.take().map(read_capped);
    let stderr = child.stderr.take().map(read_capped);
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => return Err(format!("command failed: {e}")),
        }
        let timed_out = Instant::now() >= deadline;
        if timed_out || cancelled() {
            // SAFETY: plain syscall; the negative pid addresses the process group created above.
            unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
            let _ = child.wait();
            return Err(if timed_out {
                format!("command timed out after {} s", COMMAND_TIMEOUT.as_secs())
            } else {
                "cancelled".into()
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let collect = |c: Option<Capture>| c.map(Capture::finish).unwrap_or_default();
    Ok(CommandOutput { code: exit_code(status), stdout: collect(stdout), stderr: collect(stderr) })
}

fn exit_code(status: ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status.code().or_else(|| status.signal().map(|s| 128 + s)).unwrap_or(-1)
}

/// A stream being read to EOF on a helper thread, keeping the first `OUTPUT_CAP` bytes.
struct Capture {
    kept: Arc<Mutex<Vec<u8>>>,
    eof: mpsc::Receiver<()>,
}

/// How long to wait for EOF after the command exited; background children may hold the pipe open.
const DRAIN_GRACE: Duration = Duration::from_millis(200);

impl Capture {
    fn finish(self) -> String {
        let _ = self.eof.recv_timeout(DRAIN_GRACE);
        let kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        String::from_utf8_lossy(&kept).into_owned()
    }
}

fn read_capped(mut stream: impl Read + Send + 'static) -> Capture {
    let kept = Arc::new(Mutex::new(Vec::new()));
    let (done, eof) = mpsc::channel();
    let sink = kept.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let mut kept = sink.lock().unwrap_or_else(PoisonError::into_inner);
                    let room = OUTPUT_CAP.saturating_sub(kept.len());
                    kept.extend_from_slice(&buf[..n.min(room)]);
                }
            }
        }
        let _ = done.send(());
    });
    Capture { kept, eof }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_output_and_exit_code() {
        let out = run_captured("printf out; printf err >&2; exit 3", || false).unwrap();
        assert_eq!(out, CommandOutput { code: 3, stdout: "out".into(), stderr: "err".into() });
    }

    #[test]
    fn output_is_capped() {
        let out = run_captured("head -c 3000000 /dev/zero", || false).unwrap();
        assert_eq!(out.stdout.len(), OUTPUT_CAP);
    }

    #[test]
    fn cancel_kills_the_process_group() {
        let started = Instant::now();
        let err = run_captured("sleep 5; echo late", || started.elapsed() > Duration::from_millis(100)).unwrap_err();
        assert_eq!(err, "cancelled");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn background_children_do_not_block_the_result() {
        let started = Instant::now();
        let out = run_captured("sleep 3 & echo done", || false).unwrap();
        assert_eq!(out.stdout.trim(), "done");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn detached_process_is_not_our_child_and_runs() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let script = format!("echo $PPID > '{}'", marker.display());
        let spawner = Spawner { scopes: OnceLock::from(false) };
        spawner.detached(&["sh".into(), "-c".into(), script], None, None).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !marker.exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let ppid: u32 = std::fs::read_to_string(&marker).unwrap().trim().parse().unwrap();
        assert_ne!(ppid, std::process::id(), "must be reparented away from the daemon");
    }

    #[test]
    fn unit_names_are_escaped() {
        assert_eq!(escape_unit_part("org.kde.plasma-systemmonitor"), "org.kde.plasma\\x2dsystemmonitor");
        assert_eq!(escape_unit_part("a b/c"), "a\\x20b\\x2fc");
    }
}
