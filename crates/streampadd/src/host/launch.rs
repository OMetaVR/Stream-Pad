//! `app.launch` (focus-or-launch), `open.url` / `open.path`, and opening the editor. All calls block, so async
//! callers use `spawn_blocking`.

use super::desktop_entry::{self, DesktopEntry};
use super::spawn::{Spawner, find_in_path, find_sibling_or_path, home_dir};
use crate::kwin::Kwin;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::runtime::Handle;

/// Window ids the editor may show up with: its StartupWMClass / binary name, and its app id.
const EDITOR_WINDOW_IDS: &[&str] = &["streampad", super::APP_ID];
const EDITOR_BINARY: &str = "streampad";

/// Programs that only wrap the real app, so their name says nothing about its windows.
const LAUNCH_WRAPPERS: &[&str] = &["env", "sh", "bash", "dash", "zsh", "flatpak", "snap", "gio", "gtk-launch", "xdg-open", "python", "python3", "java"];

/// Terminals for `Terminal=true` entries, with the flag that precedes the command.
const TERMINALS: &[(&str, Option<&str>)] = &[
    ("xdg-terminal-exec", None),
    ("konsole", Some("-e")),
    ("gnome-terminal", Some("--")),
    ("alacritty", Some("-e")),
    ("kitty", None),
    ("foot", None),
    ("xterm", Some("-e")),
];

pub struct Launcher {
    spawner: Spawner,
    kwin: Option<Arc<Kwin>>,
    rt: Handle,
}

impl Launcher {
    pub fn new(kwin: Option<Arc<Kwin>>, rt: Handle) -> Launcher {
        Launcher { spawner: Spawner::default(), kwin, rt }
    }

    pub fn spawner(&self) -> &Spawner {
        &self.spawner
    }

    /// `app` is a desktop-file id (exact or fuzzy, see `desktop_entry::find`) or else a command line.
    pub fn launch_app(&self, app: &str, focus_if_running: bool, args: &str) -> Result<(), String> {
        let app = app.trim();
        if app.is_empty() {
            return Err("app.launch: no app given".into());
        }
        let entry = desktop_entry::find(app);
        if focus_if_running {
            let ids = match &entry {
                Some(e) => window_ids(e),
                None => app.split_whitespace().next().map(program_name).into_iter().collect(),
            };
            if self.focus_any(&ids) {
                return Ok(());
            }
        }
        match entry {
            Some(e) => self.start_entry(&e, &desktop_entry::split_args(args)),
            None => {
                let command = if args.trim().is_empty() { app.to_string() } else { format!("{app} {args}") };
                let id = app.split_whitespace().next().map(program_name);
                self.spawner.detached(&["/bin/sh".into(), "-c".into(), command], id.as_deref(), None)
            }
        }
    }

    /// Close every window of `app` (desktop id or program name). With `force`, its processes are killed
    /// (SIGKILL) instead — for apps that hang. Returns whether anything matched.
    pub fn close_app(&self, app: &str, force: bool) -> Result<bool, String> {
        let app = app.trim();
        if app.is_empty() {
            return Err("app.close: no app given".into());
        }
        let ids = match desktop_entry::find(app) {
            Some(e) => window_ids(&e),
            None => app.split_whitespace().next().map(program_name).into_iter().collect(),
        };
        if let Some(kwin) = self.kwin.as_ref().filter(|k| k.available()) {
            let pids = self.rt.block_on(kwin.close_app(&ids))?;
            if force {
                for pid in &pids {
                    // SAFETY: plain syscall; pids come from KWin's window list.
                    unsafe { libc::kill(*pid as libc::pid_t, libc::SIGKILL) };
                }
            }
            if !pids.is_empty() {
                return Ok(true);
            }
        }
        // No (matching) windows, or not on KDE: fall back to the program name.
        let Some(program) = ids.last() else { return Ok(false) };
        let signal = if force { "-KILL" } else { "-TERM" };
        let status = std::process::Command::new("pkill")
            .args([signal, "-x", program])
            .status()
            .map_err(|e| format!("app.close: pkill: {e}"))?;
        Ok(status.success())
    }

    fn start_entry(&self, e: &DesktopEntry, extra: &[String]) -> Result<(), String> {
        // `gio launch` implements the whole spec (D-Bus activation, Terminal, Path, field codes); our own Exec
        // handling covers extra arguments and systems without GLib's tool.
        if extra.is_empty() && find_in_path("gio").is_some() {
            let argv = vec!["gio".into(), "launch".into(), e.path.to_string_lossy().into_owned()];
            return self.spawner.detached(&argv, Some(&e.id), None);
        }
        let mut argv = desktop_entry::expand_exec(e, extra)?;
        if e.terminal {
            argv = in_terminal(argv)?;
        }
        self.spawner.detached(&argv, Some(&e.id), e.working_dir.as_deref().map(Path::new))
    }

    /// `open.url` / `open.path`: hand off to the desktop's default handler.
    pub fn open(&self, target: &str) -> Result<(), String> {
        self.spawner.detached(&["xdg-open".into(), target.into()], None, None)
    }

    /// Focus the editor if it is open, else start it.
    pub fn open_editor(&self) -> Result<(), String> {
        let ids: Vec<String> = EDITOR_WINDOW_IDS.iter().map(|s| s.to_string()).collect();
        if self.focus_any(&ids) {
            return Ok(());
        }
        let exe = find_sibling_or_path(EDITOR_BINARY).ok_or("the StreamPad editor (streampad) is not installed")?;
        self.spawner.detached(&[exe.to_string_lossy().into_owned()], Some(EDITOR_BINARY), None)
    }

    fn focus_any(&self, ids: &[String]) -> bool {
        let Some(kwin) = self.kwin.as_ref().filter(|k| k.available()) else { return false };
        if ids.is_empty() {
            return false;
        }
        self.rt.block_on(kwin.focus_app(ids)).unwrap_or_else(|e| {
            log::warn!("focus {ids:?}: {e}");
            false
        })
    }
}

/// Identifiers a running instance's windows may carry: the desktop id and its last dotted part, the
/// StartupWMClass and the program name.
fn window_ids(e: &DesktopEntry) -> Vec<String> {
    let mut ids = vec![e.id.clone()];
    if let Some((_, last)) = e.id.rsplit_once('.')
        && last.len() >= 3
    {
        ids.push(last.to_string());
    }
    ids.extend(e.startup_wm_class.clone());
    let program = e.exec.as_deref().and_then(|x| desktop_entry::split_exec(x).ok()).and_then(|a| a.into_iter().next());
    if let Some(name) = program.as_deref().map(program_name).filter(|n| !LAUNCH_WRAPPERS.contains(&n.as_str())) {
        ids.push(name);
    }
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(id.to_ascii_lowercase()));
    ids
}

fn program_name(path: &str) -> String {
    Path::new(path).file_name().map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
}

fn in_terminal(argv: Vec<String>) -> Result<Vec<String>, String> {
    let (term, flag) = TERMINALS
        .iter()
        .find(|(t, _)| find_in_path(t).is_some())
        .ok_or("this app needs a terminal, but none was found")?;
    let mut wrapped = vec![term.to_string()];
    wrapped.extend(flag.map(str::to_string));
    wrapped.extend(argv);
    Ok(wrapped)
}

/// `~`, `~/x` and relative paths are taken relative to the home directory.
pub fn expand_user_path(path: &str) -> PathBuf {
    let home = home_dir();
    match path.trim() {
        "~" => home,
        p if p.starts_with("~/") => home.join(&p[2..]),
        p if Path::new(p).is_absolute() => PathBuf::from(p),
        p => home.join(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_ids_cover_wm_class_and_program() {
        let e = DesktopEntry {
            id: "org.kde.dolphin".into(),
            path: PathBuf::from("/usr/share/applications/org.kde.dolphin.desktop"),
            name: "Dolphin".into(),
            icon: None,
            exec: Some("/usr/bin/dolphin %u".into()),
            working_dir: None,
            terminal: false,
            startup_wm_class: Some("Dolphin".into()),
        };
        assert_eq!(window_ids(&e), ["org.kde.dolphin", "dolphin"], "case-insensitive duplicates are dropped");
        let flatpak = DesktopEntry {
            id: "com.spotify.Client".into(),
            exec: Some("/usr/bin/flatpak run --branch=stable com.spotify.Client".into()),
            startup_wm_class: Some("Spotify".into()),
            ..e
        };
        assert_eq!(window_ids(&flatpak), ["com.spotify.Client", "Client", "Spotify"]);
    }

    #[test]
    fn user_paths() {
        let home = home_dir();
        assert_eq!(expand_user_path("~"), home);
        assert_eq!(expand_user_path("~/Videos"), home.join("Videos"));
        assert_eq!(expand_user_path("Videos"), home.join("Videos"));
        assert_eq!(expand_user_path("/tmp"), PathBuf::from("/tmp"));
    }
}
