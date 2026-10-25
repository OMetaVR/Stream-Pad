//! Starting the daemon and "start at login". Uses the `streampad.service` systemd user unit when it is
//! installed; otherwise (or when StreamPad paths are overridden for development/tests) runs `streampadd`
//! directly and manages an XDG autostart entry.

use crate::daemon::{DaemonClient, DaemonStatus};
use crate::env;
use crate::error::{CmdError, CmdResult, blocking};
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use streampad_core::paths::Paths;
use tauri::{AppHandle, State};

const UNIT: &str = "streampad.service";
const DAEMON_BIN: &str = "streampadd";
const AUTOSTART_FILE: &str = "streampad-daemon.desktop";
const SYSTEMCTL_TIMEOUT: Duration = Duration::from_secs(15);

enum Supervisor {
    Systemd,
    Direct,
}

async fn supervisor() -> Supervisor {
    if env::has_path_overrides() {
        return Supervisor::Direct;
    }
    match systemctl(&["show", "--property=LoadState", "--value", UNIT]).await {
        Ok(out) if out.status_ok && out.stdout.trim() == "loaded" => Supervisor::Systemd,
        _ => Supervisor::Direct,
    }
}

struct Output {
    status_ok: bool,
    stdout: String,
    stderr: String,
}

async fn systemctl(args: &[&str]) -> CmdResult<Output> {
    let run = tokio::process::Command::new("systemctl").arg("--user").args(args).stdin(Stdio::null()).output();
    let out = tokio::time::timeout(SYSTEMCTL_TIMEOUT, run)
        .await
        .map_err(|_| CmdError::new("systemctl did not respond"))?
        .map_err(|e| CmdError::new(format!("cannot run systemctl: {e}")))?;
    Ok(Output {
        status_ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
    })
}

async fn systemctl_checked(args: &[&str]) -> CmdResult<()> {
    let out = systemctl(args).await?;
    if out.status_ok {
        Ok(())
    } else {
        Err(CmdError::new(format!("systemctl --user {} failed: {}", args.join(" "), out.stderr)))
    }
}

/// `$STREAMPAD_DAEMON_BIN`, next to the editor binary, `$PATH`, then `~/.local/bin`.
fn daemon_binary() -> Option<PathBuf> {
    let explicit = std::env::var_os("STREAMPAD_DAEMON_BIN").map(PathBuf::from);
    let sibling = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(DAEMON_BIN)));
    explicit
        .into_iter()
        .chain(sibling)
        .chain(env::find_in_path(DAEMON_BIN))
        .chain(std::iter::once(env::home().join(".local/bin").join(DAEMON_BIN)))
        .find(|p| env::is_executable(p))
}

/// Run `streampadd` in its own process group (it outlives the editor); output goes to
/// `<state dir>/daemon.log`.
fn spawn_daemon() -> CmdResult<()> {
    let bin = daemon_binary()
        .ok_or_else(|| CmdError::new("streampadd was not found next to the editor, on PATH or in ~/.local/bin"))?;
    let log_path = Paths::resolve().state_dir.join("daemon.log");
    let log = log_path
        .parent()
        .and_then(|d| fs::create_dir_all(d).ok())
        .and_then(|_| fs::OpenOptions::new().create(true).append(true).open(&log_path).ok());
    let (stdout, stderr) = match log.and_then(|f| f.try_clone().ok().map(|g| (f, g))) {
        Some((a, b)) => (Stdio::from(a), Stdio::from(b)),
        None => (Stdio::null(), Stdio::null()),
    };
    let mut child = Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .process_group(0)
        .spawn()
        .map_err(|e| CmdError::new(format!("cannot start {}: {e}", bin.display())))?;
    log::info!("started {} (pid {})", bin.display(), child.id());
    // Reap it if it exits while the editor is still open.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[tauri::command]
pub async fn start_daemon(app: AppHandle, client: State<'_, DaemonClient>) -> CmdResult<()> {
    if matches!(client.status(), DaemonStatus::Connected { .. }) {
        return Ok(());
    }
    match supervisor().await {
        Supervisor::Systemd => systemctl_checked(&["start", UNIT]).await?,
        Supervisor::Direct => blocking(spawn_daemon).await?,
    }
    client.expect_daemon(&app);
    Ok(())
}

fn autostart_path() -> PathBuf {
    env::config_home().join("autostart").join(AUTOSTART_FILE)
}

fn autostart_entry_enabled() -> bool {
    crate::ini::parse_file(&autostart_path()).and_then(|g| g.get("Desktop Entry").cloned()).is_some_and(|g| {
        !crate::ini::is_true(&g, "Hidden") && g.get("X-GNOME-Autostart-enabled").is_none_or(|v| v != "false")
    })
}

/// Quote an argument for a desktop entry `Exec=` key.
fn exec_quote(arg: &str) -> String {
    if !arg.chars().any(|c| c.is_whitespace() || "\"'\\`$;&|<>()*?#~".contains(c)) {
        return arg.to_string();
    }
    let mut q = String::from("\"");
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            q.push('\\');
        }
        q.push(c);
    }
    q.push('"');
    q
}

fn write_autostart_entry() -> CmdResult<()> {
    let bin = daemon_binary().ok_or_else(|| CmdError::new("streampadd was not found, so it cannot start at login"))?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=StreamPad\nComment=Numpad macropad background service\nExec={}\nIcon=io.github.ometavr.streampad\nTerminal=false\nNoDisplay=true\nX-KDE-autostart-phase=2\n",
        exec_quote(&bin.display().to_string())
    );
    streampad_core::write_atomic(&autostart_path(), entry.as_bytes())?;
    Ok(())
}

#[tauri::command]
pub async fn get_autostart() -> CmdResult<bool> {
    match supervisor().await {
        // `is-enabled` exits non-zero for "disabled", so only stdout matters.
        Supervisor::Systemd => {
            Ok(matches!(systemctl(&["is-enabled", UNIT]).await?.stdout.trim(), "enabled" | "enabled-runtime"))
        }
        Supervisor::Direct => blocking(|| Ok(autostart_entry_enabled())).await,
    }
}

#[tauri::command]
pub async fn set_autostart(on: bool) -> CmdResult<()> {
    match supervisor().await {
        Supervisor::Systemd => systemctl_checked(&[if on { "enable" } else { "disable" }, UNIT]).await,
        Supervisor::Direct => {
            blocking(move || {
                if on {
                    return write_autostart_entry();
                }
                match fs::remove_file(autostart_path()) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
                    _ => Ok(()),
                }
            })
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_exec_arguments() {
        assert_eq!(exec_quote("/home/me/.local/bin/streampadd"), "/home/me/.local/bin/streampadd");
        assert_eq!(exec_quote("/opt/Stream Pad/streampadd"), "\"/opt/Stream Pad/streampadd\"");
        assert_eq!(exec_quote("/a$b"), "\"/a\\$b\"");
    }
}
