//! Qt `QKeyCombination` ints (as sent by kglobalaccel) → display text like `Meta+Ctrl+Down`.

const SHIFT: i32 = 0x0200_0000;
const CTRL: i32 = 0x0400_0000;
const ALT: i32 = 0x0800_0000;
const META: i32 = 0x1000_0000;
const KEY_MASK: i32 = 0x01ff_ffff;

pub fn format_key_combination(combo: i32) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (bit, label) in [(META, "Meta"), (CTRL, "Ctrl"), (ALT, "Alt"), (SHIFT, "Shift")] {
        if combo & bit != 0 {
            parts.push(label.into());
        }
    }
    parts.push(key_name(combo & KEY_MASK));
    parts.join("+")
}

fn key_name(key: i32) -> String {
    if let Some(c) = u32::try_from(key).ok().filter(|k| (0x21..0x7f).contains(k)).and_then(char::from_u32) {
        return c.to_ascii_uppercase().to_string();
    }
    if (0x0100_0030..=0x0100_0052).contains(&key) {
        return format!("F{}", key - 0x0100_0030 + 1);
    }
    let name = match key {
        0x20 => "Space",
        0x0100_0000 => "Esc",
        0x0100_0001 => "Tab",
        0x0100_0002 => "Backtab",
        0x0100_0003 => "Backspace",
        0x0100_0004 => "Return",
        0x0100_0005 => "Enter",
        0x0100_0006 => "Ins",
        0x0100_0007 => "Del",
        0x0100_0008 => "Pause",
        0x0100_0009 => "Print",
        0x0100_000a => "SysReq",
        0x0100_000b => "Clear",
        0x0100_0010 => "Home",
        0x0100_0011 => "End",
        0x0100_0012 => "Left",
        0x0100_0013 => "Up",
        0x0100_0014 => "Right",
        0x0100_0015 => "Down",
        0x0100_0016 => "PgUp",
        0x0100_0017 => "PgDown",
        0x0100_0020 => "Shift",
        0x0100_0021 => "Ctrl",
        0x0100_0022 => "Meta",
        0x0100_0023 => "Alt",
        0x0100_0024 => "CapsLock",
        0x0100_0025 => "NumLock",
        0x0100_0026 => "ScrollLock",
        0x0100_0055 => "Menu",
        0x0100_0061 => "Back",
        0x0100_0062 => "Forward",
        0x0100_0063 => "Stop",
        0x0100_0064 => "Refresh",
        0x0100_0070 => "Volume Down",
        0x0100_0071 => "Volume Mute",
        0x0100_0072 => "Volume Up",
        0x0100_0080 => "Media Play",
        0x0100_0081 => "Media Stop",
        0x0100_0082 => "Media Previous",
        0x0100_0083 => "Media Next",
        0x0100_0084 => "Media Record",
        0x0100_0085 => "Media Pause",
        0x0100_0086 => "Media Play/Pause",
        0x0100_0090 => "Home Page",
        0x0100_00a0 => "Launch Mail",
        0x0100_00a1 => "Launch Media",
        0x0100_00b2 => "Monitor Brightness Up",
        0x0100_00b3 => "Monitor Brightness Down",
        0x0100_00b7 => "Power Off",
        0x0100_00b8 => "Wake Up",
        0x0100_00ba => "Screensaver",
        0x0100_00bd => "Calculator",
        0x0100_0110 => "Sleep",
        0x0100_0117 => "Hibernate",
        0x0100_0119 => "Touchpad Toggle",
        0x0100_00dc => "Explorer",
        0x0100_00f2 => "Mic Mute",
        _ => "",
    };
    if !name.is_empty() {
        return name.to_string();
    }
    // Launch (0)…(9), Launch (A)…(F)
    if (0x0100_00a2..=0x0100_00b1).contains(&key) {
        let n = key - 0x0100_00a2;
        let label = char::from_digit(n as u32, 16).map(|c| c.to_ascii_uppercase()).unwrap_or('?');
        return format!("Launch ({label})");
    }
    format!("0x{key:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_kglobalaccel_samples() {
        assert_eq!(format_key_combination(285_212_691), "Meta+Up");
        assert_eq!(format_key_combination(352_321_557), "Meta+Ctrl+Down");
        assert_eq!(format_key_combination(83_886_137), "Ctrl+F10");
        assert_eq!(format_key_combination(16_777_390), "Launch (C)");
        assert_eq!(format_key_combination(META | 0x57), "Meta+W");
    }
}
