//! XDG desktop entries: lookup by desktop-file id and `Exec` parsing (Desktop Entry Specification 1.5).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopEntry {
    /// Desktop-file id without ".desktop", e.g. "org.kde.dolphin".
    pub id: String,
    pub path: PathBuf,
    pub name: String,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub working_dir: Option<String>,
    pub terminal: bool,
    pub startup_wm_class: Option<String>,
}

/// `$XDG_DATA_HOME/applications` then each `$XDG_DATA_DIRS/applications`, in priority order.
pub fn application_dirs() -> Vec<PathBuf> {
    let home = super::spawn::home_dir();
    let data_home = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()).map_or_else(|| home.join(".local/share"), PathBuf::from);
    let data_dirs = std::env::var("XDG_DATA_DIRS").ok().filter(|v| !v.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let mut dirs = vec![data_home.join("applications")];
    for d in data_dirs.split(':').filter(|d| !d.is_empty()) {
        let dir = Path::new(d).join("applications");
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

/// Exact lookup: the first directory that has `<id>.desktop` decides (a `Hidden=true` entry there means the
/// app was removed). Ids with dashes may live in subdirectories (`kde4-foo` = `kde4/foo.desktop`).
pub fn find_exact(id: &str) -> Option<DesktopEntry> {
    let id = id.strip_suffix(".desktop").unwrap_or(id);
    if id.is_empty() || id.contains('/') || id.starts_with('.') {
        return None;
    }
    for dir in application_dirs() {
        for rel in id_paths(id) {
            let path = dir.join(&rel);
            if path.is_file()
                && let Some(entry) = parse_file(id, &path)
            {
                return Some(entry);
            }
        }
    }
    None
}

/// Exact id, else a case-insensitive id, else an id ending in ".<name>" (so "discord" finds
/// "com.discordapp.Discord").
pub fn find(name: &str) -> Option<DesktopEntry> {
    if let Some(e) = find_exact(name) {
        return Some(e);
    }
    let wanted = name.strip_suffix(".desktop").unwrap_or(name).to_ascii_lowercase();
    let suffix = format!(".{wanted}");
    let ids = all_ids();
    let pick = ids
        .keys()
        .find(|id| id.to_ascii_lowercase() == wanted)
        .or_else(|| ids.keys().find(|id| id.to_ascii_lowercase().ends_with(&suffix)))?;
    parse_file(pick, &ids[pick])
}

fn id_paths(id: &str) -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from(format!("{id}.desktop"))];
    for (i, _) in id.match_indices('-') {
        paths.push(PathBuf::from(&id[..i]).join(format!("{}.desktop", &id[i + 1..])));
    }
    paths
}

/// Every desktop-file id on the system (first directory wins), for fuzzy lookups.
fn all_ids() -> HashMap<String, PathBuf> {
    let mut ids = HashMap::new();
    for dir in application_dirs() {
        collect_ids(&dir, "", &mut ids, 0);
    }
    ids
}

fn collect_ids(dir: &Path, prefix: &str, ids: &mut HashMap<String, PathBuf>, depth: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() && depth < 2 {
            collect_ids(&path, &format!("{prefix}{name}-"), ids, depth + 1);
        } else if let Some(stem) = name.strip_suffix(".desktop") {
            ids.entry(format!("{prefix}{stem}")).or_insert(path);
        }
    }
}

fn parse_file(id: &str, path: &Path) -> Option<DesktopEntry> {
    let text = std::fs::read_to_string(path).ok()?;
    parse(id, path, &text)
}

/// Parse the `[Desktop Entry]` group. `None` for hidden or non-application entries.
pub fn parse(id: &str, path: &Path, text: &str) -> Option<DesktopEntry> {
    let mut in_main = false;
    let mut keys: HashMap<&str, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            // Localized keys (`Name[de]`) are skipped: the untranslated value is all we need.
            let k = k.trim();
            if !k.contains('[') {
                keys.entry(k).or_insert_with(|| unescape_value(v.trim()));
            }
        }
    }
    let flag = |k: &str| keys.get(k).is_some_and(|v| v == "true");
    if flag("Hidden") || keys.get("Type").is_some_and(|t| t != "Application") {
        return None;
    }
    let get = |k: &str| keys.get(k).filter(|v| !v.is_empty()).cloned();
    Some(DesktopEntry {
        id: id.to_string(),
        path: path.to_path_buf(),
        name: get("Name").unwrap_or_else(|| id.to_string()),
        icon: get("Icon"),
        exec: get("Exec"),
        working_dir: get("Path"),
        terminal: flag("Terminal"),
        startup_wm_class: get("StartupWMClass"),
    })
}

/// The "string" escapes of the spec: `\s \n \t \r \\`.
fn unescape_value(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    let mut chars = v.chars();
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
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Split an `Exec` value into arguments (double quotes; `\"`, `` \` ``, `\$`, `\\` escapes inside them).
pub fn split_exec(exec: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut in_arg = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_arg = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(e @ ('"' | '`' | '$' | '\\')) => cur.push(e),
                            Some(other) => {
                                cur.push('\\');
                                cur.push(other);
                            }
                            None => return Err("unterminated escape in Exec".into()),
                        },
                        Some(ch) => cur.push(ch),
                        None => return Err("unterminated quote in Exec".into()),
                    }
                }
            }
            ' ' | '\t' => {
                if in_arg {
                    args.push(std::mem::take(&mut cur));
                    in_arg = false;
                }
            }
            _ => {
                cur.push(c);
                in_arg = true;
            }
        }
    }
    if in_arg {
        args.push(cur);
    }
    Ok(args)
}

/// Expand field codes. `targets` (files/URLs) replace `%f %u %F %U`; when the Exec line has none of those,
/// the targets are appended so extra arguments are never silently dropped.
pub fn expand_exec(entry: &DesktopEntry, targets: &[String]) -> Result<Vec<String>, String> {
    let exec = entry.exec.as_deref().ok_or_else(|| format!("{} has no Exec line", entry.id))?;
    let mut out = Vec::new();
    let mut used_targets = false;
    for arg in split_exec(exec)? {
        match arg.as_str() {
            "%F" | "%U" => {
                out.extend(targets.iter().cloned());
                used_targets = true;
            }
            "%f" | "%u" => {
                out.extend(targets.first().cloned());
                used_targets = true;
            }
            "%i" => {
                if let Some(icon) = &entry.icon {
                    out.push("--icon".into());
                    out.push(icon.clone());
                }
            }
            _ => {
                let expanded = expand_inline(&arg, entry, targets.first(), &mut used_targets);
                if !expanded.is_empty() || !arg.contains('%') {
                    out.push(expanded);
                }
            }
        }
    }
    if !used_targets {
        out.extend(targets.iter().cloned());
    }
    if out.is_empty() {
        return Err(format!("{} has an empty Exec line", entry.id));
    }
    Ok(out)
}

fn expand_inline(arg: &str, entry: &DesktopEntry, first: Option<&String>, used_targets: &mut bool) -> String {
    let mut out = String::new();
    let mut chars = arg.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => out.push('%'),
            Some('f' | 'u' | 'F' | 'U') => {
                if let Some(t) = first {
                    out.push_str(t);
                }
                *used_targets = true;
            }
            Some('c') => out.push_str(&entry.name),
            Some('k') => out.push_str(&entry.path.to_string_lossy()),
            // %i is only meaningful as a whole argument; deprecated codes (%d %D %n %N %v %m) are dropped.
            _ => {}
        }
    }
    out
}

/// Split user-supplied extra arguments shell-style (whitespace, single and double quotes).
pub fn split_args(s: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut in_arg = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                in_arg = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_arg {
                    args.push(std::mem::take(&mut cur));
                    in_arg = false;
                }
            }
            (None, c) => {
                cur.push(c);
                in_arg = true;
            }
        }
    }
    if in_arg {
        args.push(cur);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(exec: &str) -> DesktopEntry {
        DesktopEntry {
            id: "org.example.App".into(),
            path: PathBuf::from("/usr/share/applications/org.example.App.desktop"),
            name: "Example".into(),
            icon: Some("example".into()),
            exec: Some(exec.into()),
            working_dir: None,
            terminal: false,
            startup_wm_class: None,
        }
    }

    #[test]
    fn parses_main_group_only_and_skips_hidden() {
        let text = "[Desktop Entry]\nName=Dolphin\nName[de]=Delfin\nExec=dolphin %u\nIcon=system-file-manager\nStartupWMClass=dolphin\n\
                    [Desktop Action new]\nExec=dolphin --new-window\n";
        let e = parse("org.kde.dolphin", Path::new("/x.desktop"), text).unwrap();
        assert_eq!((e.name.as_str(), e.exec.as_deref()), ("Dolphin", Some("dolphin %u")));
        assert_eq!(e.startup_wm_class.as_deref(), Some("dolphin"));
        assert!(parse("x", Path::new("/x"), "[Desktop Entry]\nHidden=true\nExec=x\n").is_none());
        assert!(parse("x", Path::new("/x"), "[Desktop Entry]\nType=Link\nURL=https://x\n").is_none());
    }

    #[test]
    fn exec_quoting_and_field_codes() {
        assert_eq!(split_exec(r#"foo "a b" "c\"d" e\s"#).unwrap(), ["foo", "a b", "c\"d", "e\\s"]);
        assert!(split_exec(r#"foo "open"#).is_err());
        let e = entry("app --name=%c %i %U");
        assert_eq!(expand_exec(&e, &[]).unwrap(), ["app", "--name=Example", "--icon", "example"]);
        assert_eq!(expand_exec(&e, &["a".into(), "b".into()]).unwrap(), ["app", "--name=Example", "--icon", "example", "a", "b"]);
        assert_eq!(expand_exec(&entry("app --url=%u"), &["x".into()]).unwrap(), ["app", "--url=x"]);
        assert_eq!(expand_exec(&entry("app %f"), &[]).unwrap(), ["app"]);
        assert_eq!(expand_exec(&entry("app 100%%"), &[]).unwrap(), ["app", "100%"]);
        assert_eq!(expand_exec(&entry("app"), &["--flag".into()]).unwrap(), ["app", "--flag"], "targets are appended");
        assert_eq!(expand_exec(&entry("/usr/bin/discord --url -- %u"), &[]).unwrap(), ["/usr/bin/discord", "--url", "--"]);
    }

    #[test]
    fn extra_args_split_like_a_shell() {
        assert_eq!(split_args(r#"--a 'b c' "d e"  f"#), ["--a", "b c", "d e", "f"]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn subdirectory_ids() {
        assert_eq!(id_paths("kde4-foo"), [PathBuf::from("kde4-foo.desktop"), PathBuf::from("kde4/foo.desktop")]);
    }

    #[test]
    fn value_escapes() {
        assert_eq!(unescape_value(r"a\sb\\c\n"), "a b\\c\n");
    }
}
