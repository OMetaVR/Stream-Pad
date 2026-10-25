//! Minimal parser for the freedesktop "desktop entry" key file format, which is also used by icon theme
//! `index.theme` files and KDE's `kdeglobals`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub type Group = HashMap<String, String>;

/// `[Group]` → key → raw value. The first occurrence of a group or key wins; comments and malformed lines
/// are skipped. KDE key modifiers such as `Theme[$e]` are stripped to `Theme`.
pub fn parse(text: &str) -> HashMap<String, Group> {
    let mut groups: HashMap<String, Group> = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = Some(name.to_string());
            groups.entry(name.to_string()).or_default();
            continue;
        }
        let (Some(group), Some((key, value))) = (current.as_ref(), line.split_once('=')) else {
            continue;
        };
        let key = strip_kde_modifier(key.trim());
        if let Some(g) = groups.get_mut(group) {
            g.entry(key.to_string()).or_insert_with(|| value.trim().to_string());
        }
    }
    groups
}

pub fn parse_file(path: &Path) -> Option<HashMap<String, Group>> {
    fs::read_to_string(path).ok().map(|t| parse(&t))
}

fn strip_kde_modifier(key: &str) -> &str {
    match key.find("[$") {
        Some(i) if key.ends_with(']') => &key[..i],
        _ => key,
    }
}

/// Unescape a desktop-entry string value (`\s`, `\n`, `\t`, `\r`, `\\`).
pub fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Split a `;`-separated list value (honouring `\;`), dropping empty items.
pub fn list(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&';') => {
                cur.push(';');
                chars.next();
            }
            ';' => items.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    items.push(cur);
    items.into_iter().map(|s| unescape(s.trim())).filter(|s| !s.is_empty()).collect()
}

/// Truthy desktop-entry boolean.
pub fn is_true(group: &Group, key: &str) -> bool {
    group.get(key).is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_groups_and_first_key_wins() {
        let g =
            parse("# c\n[Desktop Entry]\nName=Files\nName=Other\nName[de]=Dateien\n\n[Desktop Action new]\nName=New\n");
        assert_eq!(g["Desktop Entry"]["Name"], "Files");
        assert_eq!(g["Desktop Entry"]["Name[de]"], "Dateien");
        assert_eq!(g["Desktop Action new"]["Name"], "New");
    }

    #[test]
    fn strips_kde_modifiers() {
        let g = parse("[Icons]\nTheme[$e]=breeze-dark\n");
        assert_eq!(g["Icons"]["Theme"], "breeze-dark");
    }

    #[test]
    fn lists_and_escapes() {
        assert_eq!(list("AudioVideo;Audio;;"), vec!["AudioVideo", "Audio"]);
        assert_eq!(list(r"a\;b;c"), vec!["a;b", "c"]);
        assert_eq!(unescape(r"a\sb\\c"), "a b\\c");
    }
}
