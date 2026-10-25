//! The main window: created in code (not tauri.conf.json) so it can carry a boot script with the resolved
//! StreamPad paths — `backend.assetUrl()` is synchronous and needs them before the first render.

use crate::env;
use serde_json::json;
use streampad_core::paths::Paths;
use tauri::webview::Color;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const MAIN: &str = "main";

/// `--sp-bg` of the dark theme; painted before the page loads so there is no white flash.
const BACKGROUND: Color = Color(12, 13, 16, 255);

pub fn create_main(app: &AppHandle) -> tauri::Result<()> {
    let paths = Paths::resolve();
    let boot = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "dataDir": paths.data_dir,
        "shareDir": paths.share_dir,
        "home": env::home(),
        "configFile": paths.config_file(),
    });
    let script = format!(
        "Object.defineProperty(window, \"__STREAMPAD__\", {{ value: Object.freeze({boot}), configurable: false }});"
    );
    WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
        .title("StreamPad")
        .inner_size(1280.0, 820.0)
        .min_inner_size(960.0, 640.0)
        .background_color(BACKGROUND)
        .initialization_script(script)
        .build()?;
    Ok(())
}

/// Bring the existing window forward (second launch, daemon "open editor").
pub fn focus_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Extend the asset protocol scope (static part in tauri.conf.json) with directories that depend on the
/// environment: the StreamPad data and share dirs (which tests relocate) and every XDG icon dir.
pub fn allow_asset_dirs(app: &AppHandle) {
    let paths = Paths::resolve();
    let mut dirs = vec![paths.data_dir.clone(), env::home().join(".icons")];
    dirs.extend(paths.share_dir.clone());
    for d in env::data_search_dirs() {
        dirs.push(d.join("icons"));
        dirs.push(d.join("pixmaps"));
    }
    let scope = app.asset_protocol_scope();
    for d in dirs {
        if let Err(e) = scope.allow_directory(&d, true) {
            log::warn!("cannot allow asset dir {}: {e}", d.display());
        }
    }
}
