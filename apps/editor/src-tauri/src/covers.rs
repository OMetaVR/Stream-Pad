//! Image import for key covers and icons: files, dropped bytes and URLs are copied into
//! `<data dir>/<covers|icons>/<sha256>.<ext>` so configs stay portable and identical images are stored once.
//! Returns the AssetRef, e.g. `covers/<sha256>.<ext>`.

use crate::error::{CmdError, CmdResult, blocking};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use streampad_core::paths::Paths;
use tauri::ipc::{InvokeBody, Request};

/// Larger files are almost certainly a mistake (a video renamed to .gif, a RAW photo, …).
const MAX_COVER_BYTES: u64 = 64 * 1024 * 1024;

/// Where an imported image goes.
#[derive(Debug, Clone, Copy, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssetKind {
    #[default]
    Cover,
    Icon,
}

impl AssetKind {
    fn dir(self) -> &'static str {
        match self {
            AssetKind::Cover => "covers",
            AssetKind::Icon => "icons",
        }
    }

    fn from_header(v: Option<&str>) -> AssetKind {
        if v == Some("icon") { AssetKind::Icon } else { AssetKind::Cover }
    }
}

#[tauri::command]
pub async fn import_cover(path: String, kind: Option<AssetKind>) -> CmdResult<String> {
    let kind = kind.unwrap_or_default();
    blocking(move || {
        let src = PathBuf::from(&path);
        let size = fs::metadata(&src).map_err(|e| CmdError::new(format!("cannot read {path}: {e}")))?.len();
        check_size(size)?;
        let bytes = fs::read(&src).map_err(|e| CmdError::new(format!("cannot read {path}: {e}")))?;
        store(&bytes, src.file_name().and_then(|n| n.to_str()).unwrap_or_default(), kind)
    })
    .await
}

/// Download an image from an http(s) URL.
#[tauri::command]
pub async fn import_image_url(url: String, kind: Option<AssetKind>) -> CmdResult<String> {
    let kind = kind.unwrap_or_default();
    let url = url.trim().to_string();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(CmdError::new("enter an http(s) link to an image"));
    }
    blocking(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(30)))
            .user_agent(concat!("StreamPad/", env!("CARGO_PKG_VERSION"), " (+https://github.com/OMetaVR/Stream-Pad)"))
            .build()
            .into();
        let mut response = agent.get(&url).call().map_err(|e| CmdError::new(format!("couldn't download the image: {e}")))?;
        let bytes = response
            .body_mut()
            .with_config()
            .limit(MAX_COVER_BYTES)
            .read_to_vec()
            .map_err(|e| CmdError::new(format!("couldn't download the image: {e}")))?;
        let name = url.split(['?', '#']).next().and_then(|u| u.rsplit('/').next()).unwrap_or_default().to_string();
        store(&bytes, &name, kind)
    })
    .await
}

/// Raw-body variant for files dropped onto the webview. The original file name travels in the
/// `x-file-name` header (URI-encoded) and is only used as a hint when sniffing fails.
#[tauri::command]
pub async fn import_cover_bytes(request: Request<'_>) -> CmdResult<String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(CmdError::new("expected the image bytes as a raw request body"));
    };
    check_size(bytes.len() as u64)?;
    let name =
        request.headers().get("x-file-name").and_then(|v| v.to_str().ok()).map(percent_decode).unwrap_or_default();
    let kind = AssetKind::from_header(request.headers().get("x-asset-kind").and_then(|v| v.to_str().ok()));
    let bytes = bytes.clone();
    blocking(move || store(&bytes, &name, kind)).await
}

fn check_size(size: u64) -> CmdResult<()> {
    if size > MAX_COVER_BYTES {
        return Err(CmdError::new(format!("image is too large ({} MB, limit 64 MB)", size / (1024 * 1024))));
    }
    Ok(())
}

fn store(bytes: &[u8], name_hint: &str, kind: AssetKind) -> CmdResult<String> {
    let ext = sniff_extension(bytes, name_hint)
        .ok_or_else(|| CmdError::new("not a supported image (use PNG, JPEG, WebP, GIF or SVG)"))?;
    let file_name = format!("{}.{ext}", sha256_hex(bytes));
    let paths = Paths::resolve();
    let dest = paths.data_dir.join(kind.dir()).join(&file_name);
    if !dest.is_file() {
        streampad_core::write_atomic(&dest, bytes)?;
    }
    Ok(format!("{}/{file_name}", kind.dir()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Detect the image type from its magic bytes; SVG (text) is recognised by content or file extension.
fn sniff_extension(bytes: &[u8], name_hint: &str) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("webp");
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_ascii_lowercase();
    let hinted_svg = Path::new(name_hint).extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    if head.contains("<svg") || (hinted_svg && head.trim_start().starts_with('<')) {
        return Some("svg");
    }
    None
}

fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(hi << 4 | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_common_formats() {
        assert_eq!(sniff_extension(b"\x89PNG\r\n\x1a\nrest", ""), Some("png"));
        assert_eq!(sniff_extension(b"GIF89a....", "x.png"), Some("gif"));
        assert_eq!(sniff_extension(b"RIFF\0\0\0\0WEBPVP8 ", ""), Some("webp"));
        assert_eq!(sniff_extension(b"<?xml version='1.0'?><svg xmlns=''/>", ""), Some("svg"));
        assert_eq!(sniff_extension(b"hello", "notes.txt"), None);
    }

    #[test]
    fn hashes_and_decodes() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(percent_decode("my%20cover%2B1.gif"), "my cover+1.gif");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
