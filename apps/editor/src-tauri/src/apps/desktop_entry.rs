//! Installed applications from `.desktop` files (freedesktop Desktop Entry spec): precedence by data dir,
//! `Hidden` / `NoDisplay` / `OnlyShowIn` / `NotShowIn` / `TryExec`, and localized names.

use crate::env;
use crate::ini::{self, Group};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct DesktopApp {
    /// Desktop file id without ".desktop" (`org.kde.dolphin`).
    pub id: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub wm_class: Option<String>,
}

/// What the current session looks like, for entry filtering.
pub struct Session {
    desktops: Vec<String>,
    locales: Vec<String>,
}

impl Session {
    pub fn from_env() -> Session {
        let desktops = std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .split(':')
            .filter(|d| !d.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
            .unwrap_or_default();
        Session { desktops, locales: locale_variants(&locale) }
    }

    fn shows_in(&self, list: &str) -> bool {
        ini::list(list).iter().any(|d| self.desktops.contains(&d.to_ascii_lowercase()))
    }

    fn localized(&self, group: &Group, key: &str) -> Option<String> {
        self.locales
            .iter()
            .find_map(|l| group.get(&format!("{key}[{l}]")))
            .or_else(|| group.get(key))
            .map(|v| ini::unescape(v))
            .filter(|v| !v.trim().is_empty())
    }
}

/// `en_US.UTF-8@euro` → `["en_US@euro", "en_US", "en@euro", "en"]` (desktop entry matching order).
fn locale_variants(locale: &str) -> Vec<String> {
    if locale.is_empty() || locale == "C" || locale == "POSIX" {
        return Vec::new();
    }
    let (rest, modifier) = match locale.split_once('@') {
        Some((r, m)) => (r, Some(m)),
        None => (locale, None),
    };
    let rest = rest.split('.').next().unwrap_or(rest);
    let (lang, country) = match rest.split_once('_') {
        Some((l, c)) => (l, Some(c)),
        None => (rest, None),
    };
    let mut out = Vec::new();
    if let (Some(c), Some(m)) = (country, modifier) {
        out.push(format!("{lang}_{c}@{m}"));
    }
    if let Some(c) = country {
        out.push(format!("{lang}_{c}"));
    }
    if let Some(m) = modifier {
        out.push(format!("{lang}@{m}"));
    }
    out.push(lang.to_string());
    out
}

fn application_dirs() -> Vec<PathBuf> {
    env::data_search_dirs().into_iter().map(|d| d.join("applications")).filter(|d| d.is_dir()).collect()
}

/// All visible applications, deduplicated by desktop id (the first file found for an id decides).
pub fn scan(session: &Session) -> Vec<DesktopApp> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut apps = Vec::new();
    for dir in application_dirs() {
        let mut files = Vec::new();
        collect_desktop_files(&dir, &dir, 0, &mut files);
        for (id, path) in files {
            if seen.contains(&id) {
                continue;
            }
            // An unreadable file (e.g. a dangling symlink in ~/.local/share/applications) must not shadow
            // a working entry with the same id further down the search path.
            let Some(groups) = ini::parse_file(&path) else { continue };
            seen.insert(id.clone());
            if let Some(app) = groups.get("Desktop Entry").and_then(|g| parse_entry(id, g, session)) {
                apps.push(app);
            }
        }
    }
    apps
}

/// Desktop file ids are paths relative to the `applications` dir with `/` replaced by `-`.
fn collect_desktop_files(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if depth < 3 {
                collect_desktop_files(root, &path, depth + 1, out);
            }
            continue;
        }
        if path.extension().is_none_or(|e| e != "desktop") {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else { continue };
        let id = rel.with_extension("").to_string_lossy().replace('/', "-");
        out.push((id, path));
    }
}

fn parse_entry(id: String, g: &Group, session: &Session) -> Option<DesktopApp> {
    if g.get("Type").map(String::as_str) != Some("Application") {
        return None;
    }
    if ini::is_true(g, "NoDisplay") || ini::is_true(g, "Hidden") {
        return None;
    }
    if g.get("OnlyShowIn").is_some_and(|l| !session.shows_in(l)) {
        return None;
    }
    if g.get("NotShowIn").is_some_and(|l| session.shows_in(l)) {
        return None;
    }
    if g.get("TryExec").is_some_and(|t| !try_exec_ok(&ini::unescape(t))) {
        return None;
    }
    if !g.contains_key("Exec") && !ini::is_true(g, "DBusActivatable") {
        return None;
    }
    let name = session.localized(g, "Name")?;
    Some(DesktopApp {
        id,
        name,
        generic_name: session.localized(g, "GenericName"),
        comment: session.localized(g, "Comment"),
        icon: g.get("Icon").map(|v| ini::unescape(v)).filter(|v| !v.trim().is_empty()),
        categories: g.get("Categories").map(|v| ini::list(v)).unwrap_or_default(),
        wm_class: g.get("StartupWMClass").map(|v| ini::unescape(v).to_lowercase()).filter(|v| !v.is_empty()),
    })
}

fn try_exec_ok(program: &str) -> bool {
    let p = Path::new(program);
    if p.is_absolute() { env::is_executable(p) } else { env::find_in_path(program).is_some() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(desktop: &str, locale: &str) -> Session {
        Session { desktops: vec![desktop.to_ascii_lowercase()], locales: locale_variants(locale) }
    }

    fn entry(text: &str, s: &Session) -> Option<DesktopApp> {
        parse_entry("test".into(), &ini::parse(text)["Desktop Entry"], s)
    }

    #[test]
    fn locale_matching_order() {
        assert_eq!(locale_variants("de_DE.UTF-8"), vec!["de_DE", "de"]);
        assert_eq!(locale_variants("sr_RS@latin"), vec!["sr_RS@latin", "sr_RS", "sr@latin", "sr"]);
        assert!(locale_variants("C").is_empty());
    }

    #[test]
    fn localized_and_filtered() {
        let s = session("KDE", "de_DE.UTF-8");
        let app = entry(
            "[Desktop Entry]\nType=Application\nName=Files\nName[de]=Dateien\nExec=files\nCategories=System;FileManager;\nStartupWMClass=Files\n",
            &s,
        )
        .unwrap();
        assert_eq!(app.name, "Dateien");
        assert_eq!(app.categories, vec!["System", "FileManager"]);
        assert_eq!(app.wm_class.as_deref(), Some("files"));
        assert!(entry("[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n", &s).is_none());
        assert!(entry("[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=GNOME;\n", &s).is_none());
        assert!(entry("[Desktop Entry]\nType=Application\nName=X\nExec=x\nNotShowIn=KDE;\n", &s).is_none());
        assert!(entry("[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=KDE;\n", &s).is_some());
        assert!(entry("[Desktop Entry]\nType=Link\nName=X\nURL=https://x\n", &s).is_none());
        assert!(entry("[Desktop Entry]\nType=Application\nName=X\nExec=x\nTryExec=/nonexistent/bin\n", &s).is_none());
    }
}
