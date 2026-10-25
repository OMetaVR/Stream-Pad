//! Loading, validating, first-run creation and change watching of `config.json`.

use notify::{EventKind, RecursiveMode, Watcher};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use streampad_core::paths::Paths;
use streampad_core::{Config, KeyId, combo, defaults};

/// Read + normalize + validate. `Ok(None)` = the file does not exist.
pub fn load(paths: &Paths) -> Result<Option<Config>, String> {
    let cfg = Config::load(&paths.config_file()).map_err(|e| e.to_string())?;
    if let Some(cfg) = &cfg {
        validate(cfg)?;
    }
    Ok(cfg)
}

/// Problems `Config::normalize` cannot repair.
fn validate(cfg: &Config) -> Result<(), String> {
    let toggle = &cfg.settings.toggle_key;
    if combo::key_code(toggle).is_none() {
        return Err(format!("invalid config.json: unknown toggle key \"{toggle}\""));
    }
    Ok(())
}

/// Startup load. A missing file is created from the defaults (Apps bound to the ids `installed_apps`
/// finds). An invalid file is left untouched for the user to fix; the defaults run meanwhile and the error is
/// returned for `configError`.
pub fn load_or_create(paths: &Paths, installed_apps: impl FnOnce() -> BTreeMap<KeyId, String>) -> (Config, Option<String>) {
    match load(paths) {
        Ok(Some(cfg)) => (cfg, None),
        Ok(None) => {
            let cfg = defaults::default_config_with_apps(&installed_apps());
            let path = paths.config_file();
            match cfg.save_atomic(&path) {
                Ok(()) => log::info!("created default config at {}", path.display()),
                Err(e) => log::error!("cannot write default config to {}: {e}", path.display()),
            }
            (cfg, None)
        }
        Err(message) => {
            log::error!("{message}; running with the default profiles until it is fixed");
            (defaults::default_config_with_apps(&installed_apps()), Some(message))
        }
    }
}

/// Calls `on_change` (from the watcher thread) whenever `config.json` may have changed. The config directory
/// is watched rather than the file so atomic replace-by-rename is seen. Keep the returned watcher alive.
pub fn watch(paths: &Paths, on_change: impl Fn() + Send + 'static) -> notify::Result<notify::RecommendedWatcher> {
    let dir = paths.config_dir.clone();
    std::fs::create_dir_all(&dir).map_err(notify::Error::io)?;
    let file_name = paths.config_file().file_name().map(OsStr::to_owned);
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
        Ok(ev) => {
            let relevant = !matches!(ev.kind, EventKind::Access(_))
                && (ev.paths.is_empty() || ev.paths.iter().any(|p| p.file_name() == file_name.as_deref()));
            if relevant {
                on_change();
            }
        }
        Err(e) => log::warn!("config watcher: {e}"),
    })?;
    watcher.watch(&dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn paths_in(dir: &Path) -> Paths {
        Paths {
            config_dir: dir.join("config"),
            data_dir: dir.join("data"),
            state_dir: dir.join("state"),
            runtime_dir: dir.join("run"),
            socket: dir.join("run/daemon.sock"),
            share_dir: None,
        }
    }

    #[test]
    fn first_run_creates_defaults_with_found_apps() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let (cfg, err) = load_or_create(&paths, || BTreeMap::from([(KeyId::KP6, "org.kde.dolphin".into())]));
        assert!(err.is_none());
        assert_eq!(cfg.profiles.len(), 5);
        assert_eq!(cfg.profiles[1].keys.len(), 1);
        let reloaded = load(&paths).unwrap().expect("file was written");
        assert_eq!(reloaded, cfg);
    }

    #[test]
    fn invalid_file_is_reported_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(&paths.config_dir).unwrap();
        std::fs::write(paths.config_file(), r#"{"settings":{"toggleKey":"NoSuchKey"}}"#).unwrap();
        let (cfg, err) = load_or_create(&paths, BTreeMap::new);
        assert!(err.unwrap().contains("NoSuchKey"));
        assert_eq!(cfg.profiles.len(), 5);
        let on_disk = std::fs::read_to_string(paths.config_file()).unwrap();
        assert!(on_disk.contains("NoSuchKey"), "a broken file must not be overwritten");

        std::fs::write(paths.config_file(), "{ trailing garbage").unwrap();
        assert!(load(&paths).unwrap_err().contains("invalid config.json"));
    }
}
