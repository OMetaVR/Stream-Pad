//! config.json commands. The editor is the only writer; the daemon hot-reloads the file.

use crate::error::{CmdResult, blocking};
use streampad_core::Config;
use streampad_core::defaults::default_config;
use streampad_core::paths::Paths;

/// Load config.json, creating it from the first-run defaults when missing. A file that exists but does
/// not parse is reported as an error and left untouched, so a hand-edit typo never loses the user's data.
#[tauri::command]
pub async fn load_config() -> CmdResult<Config> {
    blocking(|| {
        let path = Paths::resolve().config_file();
        if let Some(cfg) = Config::load(&path)? {
            return Ok(cfg);
        }
        let cfg = default_config();
        cfg.save_atomic(&path)?;
        log::info!("created default config at {}", path.display());
        Ok(cfg)
    })
    .await
}

#[tauri::command]
pub async fn save_config(config: Config) -> CmdResult<()> {
    blocking(move || {
        let path = Paths::resolve().config_file();
        config.save_atomic(&path)?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn config_path() -> String {
    Paths::resolve().config_file().display().to_string()
}
