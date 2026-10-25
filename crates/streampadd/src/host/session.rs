//! Session services used by macros: desktop notifications, KDE global shortcuts, clipboard and sound.

use super::spawn::find_in_path;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use zbus::zvariant::Value as Variant;

const CLIPBOARD_TIMEOUT: Duration = Duration::from_secs(2);

/// `org.freedesktop.Notifications.Notify`.
pub async fn notify(conn: &zbus::Connection, title: &str, body: &str) -> Result<(), String> {
    let hints: HashMap<&str, Variant<'_>> = HashMap::from([("desktop-entry", Variant::from(super::APP_ID))]);
    let actions: Vec<&str> = Vec::new();
    conn.call_method(
        Some("org.freedesktop.Notifications"),
        "/org/freedesktop/Notifications",
        Some("org.freedesktop.Notifications"),
        "Notify",
        &("StreamPad", 0u32, super::APP_ID, title, body, actions, hints, -1i32),
    )
    .await
    .map(drop)
    .map_err(|e| format!("cannot show a notification: {e}"))
}

/// kglobalaccel object path of a component ("org.kde.dolphin.desktop" → "/component/org_kde_dolphin_desktop").
fn component_path(component: &str) -> String {
    let name: String = component.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    format!("/component/{name}")
}

/// Trigger a KDE global shortcut, e.g. `("kwin", "Overview")`.
pub async fn kde_shortcut(conn: &zbus::Connection, component: &str, name: &str) -> Result<(), String> {
    const SERVICE: &str = "org.kde.kglobalaccel";
    const IFACE: &str = "org.kde.kglobalaccel.Component";
    let path = component_path(component);
    let names: Vec<String> = conn
        .call_method(Some(SERVICE), path.as_str(), Some(IFACE), "shortcutNames", &())
        .await
        .and_then(|reply| reply.body().deserialize())
        .map_err(|_| format!("no KDE shortcut component \"{component}\" (KDE global shortcuts are unavailable on this desktop?)"))?;
    if !names.iter().any(|n| n == name) {
        return Err(format!("KDE component \"{component}\" has no shortcut \"{name}\""));
    }
    conn.call_method(Some(SERVICE), path.as_str(), Some(IFACE), "invokeShortcut", &(name,))
        .await
        .map(drop)
        .map_err(|e| format!("cannot trigger {component}/{name}: {e}"))
}

enum ClipboardTool {
    Wayland,
    Xclip,
    Xsel,
}

fn clipboard_tool() -> Result<ClipboardTool, String> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
    if wayland {
        return if find_in_path("wl-copy").is_some() {
            Ok(ClipboardTool::Wayland)
        } else {
            Err("clipboard access needs wl-clipboard (wl-copy / wl-paste)".into())
        };
    }
    if find_in_path("xclip").is_some() {
        Ok(ClipboardTool::Xclip)
    } else if find_in_path("xsel").is_some() {
        Ok(ClipboardTool::Xsel)
    } else {
        Err("clipboard access needs xclip or xsel".into())
    }
}

pub fn clipboard_get() -> Result<String, String> {
    let (program, args): (&str, &[&str]) = match clipboard_tool()? {
        ClipboardTool::Wayland => ("wl-paste", &["--no-newline", "--type", "text"]),
        ClipboardTool::Xclip => ("xclip", &["-selection", "clipboard", "-o"]),
        ClipboardTool::Xsel => ("xsel", &["--clipboard", "--output"]),
    };
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    let output = wait_with_timeout(child, program)?;
    // An empty clipboard makes the tools exit non-zero; that's just "".
    Ok(if output.status.success() { String::from_utf8_lossy(&output.stdout).into_owned() } else { String::new() })
}

pub fn clipboard_set(text: &str) -> Result<(), String> {
    let (program, args): (&str, &[&str]) = match clipboard_tool()? {
        ClipboardTool::Wayland => ("wl-copy", &[]),
        ClipboardTool::Xclip => ("xclip", &["-selection", "clipboard", "-i"]),
        ClipboardTool::Xsel => ("xsel", &["--clipboard", "--input"]),
    };
    // These tools fork a background owner that serves the content; the foreground process exits right away.
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot run {program}: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| format!("{program}: {e}"))?;
    }
    let output = wait_with_timeout(child, program)?;
    if output.status.success() { Ok(()) } else { Err(format!("{program} failed ({})", output.status)) }
}

fn wait_with_timeout(mut child: std::process::Child, program: &str) -> Result<std::process::Output, String> {
    let deadline = Instant::now() + CLIPBOARD_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().map_err(|e| format!("{program}: {e}")),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} did not respond"));
            }
            Err(e) => return Err(format!("{program}: {e}")),
        }
    }
}

/// Play a sound file without waiting for it to finish.
pub fn play_sound(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("sound file not found: {}", path.display()));
    }
    let player = ["pw-play", "paplay"].into_iter().find(|p| find_in_path(p).is_some()).ok_or("playing sounds needs pw-play (PipeWire) or paplay")?;
    let mut child = Command::new(player)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot run {player}: {e}"))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kglobalaccel_paths() {
        assert_eq!(component_path("kwin"), "/component/kwin");
        assert_eq!(component_path("org.kde.dolphin.desktop"), "/component/org_kde_dolphin_desktop");
        assert_eq!(component_path("KDE Keyboard Layout Switcher"), "/component/KDE_Keyboard_Layout_Switcher");
    }
}
