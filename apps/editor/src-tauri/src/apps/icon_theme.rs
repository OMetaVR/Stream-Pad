//! Icon name → file resolution following the freedesktop icon theme spec closely enough for app icons:
//! the current theme (KDE `kdeglobals [Icons] Theme`, else GTK's setting, else Breeze), its `Inherits`
//! chain, hicolor, then `pixmaps`. Within a theme, SVG beats PNG and PNGs close to 128 px (≥ 64) win.

use crate::env;
use crate::ini::{self, Group};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const FALLBACK_THEME: &str = "hicolor";

pub struct IconLookup {
    /// Indexed themes in lookup order.
    themes: Vec<HashMap<String, Candidate>>,
    pixmap_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct Candidate {
    score: u32,
    path: PathBuf,
}

impl IconLookup {
    pub fn new() -> IconLookup {
        let bases = icon_base_dirs();
        let chain = theme_chain(&bases, &current_theme_name());
        let themes = chain.iter().map(|t| index_theme(&bases, t)).filter(|t| !t.is_empty()).collect();
        let pixmap_dirs =
            env::data_search_dirs().into_iter().map(|d| d.join("pixmaps")).filter(|d| d.is_dir()).collect();
        IconLookup { themes, pixmap_dirs }
    }

    /// Resolve a desktop entry `Icon=` value (name or absolute path) to a displayable file.
    pub fn find(&self, icon: &str) -> Option<PathBuf> {
        let icon = icon.trim();
        let as_path = Path::new(icon);
        if as_path.is_absolute() {
            return (as_path.is_file() && is_displayable(as_path)).then(|| as_path.to_path_buf());
        }
        let name = strip_image_extension(icon);
        if let Some(c) = self.themes.iter().find_map(|t| t.get(name)) {
            return Some(c.path.clone());
        }
        self.pixmap_dirs
            .iter()
            .flat_map(|d| ["svg", "png"].map(|ext| d.join(format!("{name}.{ext}"))))
            .find(|p| p.is_file())
    }
}

fn is_displayable(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "svg" | "png" | "jpg" | "jpeg" | "webp" | "gif"))
}

fn strip_image_extension(name: &str) -> &str {
    for ext in [".png", ".svg", ".xpm", ".svgz"] {
        if let Some(stem) = name.strip_suffix(ext) {
            return stem;
        }
    }
    name
}

/// `~/.icons`, then `<data dir>/icons` for every XDG data dir.
fn icon_base_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![env::home().join(".icons")];
    dirs.extend(env::data_search_dirs().into_iter().map(|d| d.join("icons")));
    dirs.retain(|d| d.is_dir());
    dirs
}

fn current_theme_name() -> String {
    let kde = env::config_search_dirs().into_iter().find_map(|d| {
        ini::parse_file(&d.join("kdeglobals"))?.get("Icons")?.get("Theme").cloned().filter(|t| !t.is_empty())
    });
    let gtk = || {
        ini::parse_file(&env::config_home().join("gtk-3.0/settings.ini"))?
            .get("Settings")?
            .get("gtk-icon-theme-name")
            .map(|t| t.trim_matches('"').to_string())
            .filter(|t| !t.is_empty())
    };
    kde.or_else(gtk).unwrap_or_else(|| "breeze".into())
}

fn theme_index(bases: &[PathBuf], theme: &str) -> Option<HashMap<String, Group>> {
    bases.iter().find_map(|b| ini::parse_file(&b.join(theme).join("index.theme")))
}

/// The theme, its (transitive) `Inherits`, and hicolor last.
fn theme_chain(bases: &[PathBuf], theme: &str) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    let mut queue = vec![theme.to_string()];
    while let Some(t) = queue.pop() {
        if chain.contains(&t) || t == FALLBACK_THEME || chain.len() > 16 {
            continue;
        }
        let Some(index) = theme_index(bases, &t) else { continue };
        let parents = index.get("Icon Theme").and_then(|g| g.get("Inherits")).cloned().unwrap_or_default();
        chain.push(t);
        // Depth-first in declared order: push reversed so the first parent is visited next.
        queue.extend(parents.split(',').map(str::trim).filter(|p| !p.is_empty()).rev().map(String::from));
    }
    chain.push(FALLBACK_THEME.into());
    chain
}

struct SubdirInfo {
    effective_size: u32,
}

fn subdir_info(group: Option<&Group>) -> SubdirInfo {
    let num = |k: &str| group.and_then(|g| g.get(k)).and_then(|v| v.parse::<u32>().ok());
    let size = num("Size").unwrap_or(48);
    let scale = num("Scale").unwrap_or(1).max(1);
    let scalable = group.and_then(|g| g.get("Type")).is_some_and(|t| t == "Scalable");
    let nominal = if scalable { num("MaxSize").unwrap_or(size).max(size) } else { size };
    SubdirInfo { effective_size: nominal.min(1024) * scale }
}

/// Higher is better: any SVG (bigger nominal size first — small Breeze sizes are simplified glyphs), then
/// PNGs ≥ 64 px closest to 128 px, then smaller PNGs.
fn score(ext: &str, size: u32) -> Option<u32> {
    match ext {
        "svg" => Some(3_000_000 + size),
        "png" if size >= 64 => Some(2_000_000 - size.abs_diff(128)),
        "png" => Some(1_000_000 + size),
        _ => None,
    }
}

/// name → best file for one theme, merged across every base dir that contains the theme.
fn index_theme(bases: &[PathBuf], theme: &str) -> HashMap<String, Candidate> {
    let mut icons: HashMap<String, Candidate> = HashMap::new();
    let Some(index) = theme_index(bases, theme) else { return icons };
    let header = index.get("Icon Theme");
    let mut subdirs: Vec<String> = Vec::new();
    for key in ["Directories", "ScaledDirectories"] {
        if let Some(list) = header.and_then(|g| g.get(key)) {
            subdirs.extend(list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from));
        }
    }
    for sub in &subdirs {
        let info = subdir_info(index.get(sub));
        for base in bases {
            let Ok(entries) = fs::read_dir(base.join(theme).join(sub)) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                let (Some(stem), Some(ext)) =
                    (path.file_stem().and_then(|s| s.to_str()), path.extension().and_then(|e| e.to_str()))
                else {
                    continue;
                };
                let Some(score) = score(ext, info.effective_size) else { continue };
                if icons.get(stem).is_none_or(|c| score > c.score) {
                    icons.insert(stem.to_string(), Candidate { score, path });
                }
            }
        }
    }
    icons
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_svg_then_large_png() {
        let svg48 = score("svg", 48).unwrap();
        let svg16 = score("svg", 16).unwrap();
        let png128 = score("png", 128).unwrap();
        let png256 = score("png", 256).unwrap();
        let png48 = score("png", 48).unwrap();
        assert!(svg48 > svg16 && svg16 > png128);
        assert!(png128 > png256 && png256 > png48);
        assert!(score("xpm", 48).is_none());
    }

    #[test]
    fn strips_extensions() {
        assert_eq!(strip_image_extension("steam.png"), "steam");
        assert_eq!(strip_image_extension("org.kde.dolphin"), "org.kde.dolphin");
    }

    #[test]
    fn resolves_from_a_fake_theme() {
        let root = std::env::temp_dir().join(format!("sp-icons-{}", std::process::id()));
        let theme = root.join("fake");
        fs::create_dir_all(theme.join("48x48/apps")).unwrap();
        fs::create_dir_all(theme.join("scalable/apps")).unwrap();
        fs::write(
            theme.join("index.theme"),
            "[Icon Theme]\nName=Fake\nDirectories=48x48/apps,scalable/apps\n\n[48x48/apps]\nSize=48\n\n[scalable/apps]\nSize=64\nType=Scalable\n",
        )
        .unwrap();
        fs::write(theme.join("48x48/apps/tool.png"), b"png").unwrap();
        fs::write(theme.join("scalable/apps/tool.svg"), b"<svg/>").unwrap();
        fs::write(theme.join("48x48/apps/only-png.png"), b"png").unwrap();
        let index = index_theme(std::slice::from_ref(&root), "fake");
        assert!(index["tool"].path.ends_with("scalable/apps/tool.svg"));
        assert!(index["only-png"].path.ends_with("48x48/apps/only-png.png"));
        fs::remove_dir_all(&root).unwrap();
    }
}
