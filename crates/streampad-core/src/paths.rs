//! Filesystem locations (docs/SPEC.md §3.6). Every location can be overridden by an env var so tests never
//! touch the real user's directories.

use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub state_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub socket: PathBuf,
    /// Installed shared assets (lucide icons, …), if any were found.
    pub share_dir: Option<PathBuf>,
}

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn home() -> PathBuf {
    env_path("HOME").unwrap_or_else(|| PathBuf::from("/tmp"))
}

fn xdg(key: &str, fallback: &str) -> PathBuf {
    env_path(key).unwrap_or_else(|| home().join(fallback))
}

impl Paths {
    pub fn resolve() -> Paths {
        let config_dir = env_path("STREAMPAD_CONFIG_DIR").unwrap_or_else(|| xdg("XDG_CONFIG_HOME", ".config").join("streampad"));
        let data_dir = env_path("STREAMPAD_DATA_DIR").unwrap_or_else(|| xdg("XDG_DATA_HOME", ".local/share").join("streampad"));
        let state_dir = env_path("STREAMPAD_STATE_DIR").unwrap_or_else(|| xdg("XDG_STATE_HOME", ".local/state").join("streampad"));
        let runtime_dir = env_path("STREAMPAD_RUNTIME_DIR").unwrap_or_else(|| {
            env_path("XDG_RUNTIME_DIR")
                .unwrap_or_else(|| env::temp_dir().join(format!("streampad-{}", unsafe_uid())))
                .join("streampad")
        });
        let socket = env_path("STREAMPAD_SOCKET").unwrap_or_else(|| runtime_dir.join("daemon.sock"));
        let share_dir = Self::find_share_dir(&data_dir);
        Paths { config_dir, data_dir, state_dir, runtime_dir, socket, share_dir }
    }

    fn find_share_dir(data_dir: &Path) -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(p) = env_path("STREAMPAD_SHARE_DIR") {
            candidates.push(p);
        }
        if let Ok(exe) = env::current_exe() {
            if let Some(bin) = exe.parent() {
                candidates.push(bin.join("../share/streampad"));
                // dev layout: target/<profile>/streampadd → <repo>/build/share/streampad
                candidates.push(bin.join("../../build/share/streampad"));
                candidates.push(bin.join("../../../build/share/streampad"));
            }
        }
        candidates.push(data_dir.join("share"));
        candidates.push(PathBuf::from("/usr/local/share/streampad"));
        candidates.push(PathBuf::from("/usr/share/streampad"));
        candidates.into_iter().find(|p| p.join("icons").is_dir()).map(|p| p.canonicalize().unwrap_or(p))
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.json")
    }
    pub fn state_file(&self) -> PathBuf {
        self.state_dir.join("state.json")
    }
    pub fn covers_dir(&self) -> PathBuf {
        self.data_dir.join("covers")
    }
    pub fn kwin_scripts_dir(&self) -> PathBuf {
        self.runtime_dir.join("kwin")
    }

    /// `<share>/icons/lucide/<name>.svg` if it exists.
    pub fn lucide_icon(&self, name: &str) -> Option<PathBuf> {
        if name.is_empty() || name.contains('/') || name.contains("..") {
            return None;
        }
        let p = self.share_dir.as_ref()?.join("icons/lucide").join(format!("{name}.svg"));
        p.is_file().then_some(p)
    }

    /// AssetRef → absolute path (relative refs are relative to the data dir).
    pub fn resolve_asset(&self, asset: &str) -> PathBuf {
        let p = Path::new(asset);
        if p.is_absolute() {
            p.to_path_buf()
        } else if let Some(rest) = asset.strip_prefix("~/") {
            home().join(rest)
        } else {
            self.data_dir.join(p)
        }
    }

    /// IconRef → (absolute path, tint). `None` if the icon can't be found.
    pub fn resolve_icon(&self, icon: &str) -> Option<(PathBuf, bool)> {
        if let Some(name) = icon.strip_prefix("lucide:") {
            return self.lucide_icon(name).map(|p| (p, true));
        }
        let p = self.resolve_asset(icon);
        p.is_file().then_some((p, false))
    }
}

fn unsafe_uid() -> u32 {
    // Avoid a libc dependency: read the real uid from /proc.
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Uid:")).and_then(|l| l.split_whitespace().nth(1)?.parse().ok()))
        .unwrap_or(0)
}
