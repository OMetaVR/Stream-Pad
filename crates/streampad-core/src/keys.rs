//! The 17 numpad keys StreamPad manages, with their evdev codes, legends and grid placement.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A key on the numpad. Serialized as its variant name (`"KP4"`, `"KPEnter"`, …).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum KeyId {
    NumLock,
    KPSlash,
    KPAsterisk,
    KPMinus,
    KP7,
    KP8,
    KP9,
    KPPlus,
    KP4,
    KP5,
    KP6,
    KP1,
    KP2,
    KP3,
    KPEnter,
    KP0,
    KPDot,
}

/// Placement of a key in the 4×5 numpad grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridPos {
    pub col: u8,
    pub row: u8,
    pub col_span: u8,
    pub row_span: u8,
}

/// Keycap legend: the big glyph and the small navigation glyph printed on real numpads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Legend {
    pub main: String,
    pub sub: Option<String>,
}

/// evdev code of Scroll Lock, the default macro-mode toggle key.
pub const KEY_SCROLLLOCK: u16 = 70;

impl KeyId {
    pub const ALL: [KeyId; 17] = [
        KeyId::NumLock,
        KeyId::KPSlash,
        KeyId::KPAsterisk,
        KeyId::KPMinus,
        KeyId::KP7,
        KeyId::KP8,
        KeyId::KP9,
        KeyId::KPPlus,
        KeyId::KP4,
        KeyId::KP5,
        KeyId::KP6,
        KeyId::KP1,
        KeyId::KP2,
        KeyId::KP3,
        KeyId::KPEnter,
        KeyId::KP0,
        KeyId::KPDot,
    ];

    /// Linux evdev `KEY_*` code.
    pub const fn evdev_code(self) -> u16 {
        match self {
            KeyId::NumLock => 69,
            KeyId::KPSlash => 98,
            KeyId::KPAsterisk => 55,
            KeyId::KPMinus => 74,
            KeyId::KP7 => 71,
            KeyId::KP8 => 72,
            KeyId::KP9 => 73,
            KeyId::KPPlus => 78,
            KeyId::KP4 => 75,
            KeyId::KP5 => 76,
            KeyId::KP6 => 77,
            KeyId::KP1 => 79,
            KeyId::KP2 => 80,
            KeyId::KP3 => 81,
            KeyId::KPEnter => 96,
            KeyId::KP0 => 82,
            KeyId::KPDot => 83,
        }
    }

    pub fn from_evdev_code(code: u16) -> Option<KeyId> {
        KeyId::ALL.into_iter().find(|k| k.evdev_code() == code)
    }

    /// Variant name, identical to the serialized form.
    pub const fn as_str(self) -> &'static str {
        match self {
            KeyId::NumLock => "NumLock",
            KeyId::KPSlash => "KPSlash",
            KeyId::KPAsterisk => "KPAsterisk",
            KeyId::KPMinus => "KPMinus",
            KeyId::KP7 => "KP7",
            KeyId::KP8 => "KP8",
            KeyId::KP9 => "KP9",
            KeyId::KPPlus => "KPPlus",
            KeyId::KP4 => "KP4",
            KeyId::KP5 => "KP5",
            KeyId::KP6 => "KP6",
            KeyId::KP1 => "KP1",
            KeyId::KP2 => "KP2",
            KeyId::KP3 => "KP3",
            KeyId::KPEnter => "KPEnter",
            KeyId::KP0 => "KP0",
            KeyId::KPDot => "KPDot",
        }
    }

    /// Case-insensitive parse of the variant name (`"kp4"`, `"KPEnter"`).
    pub fn parse(s: &str) -> Option<KeyId> {
        KeyId::ALL.into_iter().find(|k| k.as_str().eq_ignore_ascii_case(s.trim()))
    }

    /// Human name, e.g. "Numpad 4", "Numpad Enter".
    pub const fn display_name(self) -> &'static str {
        match self {
            KeyId::NumLock => "Num Lock",
            KeyId::KPSlash => "Numpad /",
            KeyId::KPAsterisk => "Numpad *",
            KeyId::KPMinus => "Numpad −",
            KeyId::KP7 => "Numpad 7",
            KeyId::KP8 => "Numpad 8",
            KeyId::KP9 => "Numpad 9",
            KeyId::KPPlus => "Numpad +",
            KeyId::KP4 => "Numpad 4",
            KeyId::KP5 => "Numpad 5",
            KeyId::KP6 => "Numpad 6",
            KeyId::KP1 => "Numpad 1",
            KeyId::KP2 => "Numpad 2",
            KeyId::KP3 => "Numpad 3",
            KeyId::KPEnter => "Numpad Enter",
            KeyId::KP0 => "Numpad 0",
            KeyId::KPDot => "Numpad .",
        }
    }

    pub fn legend(self) -> Legend {
        let (main, sub): (&str, Option<&str>) = match self {
            KeyId::NumLock => ("Num", Some("Lock")),
            KeyId::KPSlash => ("/", None),
            KeyId::KPAsterisk => ("*", None),
            KeyId::KPMinus => ("−", None),
            KeyId::KP7 => ("7", Some("Home")),
            KeyId::KP8 => ("8", Some("↑")),
            KeyId::KP9 => ("9", Some("PgUp")),
            KeyId::KPPlus => ("+", None),
            KeyId::KP4 => ("4", Some("←")),
            KeyId::KP5 => ("5", None),
            KeyId::KP6 => ("6", Some("→")),
            KeyId::KP1 => ("1", Some("End")),
            KeyId::KP2 => ("2", Some("↓")),
            KeyId::KP3 => ("3", Some("PgDn")),
            KeyId::KPEnter => ("Enter", None),
            KeyId::KP0 => ("0", Some("Ins")),
            KeyId::KPDot => (".", Some("Del")),
        };
        Legend { main: main.to_string(), sub: sub.map(str::to_string) }
    }

    pub const fn grid(self) -> GridPos {
        let (col, row, col_span, row_span) = match self {
            KeyId::NumLock => (0, 0, 1, 1),
            KeyId::KPSlash => (1, 0, 1, 1),
            KeyId::KPAsterisk => (2, 0, 1, 1),
            KeyId::KPMinus => (3, 0, 1, 1),
            KeyId::KP7 => (0, 1, 1, 1),
            KeyId::KP8 => (1, 1, 1, 1),
            KeyId::KP9 => (2, 1, 1, 1),
            KeyId::KPPlus => (3, 1, 1, 2),
            KeyId::KP4 => (0, 2, 1, 1),
            KeyId::KP5 => (1, 2, 1, 1),
            KeyId::KP6 => (2, 2, 1, 1),
            KeyId::KP1 => (0, 3, 1, 1),
            KeyId::KP2 => (1, 3, 1, 1),
            KeyId::KP3 => (2, 3, 1, 1),
            KeyId::KPEnter => (3, 3, 1, 2),
            KeyId::KP0 => (0, 4, 2, 1),
            KeyId::KPDot => (2, 4, 1, 1),
        };
        GridPos { col, row, col_span, row_span }
    }
}

impl fmt::Display for KeyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_roundtrip() {
        for k in KeyId::ALL {
            assert_eq!(KeyId::from_evdev_code(k.evdev_code()), Some(k));
            assert_eq!(KeyId::parse(k.as_str()), Some(k));
            assert_eq!(serde_json::to_string(&k).unwrap(), format!("\"{}\"", k.as_str()));
        }
    }

    #[test]
    fn grid_covers_4x5_exactly_once() {
        let mut cells = [[0u8; 4]; 5];
        for k in KeyId::ALL {
            let g = k.grid();
            for r in g.row..g.row + g.row_span {
                for c in g.col..g.col + g.col_span {
                    cells[r as usize][c as usize] += 1;
                }
            }
        }
        assert!(cells.iter().flatten().all(|&n| n == 1));
    }
}
