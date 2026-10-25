//! First-run default config (docs/SPEC.md §7): Linux/KDE ports of the AutoHotkey Stream-Pad profiles.
//!
//! Everything here is pure. The daemon looks up which desktop ids are installed (`APP_SLOTS` lists the
//! candidates) and calls [`default_config_with_apps`]; [`default_config`] is the lookup-free fallback the
//! editor uses, binding every Apps slot to its canonical id.

use crate::config::{Binding, Config, GraphEdge, Macro, Node, Position, Profile};
use crate::keys::KeyId;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// One key of the Apps profile: an `app.launch` with `focusIfRunning` (the AHK "ActivateOrLaunch").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSlot {
    pub key: KeyId,
    pub label: &'static str,
    /// Lucide icon name (without the `lucide:` prefix).
    pub icon: &'static str,
    /// Desktop ids in order of preference; the first is the canonical one from the spec.
    pub candidates: &'static [&'static str],
}

pub const APP_SLOTS: [AppSlot; 8] = [
    AppSlot {
        key: KeyId::KP1,
        label: "Discord",
        icon: "message-circle",
        candidates: &["discord", "com.discordapp.Discord", "vesktop", "dev.vencord.Vesktop", "equibop", "discord-canary"],
    },
    AppSlot {
        key: KeyId::KP2,
        label: "Firefox",
        icon: "compass",
        candidates: &["firefox", "org.mozilla.firefox", "firefox-esr", "firefox-developer-edition"],
    },
    AppSlot { key: KeyId::KP3, label: "Steam", icon: "gamepad", candidates: &["steam", "com.valvesoftware.Steam"] },
    AppSlot {
        key: KeyId::KP4,
        label: "Spotify",
        icon: "music",
        candidates: &["spotify", "com.spotify.Client", "spotify-launcher"],
    },
    AppSlot {
        key: KeyId::KP5,
        label: "System Monitor",
        icon: "activity",
        candidates: &["org.kde.plasma-systemmonitor", "org.kde.ksysguard", "org.gnome.SystemMonitor"],
    },
    AppSlot { key: KeyId::KP6, label: "Files", icon: "folder", candidates: &["org.kde.dolphin", "org.gnome.Nautilus"] },
    AppSlot {
        key: KeyId::KP7,
        label: "Terminal",
        icon: "square-terminal",
        candidates: &["org.kde.konsole", "org.gnome.Ptyxis", "org.gnome.Console"],
    },
    AppSlot {
        key: KeyId::KP8,
        label: "Audio Mixer",
        icon: "sliders-horizontal",
        candidates: &["org.pulseaudio.pavucontrol", "pavucontrol", "pavucontrol-qt", "com.saivert.pwvucontrol", "kcm_pulseaudio"],
    },
];

/// Apps matched by the Audio profile's "is a browser focused?" condition.
pub const BROWSER_MATCH: &str = "firefox,chromium,chrome,opera,brave,vivaldi,edge,zen,librewolf";

/// Default config with every Apps slot bound to its canonical desktop id (no installed-app lookup).
pub fn default_config() -> Config {
    let apps = APP_SLOTS.iter().map(|s| (s.key, s.candidates[0].to_string())).collect();
    default_config_with_apps(&apps)
}

/// Default config whose Apps profile binds exactly the given keys (key → installed desktop id).
/// Keys of `APP_SLOTS` missing from `apps` are left unbound.
pub fn default_config_with_apps(apps: &BTreeMap<KeyId, String>) -> Config {
    let mut cfg = Config {
        profiles: vec![audio_profile(), apps_profile(apps), web_profile(), windows_profile(), desktops_profile()],
        ..Config::default()
    };
    cfg.normalize();
    cfg
}

fn profile(name: &str, icon: &str, color: Option<&str>, keys: Vec<(KeyId, Binding)>) -> Profile {
    let mut p = Profile::new(name);
    p.icon = Some(lucide(icon));
    p.color = color.map(str::to_string);
    p.keys = keys.into_iter().collect();
    p
}

fn audio_profile() -> Profile {
    let browser_or_media = |combo: &str, media: &str| {
        Graph::new()
            .then("if", "flow.if", json!({ "condition": { "type": "activeApp", "match": BROWSER_MATCH } }))
            .branch("if", "true", "browser", "key.shortcut", json!({ "combo": combo }))
            .branch("if", "false", "media", "media", json!({ "action": media }))
            .build()
    };
    let game_mode = Graph::new()
        .then("toggle", "var.toggle", json!({ "name": "gameMode" }))
        .then("if", "flow.if", json!({ "condition": { "type": "var", "name": "gameMode", "op": "truthy" } }))
        .branch("if", "true", "on", "toast", json!({ "text": "Game mode: ON" }))
        .branch("if", "false", "off", "toast", json!({ "text": "Game mode: OFF" }))
        .build();
    profile(
        "Audio",
        "audio-lines",
        None,
        vec![
            (KeyId::KP5, binding("Play / Pause", "play", media("playPause"))),
            (KeyId::KP4, binding("Previous", "skip-back", browser_or_media("alt+left", "prev"))),
            (KeyId::KP6, binding("Next", "skip-forward", browser_or_media("alt+right", "next"))),
            (KeyId::KP8, repeating(binding("Volume +", "volume-2", media("volumeUp")))),
            (KeyId::KP2, repeating(binding("Volume −", "volume-1", media("volumeDown")))),
            (KeyId::KP0, binding("Mute", "volume-x", media("mute"))),
            (KeyId::KP3, binding("Game Mode", "gamepad-2", game_mode)),
        ],
    )
}

fn apps_profile(apps: &BTreeMap<KeyId, String>) -> Profile {
    let keys = APP_SLOTS
        .iter()
        .filter_map(|slot| {
            let app = apps.get(&slot.key)?;
            let launch = action("app.launch", json!({ "app": app, "focusIfRunning": true }));
            Some((slot.key, binding(slot.label, slot.icon, launch)))
        })
        .collect();
    profile("Apps", "layout-grid", Some("#5b9cf6"), keys)
}

fn web_profile() -> Profile {
    let site = |key, label, icon, url: &str| (key, binding(label, icon, action("open.url", json!({ "url": url }))));
    profile(
        "Web",
        "globe",
        Some("#3fbf9b"),
        vec![
            site(KeyId::KP1, "YouTube", "circle-play", "https://www.youtube.com/"),
            site(KeyId::KP2, "Google", "search", "https://www.google.com/"),
            site(KeyId::KP4, "GitHub", "git-branch", "https://github.com/"),
            site(KeyId::KP5, "Discord", "messages-square", "https://discord.com/app"),
            site(KeyId::KP7, "Twitch", "tv", "https://www.twitch.tv/"),
            site(KeyId::KP8, "Kick", "radio", "https://kick.com/"),
            site(KeyId::KP0, "ChatGPT", "sparkles", "https://chatgpt.com/"),
        ],
    )
}

fn windows_profile() -> Profile {
    let to_output = |key, label, icon, target: &str| {
        (key, binding(label, icon, action("window.moveToOutput", json!({ "target": target }))))
    };
    let window = |key, label, icon, act: &str| (key, binding(label, icon, action("window.action", json!({ "action": act }))));
    profile(
        "Windows",
        "app-window",
        Some("#f0a35b"),
        vec![
            to_output(KeyId::KP4, "Screen Left", "arrow-big-left", "left"),
            to_output(KeyId::KP6, "Screen Right", "arrow-big-right", "right"),
            to_output(KeyId::KP8, "Screen Up", "arrow-big-up", "up"),
            to_output(KeyId::KP2, "Screen Down", "arrow-big-down", "down"),
            window(KeyId::KP5, "Maximize", "maximize-2", "maximize"),
            window(KeyId::KP7, "Minimize", "minimize-2", "minimize"),
            window(KeyId::KP9, "Keep Above", "pin", "keepAbove"),
            window(KeyId::KP1, "Center", "crosshair", "center"),
            window(KeyId::KP3, "Fullscreen", "fullscreen", "fullscreen"),
        ],
    )
}

fn desktops_profile() -> Profile {
    let switch = |key, label, icon, target: &str| (key, binding(label, icon, action("desktop.switch", json!({ "target": target }))));
    let carry = |key, label, icon, target: &str| {
        (key, binding(label, icon, action("desktop.moveWindow", json!({ "target": target, "follow": true }))))
    };
    profile(
        "Desktops",
        "layers",
        Some("#e5729a"),
        vec![
            switch(KeyId::KP4, "Desktop Left", "arrow-left", "left"),
            switch(KeyId::KP6, "Desktop Right", "arrow-right", "right"),
            carry(KeyId::KP1, "Move Prev", "arrow-left-from-line", "prev"),
            carry(KeyId::KP3, "Move Next", "arrow-right-from-line", "next"),
            (KeyId::KP8, binding("New Desktop", "square-plus", action("desktop.create", json!({})))),
            (KeyId::KP2, binding("Remove Desktop", "square-minus", action("desktop.remove", json!({})))),
            (KeyId::KP5, binding("Overview", "layout-dashboard", action("desktop.overview", json!({})))),
        ],
    )
}

fn lucide(name: &str) -> String {
    format!("lucide:{name}")
}

fn binding(label: &str, icon: &str, m: Macro) -> Binding {
    Binding { label: label.into(), icon: Some(lucide(icon)), r#macro: Some(m), ..Binding::default() }
}

fn repeating(b: Binding) -> Binding {
    Binding { repeat: true, ..b }
}

fn media(act: &str) -> Macro {
    action("media", json!({ "action": act }))
}

/// `trigger.press → <kind>`: the shape the editor shows as a single action.
fn action(kind: &str, params: Value) -> Macro {
    Graph::new().then("action", kind, params).build()
}

/// Small builder for readable default graphs. Nodes are laid out left to right; branches fan out vertically.
struct Graph {
    nodes: Vec<Node>,
    edges: Vec<GraphEdge>,
    /// Id of the node the next `then` attaches to.
    tail: String,
}

const PRESS: &str = "press";
const COLUMN: f64 = 240.0;
const BRANCH_OFFSET: f64 = 90.0;

impl Graph {
    fn new() -> Self {
        let trigger = node(PRESS, "trigger.press", json!({}), Position::default());
        Graph { nodes: vec![trigger], edges: Vec::new(), tail: PRESS.into() }
    }

    /// Append `id` after the current tail via its `out` port.
    fn then(mut self, id: &str, kind: &str, params: Value) -> Self {
        let pos = self.position_of(&self.tail);
        self.push(id, kind, params, Position { x: pos.x + COLUMN, y: pos.y });
        let from = std::mem::replace(&mut self.tail, id.into());
        self.connect(&from, "out", id);
        self
    }

    /// Attach `id` to output `port` of the flow node `from`; `"true"` goes up, anything else down.
    fn branch(mut self, from: &str, port: &str, id: &str, kind: &str, params: Value) -> Self {
        let pos = self.position_of(from);
        let dy = if port == "true" { -BRANCH_OFFSET } else { BRANCH_OFFSET };
        self.push(id, kind, params, Position { x: pos.x + COLUMN, y: pos.y + dy });
        self.connect(from, port, id);
        self
    }

    fn build(self) -> Macro {
        Macro::Graph { nodes: self.nodes, edges: self.edges }
    }

    fn push(&mut self, id: &str, kind: &str, params: Value, position: Position) {
        self.nodes.push(node(id, kind, params, position));
    }

    fn connect(&mut self, source: &str, port: &str, target: &str) {
        self.edges.push(GraphEdge {
            id: format!("e_{source}_{port}"),
            source: source.into(),
            source_port: port.into(),
            target: target.into(),
        });
    }

    fn position_of(&self, id: &str) -> Position {
        self.nodes.iter().find(|n| n.id == id).map(|n| n.position).unwrap_or_default()
    }
}

fn node(id: &str, kind: &str, params: Value, position: Position) -> Node {
    let params = match params {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    Node { id: id.into(), kind: kind.into(), params, position }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::{CompileOptions, compile};
    use std::path::PathBuf;

    fn all_bindings(cfg: &Config) -> impl Iterator<Item = (&Profile, KeyId, &Binding)> {
        cfg.profiles.iter().flat_map(|p| p.keys.iter().map(move |(k, b)| (p, *k, b)))
    }

    #[test]
    fn five_profiles_in_spec_order() {
        let cfg = default_config();
        let names: Vec<&str> = cfg.profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Audio", "Apps", "Web", "Windows", "Desktops"]);
        let ids: std::collections::HashSet<&str> = cfg.profiles.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids.len(), 5, "profile ids must be unique");
        assert!(cfg.profiles.iter().all(|p| p.icon.as_deref().is_some_and(|i| i.starts_with("lucide:"))));
    }

    #[test]
    fn every_binding_has_label_icon_and_graph_macro() {
        let cfg = default_config();
        for (p, key, b) in all_bindings(&cfg) {
            assert!(!b.label.is_empty(), "{} {key}: missing label", p.name);
            assert!(b.icon.as_deref().is_some_and(|i| i.starts_with("lucide:")), "{} {key}: missing icon", p.name);
            assert!(matches!(b.r#macro, Some(Macro::Graph { .. })), "{} {key}: not a graph", p.name);
            assert!(!cfg.settings.globals.contains_key(&key), "{} {key}: bound on a global key", p.name);
        }
    }

    #[test]
    fn every_default_graph_compiles() {
        let cfg = default_config();
        for (p, key, b) in all_bindings(&cfg) {
            let m = b.r#macro.as_ref().expect("default bindings have macros");
            if let Err(e) = compile(m, &CompileOptions::default()) {
                panic!("{} {key} does not compile: {e} (node {:?})", p.name, e.node_id);
            }
        }
    }

    #[test]
    fn every_icon_exists_in_the_lucide_set() {
        let dir = std::env::var_os("STREAMPAD_SHARE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/share/streampad"))
            .join("icons/lucide");
        if !dir.is_dir() {
            eprintln!("skipping: lucide icons not installed at {}", dir.display());
            return;
        }
        let cfg = default_config();
        let icons = cfg
            .profiles
            .iter()
            .filter_map(|p| p.icon.clone())
            .chain(all_bindings(&cfg).filter_map(|(_, _, b)| b.icon.clone()));
        for icon in icons {
            let name = icon.strip_prefix("lucide:").expect("lucide icon");
            assert!(dir.join(format!("{name}.svg")).is_file(), "missing lucide icon {name}");
        }
    }

    #[test]
    fn apps_profile_binds_only_found_apps() {
        let found = BTreeMap::from([(KeyId::KP6, "org.kde.dolphin".to_string()), (KeyId::KP1, "vesktop".to_string())]);
        let cfg = default_config_with_apps(&found);
        let apps = &cfg.profiles[1];
        let bound: std::collections::BTreeSet<KeyId> = apps.keys.keys().copied().collect();
        assert_eq!(bound, [KeyId::KP1, KeyId::KP6].into());
        let Some(Macro::Graph { nodes, edges }) = &apps.keys[&KeyId::KP1].r#macro else { panic!("graph expected") };
        let launch = nodes.iter().find(|n| n.kind == "app.launch").expect("launch node");
        assert_eq!(launch.params["app"], "vesktop");
        assert_eq!(launch.params["focusIfRunning"], true);
        assert_eq!(edges.len(), 1);
    }

    #[test]
    fn fallback_uses_canonical_spec_ids() {
        let cfg = default_config();
        let apps = &cfg.profiles[1];
        assert_eq!(apps.keys.len(), 8);
        let Some(Macro::Graph { nodes, .. }) = &apps.keys[&KeyId::KP8].r#macro else { panic!("graph expected") };
        assert!(nodes.iter().any(|n| n.params.get("app") == Some(&json!("org.pulseaudio.pavucontrol"))));
    }

    #[test]
    fn audio_details_match_spec() {
        let cfg = default_config();
        let audio = &cfg.profiles[0];
        assert!(audio.keys[&KeyId::KP8].repeat && audio.keys[&KeyId::KP2].repeat);
        assert!(!audio.keys[&KeyId::KP5].repeat);
        let Some(Macro::Graph { nodes, edges }) = &audio.keys[&KeyId::KP3].r#macro else { panic!("graph expected") };
        let kinds: Vec<&str> = nodes.iter().map(|n| n.kind.as_str()).collect();
        assert_eq!(kinds, ["trigger.press", "var.toggle", "flow.if", "toast", "toast"]);
        let ports: Vec<&str> = edges.iter().map(|e| e.source_port.as_str()).collect();
        assert_eq!(ports, ["out", "out", "true", "false"]);
        let Some(Macro::Graph { nodes, .. }) = &audio.keys[&KeyId::KP4].r#macro else { panic!("graph expected") };
        assert_eq!(nodes[1].params["condition"]["type"], "activeApp");
        assert_eq!(nodes[2].params["combo"], "alt+left");
        assert_eq!(nodes[3].params["action"], "prev");
    }
}
