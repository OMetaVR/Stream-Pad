//! Combo strings like `"ctrl+shift+t"` (docs/SPEC.md §3.4) and US-layout character → key mapping.

use crate::key_table::KEY_TABLE;
use std::fmt;

pub const KEY_LEFTCTRL: u16 = 29;
pub const KEY_LEFTSHIFT: u16 = 42;
pub const KEY_LEFTALT: u16 = 56;
pub const KEY_LEFTMETA: u16 = 125;
pub const KEY_RIGHTCTRL: u16 = 97;
pub const KEY_RIGHTSHIFT: u16 = 54;
pub const KEY_RIGHTALT: u16 = 100;
pub const KEY_RIGHTMETA: u16 = 126;

/// Keys to press in order (modifiers first) and release in reverse order.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Combo {
    pub keys: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ComboError {
    #[error("empty shortcut")]
    Empty,
    #[error("unknown key \"{0}\"")]
    UnknownKey(String),
}

const ALIASES: &[(&str, u16)] = &[
    ("ctrl", KEY_LEFTCTRL),
    ("control", KEY_LEFTCTRL),
    ("lctrl", KEY_LEFTCTRL),
    ("shift", KEY_LEFTSHIFT),
    ("lshift", KEY_LEFTSHIFT),
    ("alt", KEY_LEFTALT),
    ("lalt", KEY_LEFTALT),
    ("option", KEY_LEFTALT),
    ("meta", KEY_LEFTMETA),
    ("super", KEY_LEFTMETA),
    ("win", KEY_LEFTMETA),
    ("windows", KEY_LEFTMETA),
    ("cmd", KEY_LEFTMETA),
    ("lmeta", KEY_LEFTMETA),
    ("lwin", KEY_LEFTMETA),
    ("rctrl", KEY_RIGHTCTRL),
    ("rshift", KEY_RIGHTSHIFT),
    ("ralt", KEY_RIGHTALT),
    ("altgr", KEY_RIGHTALT),
    ("rmeta", KEY_RIGHTMETA),
    ("rwin", KEY_RIGHTMETA),
    ("escape", 1),
    ("return", 28),
    ("ret", 28),
    ("pgup", 104),
    ("pgdn", 109),
    ("pgdown", 109),
    ("del", 111),
    ("ins", 110),
    ("printscreen", 99),
    ("prtsc", 99),
    ("prtscr", 99),
    ("caps", 58),
    ("capslock", 58),
    ("next", 163),
    ("nexttrack", 163),
    ("prev", 165),
    ("previous", 165),
    ("prevtrack", 165),
    ("play", 164),
    ("pause_media", 164),
    ("stop", 166),
    ("volup", 115),
    ("voldown", 114),
    ("vol+", 115),
    ("vol-", 114),
    ("-", 12),
    ("=", 13),
    ("[", 26),
    ("]", 27),
    (";", 39),
    ("'", 40),
    ("`", 41),
    ("\\", 43),
    (",", 51),
    (".", 52),
    ("/", 53),
    ("plus", 78),
    ("numpad0", 82),
    ("numpad1", 79),
    ("numpad2", 80),
    ("numpad3", 81),
    ("numpad4", 75),
    ("numpad5", 76),
    ("numpad6", 77),
    ("numpad7", 71),
    ("numpad8", 72),
    ("numpad9", 73),
];

/// Look up an evdev key code by (case-insensitive) name or alias.
pub fn key_code(name: &str) -> Option<u16> {
    let n = name.trim().to_ascii_lowercase();
    if n.is_empty() {
        return None;
    }
    let n = n.strip_prefix("key_").unwrap_or(&n);
    ALIASES
        .iter()
        .find(|(a, _)| *a == n)
        .map(|(_, c)| *c)
        .or_else(|| KEY_TABLE.iter().find(|(k, _)| *k == n).map(|(_, c)| *c))
}

/// Canonical evdev name (lower-case, no `KEY_` prefix).
pub fn key_name(code: u16) -> Option<&'static str> {
    KEY_TABLE.iter().find(|(_, c)| *c == code).map(|(n, _)| *n)
}

pub fn is_modifier(code: u16) -> bool {
    matches!(
        code,
        KEY_LEFTCTRL | KEY_LEFTSHIFT | KEY_LEFTALT | KEY_LEFTMETA | KEY_RIGHTCTRL | KEY_RIGHTSHIFT | KEY_RIGHTALT | KEY_RIGHTMETA
    )
}

/// Parse `"ctrl+shift+t"`. A trailing `"++"` means the plus key (`"ctrl++"` = ctrl + kpplus).
pub fn parse_combo(s: &str) -> Result<Combo, ComboError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ComboError::Empty);
    }
    let mut parts: Vec<String> = Vec::new();
    let (body, trailing_plus) = if s == "+" {
        ("", true)
    } else if let Some(b) = s.strip_suffix("++") {
        (b, true)
    } else {
        (s, false)
    };
    for p in body.split('+') {
        let p = p.trim();
        if !p.is_empty() {
            parts.push(p.to_string());
        }
    }
    if trailing_plus {
        parts.push("kpplus".into());
    }
    if parts.is_empty() {
        return Err(ComboError::Empty);
    }
    let mut keys = Vec::with_capacity(parts.len());
    for p in &parts {
        let code = key_code(p).ok_or_else(|| ComboError::UnknownKey(p.clone()))?;
        if !keys.contains(&code) {
            keys.push(code);
        }
    }
    // modifiers first, preserving relative order otherwise
    let (mut mods, rest): (Vec<u16>, Vec<u16>) = keys.into_iter().partition(|c| is_modifier(*c));
    mods.extend(rest);
    Ok(Combo { keys: mods })
}

/// Pretty label for a key code ("Ctrl", "←", "F13", "Play/Pause").
pub fn display_key(code: u16) -> String {
    let fixed = match code {
        KEY_LEFTCTRL | KEY_RIGHTCTRL => "Ctrl",
        KEY_LEFTSHIFT | KEY_RIGHTSHIFT => "Shift",
        KEY_LEFTALT => "Alt",
        KEY_RIGHTALT => "AltGr",
        KEY_LEFTMETA | KEY_RIGHTMETA => "Meta",
        103 => "↑",
        108 => "↓",
        105 => "←",
        106 => "→",
        1 => "Esc",
        28 => "Enter",
        57 => "Space",
        15 => "Tab",
        14 => "Backspace",
        111 => "Delete",
        110 => "Insert",
        102 => "Home",
        107 => "End",
        104 => "PgUp",
        109 => "PgDn",
        99 => "Print",
        70 => "Scroll Lock",
        119 => "Pause",
        58 => "Caps Lock",
        69 => "Num Lock",
        164 => "Play/Pause",
        163 => "Next",
        165 => "Previous",
        166 => "Stop",
        115 => "Vol+",
        114 => "Vol−",
        113 => "Mute",
        12 => "-",
        13 => "=",
        26 => "[",
        27 => "]",
        39 => ";",
        40 => "'",
        41 => "`",
        43 => "\\",
        51 => ",",
        52 => ".",
        53 => "/",
        _ => "",
    };
    if !fixed.is_empty() {
        return fixed.to_string();
    }
    match key_name(code) {
        Some(n) if n.starts_with("kp") => format!("Num {}", n[2..].to_ascii_uppercase()),
        Some(n) if n.len() == 1 => n.to_ascii_uppercase(),
        Some(n) if n.starts_with('f') && n[1..].chars().all(|c| c.is_ascii_digit()) => n.to_ascii_uppercase(),
        Some(n) => {
            let mut c = n.chars();
            c.next().map(|f| f.to_ascii_uppercase().to_string() + c.as_str()).unwrap_or_default()
        }
        None => format!("#{code}"),
    }
}

impl fmt::Display for Combo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = self.keys.iter().map(|k| display_key(*k)).collect();
        f.write_str(&parts.join(" + "))
    }
}

/// US-layout mapping for typing text: char → (key code, needs shift). `None` = not typable via keycodes
/// (callers fall back to clipboard paste).
pub fn char_to_key(c: char) -> Option<(u16, bool)> {
    const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
    const LETTER_CODES: [u16; 26] = [30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44];
    if let Some(i) = LOWER.find(c) {
        return Some((LETTER_CODES[i], false));
    }
    if c.is_ascii_uppercase() {
        let i = LOWER.find(c.to_ascii_lowercase())?;
        return Some((LETTER_CODES[i], true));
    }
    let r = match c {
        '1' => (2, false),
        '2' => (3, false),
        '3' => (4, false),
        '4' => (5, false),
        '5' => (6, false),
        '6' => (7, false),
        '7' => (8, false),
        '8' => (9, false),
        '9' => (10, false),
        '0' => (11, false),
        '!' => (2, true),
        '@' => (3, true),
        '#' => (4, true),
        '$' => (5, true),
        '%' => (6, true),
        '^' => (7, true),
        '&' => (8, true),
        '*' => (9, true),
        '(' => (10, true),
        ')' => (11, true),
        '-' => (12, false),
        '_' => (12, true),
        '=' => (13, false),
        '+' => (13, true),
        '[' => (26, false),
        '{' => (26, true),
        ']' => (27, false),
        '}' => (27, true),
        ';' => (39, false),
        ':' => (39, true),
        '\'' => (40, false),
        '"' => (40, true),
        '`' => (41, false),
        '~' => (41, true),
        '\\' => (43, false),
        '|' => (43, true),
        ',' => (51, false),
        '<' => (51, true),
        '.' => (52, false),
        '>' => (52, true),
        '/' => (53, false),
        '?' => (53, true),
        ' ' => (57, false),
        '\n' => (28, false),
        '\t' => (15, false),
        _ => return None,
    };
    Some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common() {
        assert_eq!(parse_combo("ctrl+shift+t").unwrap().keys, vec![29, 42, 20]);
        assert_eq!(parse_combo("Meta+Left").unwrap().keys, vec![125, 105]);
        assert_eq!(parse_combo("t+ctrl").unwrap().keys, vec![29, 20]);
        assert_eq!(parse_combo("F13").unwrap().keys, vec![183]);
        assert_eq!(parse_combo("kp4").unwrap().keys, vec![75]);
        assert_eq!(parse_combo("playpause").unwrap().keys, vec![164]);
        assert_eq!(parse_combo("ctrl++").unwrap().keys, vec![29, 78]);
        assert_eq!(parse_combo("alt+f4").unwrap().keys, vec![56, 62]);
        assert!(matches!(parse_combo("ctrl+nope"), Err(ComboError::UnknownKey(_))));
        assert!(matches!(parse_combo("  "), Err(ComboError::Empty)));
    }

    #[test]
    fn displays() {
        assert_eq!(parse_combo("ctrl+shift+t").unwrap().to_string(), "Ctrl + Shift + T");
        assert_eq!(parse_combo("meta+left").unwrap().to_string(), "Meta + ←");
        assert_eq!(parse_combo("f13").unwrap().to_string(), "F13");
    }

    #[test]
    fn chars() {
        assert_eq!(char_to_key('a'), Some((30, false)));
        assert_eq!(char_to_key('A'), Some((30, true)));
        assert_eq!(char_to_key('?'), Some((53, true)));
        assert_eq!(char_to_key('é'), None);
    }
}
