//! Render-ready views of the config for the overlay and editor (docs/SPEC.md §4.2 `RenderProfile`).

use std::collections::BTreeMap;
use streampad_core::ipc::{ProfileSummary, RenderKey, RenderKeyKind, RenderProfile};
use streampad_core::paths::Paths;
use streampad_core::{Config, GlobalAction, GlobalBinding, KeyId, Profile};

/// A key carries a global if either of its actions does something.
pub fn active_global(cfg: &Config, key: KeyId) -> Option<GlobalBinding> {
    cfg.settings
        .globals
        .get(&key)
        .copied()
        .filter(|g| g.tap != GlobalAction::None || g.hold.is_some_and(|h| h != GlobalAction::None))
}

/// The action a global key is presented (and tapped) as.
pub fn presented_action(g: GlobalBinding) -> GlobalAction {
    match (g.tap, g.hold) {
        (GlobalAction::None, Some(hold)) => hold,
        (tap, _) => tap,
    }
}

fn icon_path(paths: &Paths, icon: Option<&str>) -> (Option<String>, bool) {
    match icon.and_then(|i| paths.resolve_icon(i)) {
        Some((path, tint)) => (Some(path.display().to_string()), tint),
        None => (None, false),
    }
}

fn cover_path(paths: &Paths, cover: Option<&str>) -> Option<String> {
    let path = paths.resolve_asset(cover?);
    path.is_file().then(|| path.display().to_string())
}

pub fn render_key(cfg: &Config, profile: &Profile, key: KeyId, paths: &Paths) -> RenderKey {
    let legend = key.legend();
    if let Some(global) = active_global(cfg, key) {
        let action = presented_action(global);
        let (label, icon) = action.presentation();
        let (icon, icon_tint) = icon_path(paths, Some(&format!("lucide:{icon}")));
        return RenderKey {
            label: label.into(),
            icon,
            icon_tint,
            cover: None,
            cover_crop: None,
            color: None,
            kind: RenderKeyKind::Global,
            global: Some(action),
            legend,
        };
    }
    match profile.keys.get(&key) {
        Some(b) => {
            let (icon, icon_tint) = icon_path(paths, b.icon.as_deref());
            let cover = cover_path(paths, b.cover.as_deref());
            RenderKey {
                label: b.label.clone(),
                icon,
                icon_tint,
                cover_crop: cover.as_ref().and(b.cover_crop),
                cover,
                color: b.color.clone(),
                kind: if b.r#macro.is_some() { RenderKeyKind::Macro } else { RenderKeyKind::Empty },
                global: None,
                legend,
            }
        }
        None => RenderKey {
            label: String::new(),
            icon: None,
            icon_tint: false,
            cover: None,
            cover_crop: None,
            color: None,
            kind: RenderKeyKind::Empty,
            global: None,
            legend,
        },
    }
}

pub fn render_profile(cfg: &Config, index: usize, paths: &Paths) -> RenderProfile {
    let profile = &cfg.profiles[index];
    let keys: BTreeMap<KeyId, RenderKey> = KeyId::ALL.into_iter().map(|k| (k, render_key(cfg, profile, k, paths))).collect();
    RenderProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        color: profile.color.clone(),
        icon: icon_path(paths, profile.icon.as_deref()).0,
        keys,
    }
}

pub fn profile_summaries(cfg: &Config, paths: &Paths) -> Vec<ProfileSummary> {
    cfg.profiles
        .iter()
        .map(|p| ProfileSummary {
            id: p.id.clone(),
            name: p.name.clone(),
            color: p.color.clone(),
            icon: icon_path(paths, p.icon.as_deref()).0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use streampad_core::{Binding, defaults};

    fn paths_with_share(share: Option<PathBuf>, data: PathBuf) -> Paths {
        Paths {
            config_dir: data.join("config"),
            data_dir: data.clone(),
            state_dir: data.join("state"),
            runtime_dir: data.join("run"),
            socket: data.join("run/daemon.sock"),
            share_dir: share,
        }
    }

    #[test]
    fn all_keys_present_with_globals_and_resolved_icons() {
        let share = tempfile::tempdir().unwrap();
        let lucide = share.path().join("icons/lucide");
        std::fs::create_dir_all(&lucide).unwrap();
        for name in ["chevron-up", "chevron-down", "eye", "play", "audio-lines"] {
            std::fs::write(lucide.join(format!("{name}.svg")), "<svg/>").unwrap();
        }
        let data = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(data.path().join("covers")).unwrap();
        std::fs::write(data.path().join("covers/a.gif"), b"GIF89a").unwrap();
        let paths = paths_with_share(Some(share.path().to_path_buf()), data.path().to_path_buf());

        let mut cfg = defaults::default_config();
        cfg.profiles[0].keys.insert(
            KeyId::KP7,
            Binding { label: "Clip".into(), cover: Some("covers/a.gif".into()), ..Binding::default() },
        );
        let r = render_profile(&cfg, 0, &paths);
        assert_eq!(r.keys.len(), 17);
        assert!(r.icon.as_deref().is_some_and(|p| p.ends_with("audio-lines.svg")));

        let plus = &r.keys[&KeyId::KPPlus];
        assert_eq!((plus.kind, plus.global, plus.label.as_str()), (RenderKeyKind::Global, Some(GlobalAction::NextProfile), "Next"));
        assert!(plus.icon.as_deref().is_some_and(|p| p.ends_with("chevron-down.svg")) && plus.icon_tint);
        assert_eq!(r.keys[&KeyId::KPEnter].label, "Show / Hide");

        let play = &r.keys[&KeyId::KP5];
        assert_eq!(play.kind, RenderKeyKind::Macro);
        assert!(play.icon.as_deref().is_some_and(|p| p.ends_with("play.svg")));
        assert_eq!(play.legend.main, "5");

        let clip = &r.keys[&KeyId::KP7];
        assert_eq!(clip.kind, RenderKeyKind::Empty, "no macro = empty");
        assert_eq!(clip.cover.as_deref(), Some(data.path().join("covers/a.gif").to_str().unwrap()));

        let missing_icon = &r.keys[&KeyId::KP8];
        assert_eq!((missing_icon.icon.as_deref(), missing_icon.icon_tint), (None, false));
        assert_eq!(r.keys[&KeyId::KP9].kind, RenderKeyKind::Empty);
        assert_eq!(r.keys[&KeyId::KP4].legend.sub.as_deref(), Some("←"));
    }

    #[test]
    fn hold_only_global_presents_its_hold_action() {
        let mut cfg = Config::default();
        cfg.settings.globals.insert(KeyId::KPDot, GlobalBinding { tap: GlobalAction::None, hold: Some(GlobalAction::OpenEditor) });
        cfg.settings.globals.insert(KeyId::KPSlash, GlobalBinding { tap: GlobalAction::None, hold: None });
        assert_eq!(active_global(&cfg, KeyId::KPDot).map(presented_action), Some(GlobalAction::OpenEditor));
        assert_eq!(active_global(&cfg, KeyId::KPSlash), None);
    }
}
