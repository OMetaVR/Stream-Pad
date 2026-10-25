//! `config.json` schema (see docs/SPEC.md §3). Every struct is tolerant: unknown fields are ignored and
//! missing fields take their defaults, so older/newer files keep loading.

use crate::keys::KeyId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub version: u32,
    pub settings: Settings,
    pub profiles: Vec<Profile>,
    /// Shared JS prepended to every macro.
    pub library: String,
    /// Icons the user saved for reuse (IconRefs), newest first.
    pub saved_icons: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: CONFIG_VERSION,
            settings: Settings::default(),
            profiles: vec![Profile::new("Default")],
            library: String::new(),
            saved_icons: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub toggle_key: String,
    pub start_in_macro_mode: bool,
    pub remember_state: bool,
    pub unbound_keys: UnboundKeys,
    pub hold_ms: u32,
    pub globals: BTreeMap<KeyId, GlobalBinding>,
    pub devices: DeviceSettings,
    pub overlay: OverlaySettings,
    pub toast_on_error: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            toggle_key: "ScrollLock".into(),
            start_in_macro_mode: false,
            remember_state: true,
            unbound_keys: UnboundKeys::Block,
            hold_ms: 600,
            globals: default_globals(),
            devices: DeviceSettings::default(),
            overlay: OverlaySettings::default(),
            toast_on_error: true,
        }
    }
}

pub fn default_globals() -> BTreeMap<KeyId, GlobalBinding> {
    BTreeMap::from([
        (KeyId::KPEnter, GlobalBinding { tap: GlobalAction::ToggleOverlay, hold: Some(GlobalAction::OpenEditor) }),
        (KeyId::KPPlus, GlobalBinding { tap: GlobalAction::NextProfile, hold: None }),
        (KeyId::KPMinus, GlobalBinding { tap: GlobalAction::PrevProfile, hold: None }),
    ])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UnboundKeys {
    #[default]
    Block,
    Passthrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalBinding {
    pub tap: GlobalAction,
    #[serde(default)]
    pub hold: Option<GlobalAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GlobalAction {
    ToggleOverlay,
    ShowOverlay,
    HideOverlay,
    NextProfile,
    PrevProfile,
    FirstProfile,
    ToggleMacroMode,
    OpenEditor,
    None,
}

impl GlobalAction {
    /// Default (label, lucide icon) shown on a key that carries this global.
    pub fn presentation(self) -> (&'static str, &'static str) {
        match self {
            GlobalAction::ToggleOverlay => ("Show / Hide", "eye"),
            GlobalAction::ShowOverlay => ("Show", "eye"),
            GlobalAction::HideOverlay => ("Hide", "eye-off"),
            GlobalAction::NextProfile => ("Next", "chevron-down"),
            GlobalAction::PrevProfile => ("Prev", "chevron-up"),
            GlobalAction::FirstProfile => ("First", "chevrons-up"),
            GlobalAction::ToggleMacroMode => ("Numpad", "keyboard"),
            GlobalAction::OpenEditor => ("Editor", "settings-2"),
            GlobalAction::None => ("", "circle-off"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceMode {
    #[default]
    Auto,
    Manual,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceSettings {
    pub mode: DeviceMode,
    /// Manual mode: device ids to grab.
    pub include: Vec<String>,
    /// Auto mode: device ids never to grab.
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Edge {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Align {
    Start,
    Center,
    #[default]
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LegendStyle {
    #[default]
    Full,
    Number,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OverlaySettings {
    pub edge: Edge,
    pub align: Align,
    pub offset: i32,
    pub monitor: Option<String>,
    pub scale: f64,
    pub theme: Theme,
    pub accent: String,
    pub opacity: f64,
    pub blur: bool,
    pub show_header: bool,
    pub show_legends: bool,
    pub legend_style: LegendStyle,
    pub show_labels: bool,
    pub animation_ms: u32,
    pub peek_on_mode_change: bool,
    pub peek_on_profile_change: bool,
    pub peek_ms: u32,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        OverlaySettings {
            edge: Edge::Bottom,
            align: Align::End,
            offset: 0,
            monitor: None,
            scale: 1.0,
            theme: Theme::Dark,
            accent: "#8b7cf6".into(),
            opacity: 0.94,
            blur: true,
            show_header: true,
            show_legends: true,
            legend_style: LegendStyle::Full,
            show_labels: true,
            animation_ms: 320,
            peek_on_mode_change: true,
            peek_on_profile_change: true,
            peek_ms: 1600,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub keys: BTreeMap<KeyId, Binding>,
}

impl Default for Profile {
    fn default() -> Self {
        Profile::new("Profile")
    }
}

impl Profile {
    pub fn new(name: &str) -> Self {
        Profile { id: new_id("p"), name: name.into(), color: None, icon: None, keys: BTreeMap::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Binding {
    pub label: String,
    pub icon: Option<String>,
    pub cover: Option<String>,
    /// Part of the cover image shown on the key; `None` = centred "cover" fit. The cover file itself is never
    /// modified, so a crop can always be widened again.
    pub cover_crop: Option<CropRect>,
    pub color: Option<String>,
    pub r#macro: Option<Macro>,
    pub repeat: bool,
    pub concurrency: Concurrency,
}

impl Default for Binding {
    fn default() -> Self {
        Binding {
            label: String::new(),
            icon: None,
            cover: None,
            cover_crop: None,
            color: None,
            r#macro: None,
            repeat: false,
            concurrency: Concurrency::Ignore,
        }
    }
}

/// A rectangle in normalized image coordinates (0–1 of the image's width / height).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl CropRect {
    /// Clamp into the image; `None` if degenerate or (almost) the whole image.
    pub fn normalized(self) -> Option<CropRect> {
        let ok = |v: f64| v.is_finite();
        if !(ok(self.x) && ok(self.y) && ok(self.w) && ok(self.h)) {
            return None;
        }
        let w = self.w.clamp(0.0, 1.0);
        let h = self.h.clamp(0.0, 1.0);
        if w < 0.01 || h < 0.01 {
            return None;
        }
        Some(CropRect { x: self.x.clamp(0.0, 1.0 - w), y: self.y.clamp(0.0, 1.0 - h), w, h })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Concurrency {
    #[default]
    Ignore,
    Restart,
    Parallel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Macro {
    Graph {
        #[serde(default)]
        nodes: Vec<Node>,
        #[serde(default)]
        edges: Vec<GraphEdge>,
    },
    Script {
        #[serde(default)]
        source: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub params: Map<String, Value>,
    #[serde(default)]
    pub position: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    #[serde(default = "default_port")]
    pub source_port: String,
    pub target: String,
}

fn default_port() -> String {
    "out".into()
}

/// Random short id with a prefix, e.g. `p_3f9c2a71`.
pub fn new_id(prefix: &str) -> String {
    format!("{prefix}_{:08x}", fastrand::u32(..))
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("invalid config.json: {0}")]
    Parse(#[from] serde_json::Error),
}

impl Config {
    /// Read + parse + normalize. Returns `Ok(None)` when the file does not exist.
    pub fn load(path: &Path) -> Result<Option<Config>, ConfigError> {
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(ConfigError::Io { path: path.display().to_string(), source: e }),
        };
        let mut cfg: Config = serde_json::from_str(&text)?;
        cfg.normalize();
        Ok(Some(cfg))
    }

    /// Pretty JSON, written atomically (tmp file + fsync + rename) so readers never see half a file.
    pub fn save_atomic(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        write_atomic(path, json.as_bytes())
    }

    /// Repair anything that would break the runtime: ensure ≥1 profile, unique non-empty ids,
    /// clamp numeric ranges, drop bindings on global keys.
    pub fn normalize(&mut self) {
        if self.profiles.is_empty() {
            self.profiles.push(Profile::new("Default"));
        }
        let mut seen = std::collections::HashSet::new();
        for p in &mut self.profiles {
            if p.id.trim().is_empty() || !seen.insert(p.id.clone()) {
                p.id = new_id("p");
                seen.insert(p.id.clone());
            }
            for b in p.keys.values_mut() {
                b.cover_crop = if b.cover.is_some() { b.cover_crop.and_then(CropRect::normalized) } else { None };
            }
            for k in self.settings.globals.keys() {
                if self.settings.globals[k].tap != GlobalAction::None {
                    p.keys.remove(k);
                }
            }
        }
        let s = &mut self.settings;
        s.hold_ms = s.hold_ms.clamp(150, 5000);
        let o = &mut s.overlay;
        o.scale = if o.scale.is_finite() { o.scale.clamp(0.5, 2.0) } else { 1.0 };
        o.opacity = if o.opacity.is_finite() { o.opacity.clamp(0.3, 1.0) } else { 0.94 };
        o.animation_ms = o.animation_ms.clamp(0, 2000);
        o.peek_ms = o.peek_ms.clamp(300, 10_000);
        if o.monitor.as_deref().is_some_and(|m| m.trim().is_empty()) {
            o.monitor = None;
        }
        self.version = CONFIG_VERSION;
    }

    pub fn profile_index(&self, id: &str) -> Option<usize> {
        self.profiles.iter().position(|p| p.id == id)
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_roundtrip() {
        let c = Config::default();
        let s = serde_json::to_string(&c).unwrap();
        let back: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(c, back);
        assert!(s.contains("\"toggleKey\":\"ScrollLock\""));
        assert!(s.contains("\"KPEnter\":{\"tap\":\"toggleOverlay\",\"hold\":\"openEditor\"}"));
    }

    #[test]
    fn tolerant_parse() {
        let c: Config = serde_json::from_str(
            r#"{"profiles":[{"name":"A","keys":{"KP4":{"label":"x","macro":{"type":"script","source":"press('a')"}}}}],"unknown":1}"#,
        )
        .unwrap();
        assert_eq!(c.profiles[0].keys[&KeyId::KP4].label, "x");
        assert_eq!(c.settings.overlay.peek_ms, 1600);
        assert!(matches!(c.profiles[0].keys[&KeyId::KP4].r#macro, Some(Macro::Script { .. })));
    }

    #[test]
    fn normalize_repairs() {
        let mut c: Config = serde_json::from_str(
            r#"{"profiles":[{"id":"a","keys":{"KPEnter":{"label":"nope"}}},{"id":"a"}],"settings":{"overlay":{"scale":99}}}"#,
        )
        .unwrap();
        c.normalize();
        assert_ne!(c.profiles[0].id, c.profiles[1].id);
        assert!(!c.profiles[0].keys.contains_key(&KeyId::KPEnter));
        assert_eq!(c.settings.overlay.scale, 2.0);
    }
}
