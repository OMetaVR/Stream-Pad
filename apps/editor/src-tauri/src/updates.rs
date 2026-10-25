//! Updates from GitHub releases: check the latest release, and install it the way StreamPad was installed —
//! the Arch package via `pkexec pacman -U` (one polkit password prompt), a `~/.local` install via the
//! release tarball's own installer (no root). Every download is verified against the release's SHA256SUMS.
//! Progress is reported to the webview as `streampad:update-progress` events.

use crate::env;
use crate::error::{CmdError, CmdResult, blocking};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const REPO: &str = "OMetaVR/Stream-Pad";
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;
const PROGRESS_EVENT: &str = "streampad:update-progress";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallMethod {
    /// Installed by pacman (`/usr/bin/streampad` owned by the `streampad` package).
    Pacman,
    /// Installed for the user (release tarball or `make install`) under `~/.local`.
    User,
    /// Anything else (a development build, a custom prefix): updates are manual.
    Manual,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub name: String,
    pub notes: String,
    pub url: String,
    pub published_at: String,
    pub method: InstallMethod,
    /// The asset `install_update` would use for this install method, if the release has one.
    pub asset: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    phase: &'static str,
    received: u64,
    total: u64,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(300)))
        .user_agent(concat!("StreamPad/", env!("CARGO_PKG_VERSION"), " (+https://github.com/OMetaVR/Stream-Pad)"))
        .build()
        .into()
}

fn latest_release() -> CmdResult<Value> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut res = agent()
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| CmdError::new(format!("couldn't reach GitHub: {e}")))?;
    let text = res.body_mut().read_to_string().map_err(|e| CmdError::new(format!("couldn't read GitHub's answer: {e}")))?;
    serde_json::from_str(&text).map_err(|e| CmdError::new(format!("unexpected answer from GitHub: {e}")))
}

/// "v2.1.0" / "2.1.0-rc1" → (2, 1, 0). Missing parts are 0.
fn parse_version(v: &str) -> (u64, u64, u64) {
    let core = v.trim().trim_start_matches('v').split(['-', '+']).next().unwrap_or("");
    let mut parts = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

fn install_method() -> InstallMethod {
    let Ok(exe) = std::env::current_exe().and_then(fs::canonicalize) else { return InstallMethod::Manual };
    if exe.starts_with("/usr/")
        && Command::new("pacman").args(["-Qoq"]).arg(&exe).output().is_ok_and(|o| o.status.success() && o.stdout.starts_with(b"streampad"))
    {
        return InstallMethod::Pacman;
    }
    if exe.starts_with(env::home().join(".local/bin")) {
        return InstallMethod::User;
    }
    InstallMethod::Manual
}

fn asset_suffix(method: InstallMethod) -> Option<String> {
    let arch = std::env::consts::ARCH;
    match method {
        InstallMethod::Pacman => Some(format!("-{arch}.pkg.tar.zst")),
        InstallMethod::User => Some(format!("-linux-{arch}.tar.gz")),
        InstallMethod::Manual => None,
    }
}

fn assets(release: &Value) -> Vec<(String, String, u64)> {
    release["assets"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| Some((x["name"].as_str()?.to_string(), x["browser_download_url"].as_str()?.to_string(), x["size"].as_u64().unwrap_or(0))))
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
pub async fn check_update() -> CmdResult<UpdateInfo> {
    blocking(|| {
        let release = latest_release()?;
        let latest = release["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
        if latest.is_empty() {
            return Err(CmdError::new("the latest release has no version tag"));
        }
        let current = env!("CARGO_PKG_VERSION").to_string();
        let method = install_method();
        let asset = asset_suffix(method).and_then(|suffix| assets(&release).into_iter().map(|a| a.0).find(|n| n.ends_with(&suffix)));
        Ok(UpdateInfo {
            available: parse_version(&latest) > parse_version(&current),
            name: release["name"].as_str().unwrap_or("").to_string(),
            notes: release["body"].as_str().unwrap_or("").to_string(),
            url: release["html_url"].as_str().unwrap_or("").to_string(),
            published_at: release["published_at"].as_str().unwrap_or("").to_string(),
            current,
            latest,
            method,
            asset,
        })
    })
    .await
}

/// Download, verify and install the latest release. On success the background service is already running
/// the new version; the editor itself picks it up after `restart_editor`.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> CmdResult<String> {
    blocking(move || {
        let method = install_method();
        let suffix = asset_suffix(method).ok_or_else(|| CmdError::new("this copy of StreamPad wasn't installed from a release — update it the way you installed it"))?;
        let release = latest_release()?;
        let version = release["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
        let assets = assets(&release);
        let (name, url, size) = assets.iter().find(|a| a.0.ends_with(&suffix)).cloned().ok_or_else(|| CmdError::new(format!("release {version} has no {suffix} download")))?;
        let sums = assets.iter().find(|a| a.0 == "SHA256SUMS").ok_or_else(|| CmdError::new("release has no SHA256SUMS to verify the download"))?;

        let dir = std::env::temp_dir().join(format!("streampad-update-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir)?;
        let file = dir.join(&name);
        let emit = |phase: &'static str, received: u64, total: u64| {
            let _ = app.emit(PROGRESS_EVENT, Progress { phase, received, total });
        };

        let digest = download(&url, &file, size, |r, t| emit("download", r, t))?;
        let expected = expected_hash(&sums.1, &name)?;
        if !digest.eq_ignore_ascii_case(&expected) {
            return Err(CmdError::new("the download didn't match its checksum — try again"));
        }

        emit("install", 0, 0);
        let result = match method {
            InstallMethod::Pacman => install_package(&file),
            InstallMethod::User => install_tarball(&dir, &file),
            InstallMethod::Manual => unreachable!(),
        };
        let _ = fs::remove_dir_all(&dir);
        result?;
        emit("done", 0, 0);
        Ok(version)
    })
    .await
}

#[tauri::command]
pub fn restart_editor(app: AppHandle) {
    app.restart();
}

/// Streams `url` into `dest` and returns its sha256 (hex).
fn download(url: &str, dest: &Path, size: u64, progress: impl Fn(u64, u64)) -> CmdResult<String> {
    let mut res = agent().get(url).call().map_err(|e| CmdError::new(format!("download failed: {e}")))?;
    let mut reader = res.body_mut().with_config().limit(MAX_DOWNLOAD).reader();
    let mut out = fs::File::create(dest)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut received = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| CmdError::new(format!("download failed: {e}")))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        received += n as u64;
        progress(received, size);
    }
    out.sync_all()?;
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn expected_hash(sums_url: &str, name: &str) -> CmdResult<String> {
    let mut res = agent().get(sums_url).call().map_err(|e| CmdError::new(format!("couldn't fetch checksums: {e}")))?;
    let text = res.body_mut().read_to_string().map_err(|e| CmdError::new(format!("couldn't fetch checksums: {e}")))?;
    text.lines()
        .filter_map(|l| l.split_once(char::is_whitespace))
        .find(|(_, file)| file.trim().trim_start_matches('*') == name)
        .map(|(hash, _)| hash.to_string())
        .ok_or_else(|| CmdError::new(format!("SHA256SUMS has no entry for {name}")))
}

fn run(cmd: &mut Command, what: &str) -> CmdResult<()> {
    let out = cmd.output().map_err(|e| CmdError::new(format!("{what}: {e}")))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    Err(CmdError::new(if last.is_empty() { format!("{what} failed ({})", out.status) } else { format!("{what} failed: {last}") }))
}

fn install_package(pkg: &Path) -> CmdResult<()> {
    // pkexec shows the desktop's password dialog; the package's post_upgrade hook prints nothing we need.
    run(Command::new("pkexec").arg("pacman").args(["-U", "--noconfirm"]).arg(pkg), "installing the package")?;
    run(Command::new("systemctl").args(["--user", "restart", "streampad.service"]), "restarting StreamPad")
}

fn install_tarball(dir: &Path, tarball: &Path) -> CmdResult<()> {
    run(Command::new("tar").arg("-xzf").arg(tarball).arg("-C").arg(dir), "unpacking the update")?;
    let installer = fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path().join("install.sh"))
        .find(|p| p.is_file())
        .ok_or_else(|| CmdError::new("the update has no installer"))?;
    // The installer restarts the service if it was running and leaves it stopped otherwise.
    run(Command::new(&installer).arg("--no-start").current_dir(installer.parent().unwrap_or(dir)), "installing the update")
}

/// Exposed for tests.
#[cfg(test)]
pub(crate) fn newer(a: &str, b: &str) -> bool {
    parse_version(a) > parse_version(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(newer("v2.1.0", "2.0.0"));
        assert!(newer("2.0.10", "2.0.9"));
        assert!(!newer("2.0.0", "2.0.0"));
        assert!(!newer("v1.9.9", "2.0.0"));
        assert!(!newer("2.0.0-rc1", "2.0.0"));
        assert_eq!(parse_version("v3"), (3, 0, 0));
    }

    /// Network test against the real latest release: `cargo test -- --ignored release`.
    #[test]
    #[ignore = "downloads the latest release from GitHub"]
    fn latest_release_downloads_and_verifies() {
        let release = latest_release().unwrap();
        let assets = assets(&release);
        let suffix = asset_suffix(InstallMethod::User).unwrap();
        let (name, url, size) = assets.iter().find(|a| a.0.ends_with(&suffix)).cloned().expect("tarball asset");
        let sums = assets.iter().find(|a| a.0 == "SHA256SUMS").expect("SHA256SUMS asset");
        let dir = std::env::temp_dir().join(format!("streampad-update-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(&name);
        let digest = download(&url, &file, size, |_, _| {}).unwrap();
        assert_eq!(fs::metadata(&file).unwrap().len(), size);
        assert_eq!(digest, expected_hash(&sums.1, &name).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }
}
