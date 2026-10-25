//! Process environment: GPU workarounds applied before Tauri starts, XDG directory helpers, and the
//! development/test override detection shared by the service and autostart logic.

use std::env;
use std::path::{Path, PathBuf};

/// Env vars that point StreamPad at non-default directories (tests, development).
const PATH_OVERRIDES: [&str; 5] =
    ["STREAMPAD_CONFIG_DIR", "STREAMPAD_DATA_DIR", "STREAMPAD_STATE_DIR", "STREAMPAD_RUNTIME_DIR", "STREAMPAD_SOCKET"];

/// WebKitGTK's DMA-BUF renderer frequently paints a blank window or crashes on the proprietary NVIDIA
/// driver. Must run before any thread is spawned (it mutates the process environment).
pub fn apply_webkit_workarounds() {
    let nvidia = Path::new("/proc/driver/nvidia").exists();
    if nvidia && env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: called first thing in `main`, before Tauri or any other thread exists.
        unsafe { env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

/// True when any StreamPad path is overridden. In that mode the editor never touches the user's systemd
/// unit (which would run against the real directories) and spawns `streampadd` directly instead.
pub fn has_path_overrides() -> bool {
    PATH_OVERRIDES.iter().any(|k| env::var_os(k).is_some_and(|v| !v.is_empty()))
}

pub fn home() -> PathBuf {
    env::var_os("HOME").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg_dir(key: &str, fallback: &str) -> PathBuf {
    env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(fallback))
}

fn xdg_dirs(key: &str, fallback: &str) -> Vec<PathBuf> {
    let raw = env::var(key).ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| fallback.to_string());
    let mut dirs: Vec<PathBuf> = Vec::new();
    for d in raw.split(':').filter(|d| !d.is_empty()).map(PathBuf::from) {
        if d.is_absolute() && !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    dirs
}

pub fn config_home() -> PathBuf {
    xdg_dir("XDG_CONFIG_HOME", ".config")
}

pub fn data_home() -> PathBuf {
    xdg_dir("XDG_DATA_HOME", ".local/share")
}

/// `$XDG_CONFIG_HOME` followed by `$XDG_CONFIG_DIRS`, most important first.
pub fn config_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![config_home()];
    dirs.extend(xdg_dirs("XDG_CONFIG_DIRS", "/etc/xdg"));
    dirs
}

/// `$XDG_DATA_HOME`, `$XDG_DATA_DIRS`, then the flatpak export dirs (if not already listed).
pub fn data_search_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![data_home()];
    for d in xdg_dirs("XDG_DATA_DIRS", "/usr/local/share:/usr/share") {
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    for d in [data_home().join("flatpak/exports/share"), PathBuf::from("/var/lib/flatpak/exports/share")] {
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    dirs
}

/// First executable named `name` on `$PATH`.
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| env::split_paths(&paths).map(|d| d.join(name)).find(|p| is_executable(p)))
}

pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
