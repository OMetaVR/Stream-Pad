//! The daemon with the real input engine, driven by a fake keyboard. Ignored by default (needs /dev/uinput);
//! run with `cargo test -p streampadd --test input_e2e -- --include-ignored`. See `common::uinput` for the
//! rules that keep every injected event away from the desktop.

mod common;

use common::Daemon;
use common::uinput::{FakeKeyboard, Grabbed, new_node_named, nodes_named};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::time::Duration;
use streampad_core::{Binding, Config, GraphEdge, KeyId, Macro, Node, Position, Profile};
use streampad_input::{MIRROR_NAME_PREFIX, VIRTUAL_KEYBOARD_NAME};

const KEY_A: u16 = 30;
const KEY_H: u16 = 35;
const KEY_I: u16 = 23;
const KEY_SCROLLLOCK: u16 = 70;
const KEY_KP7: u16 = 71;
const KEY_KP4: u16 = 75;
const KEY_KP5: u16 = 76;
const KEY_KPPLUS: u16 = 78;
const KEY_KP1: u16 = 79;
const KEY_KP2: u16 = 80;
const KEY_VOLUMEUP: u16 = 115;
const WINDOW: Duration = Duration::from_millis(600);

fn action(kind: &str, params: Value) -> Binding {
    let node = |id: &str, kind: &str, params: Value| Node {
        id: id.into(),
        kind: kind.into(),
        params: params.as_object().cloned().unwrap_or_default(),
        position: Position::default(),
    };
    let m = Macro::Graph {
        nodes: vec![node("press", "trigger.press", json!({})), node("act", kind, params)],
        edges: vec![GraphEdge { id: "e".into(), source: "press".into(), source_port: "out".into(), target: "act".into() }],
    };
    Binding { label: kind.into(), r#macro: Some(m), ..Binding::default() }
}

fn press_release(code: u16) -> [(u16, i32); 2] {
    [(code, 1), (code, 0)]
}

#[test]
#[ignore = "needs /dev/uinput; run with --include-ignored"]
fn real_keys_switch_modes_run_macros_and_type_through_the_virtual_keyboard() {
    let marker_dir = tempfile::tempdir().unwrap();
    let marker = marker_dir.path().join("touched");
    let mut profile = Profile::new("E2E");
    profile.keys.insert(KeyId::KP1, action("key.shortcut", json!({ "combo": "a" })));
    profile.keys.insert(KeyId::KP2, action("key.type", json!({ "text": "hi", "delayMs": 1 })));
    profile.keys.insert(KeyId::KP5, action("media", json!({ "action": "volumeUp" })));
    let touch = format!("touch '{}'", marker.display());
    profile.keys.insert(KeyId::KP4, action("run.command", json!({ "command": touch, "wait": true })));
    let cfg = Config { profiles: vec![profile, Profile::new("Two")], ..Config::default() };

    let virtual_keyboards_before = nodes_named(VIRTUAL_KEYBOARD_NAME);
    let keyboard = FakeKeyboard::create(&format!("SPTEST daemon {:08x}", fastrand::u32(..)));
    let mut daemon = Daemon::start_with(Some(&cfg), &["--no-overlay", "--only-device", &keyboard.name, "--grab-timeout", "60"]);
    let mut c = daemon.client();
    c.ok("hello", json!({ "client": "cli", "subscribe": true }));
    common::wait_for("the daemon to grab the fake keyboard", || {
        let devices = Daemon::client(&daemon).ok("listDevices", Value::Null);
        devices.as_array().is_some_and(|d| d.iter().any(|d| d["name"] == keyboard.name.as_str() && d["grabbed"] == true))
    });
    // Contain everything before the first injected event.
    let mirror = Grabbed::open(&new_node_named(&format!("{MIRROR_NAME_PREFIX}{}", keyboard.name), &HashSet::new()));
    let output = Grabbed::open(&new_node_named(VIRTUAL_KEYBOARD_NAME, &virtual_keyboards_before));

    // Numpad mode: pad keys pass through to the mirror.
    keyboard.tap(KEY_KP4);
    assert_eq!(mirror.keys(WINDOW), press_release(KEY_KP4));

    // Scroll Lock enters macro mode and is consumed.
    keyboard.tap(KEY_SCROLLLOCK);
    c.wait_event("state", |s| s["macroMode"] == true);
    assert!(mirror.keys(WINDOW).is_empty(), "the toggle key must not reach the system");

    // Macros type through the virtual keyboard; nothing leaks to the mirror.
    keyboard.tap(KEY_KP1);
    c.wait_event("key", |k| k == &json!({ "key": "KP1", "pressed": true, "kind": "macro" }));
    assert_eq!(output.keys(WINDOW), press_release(KEY_A));
    keyboard.tap(KEY_KP2);
    assert_eq!(output.keys(WINDOW), [press_release(KEY_H), press_release(KEY_I)].concat());
    keyboard.tap(KEY_KP5);
    assert_eq!(output.keys(WINDOW), press_release(KEY_VOLUMEUP));
    keyboard.tap(KEY_KP4);
    common::wait_for("run.command to touch the marker", || marker.exists());

    // Globals and blocked unbound keys.
    keyboard.tap(KEY_KPPLUS);
    c.wait_event("profile", |p| p["index"] == 1);
    keyboard.tap(KEY_KP7);
    c.wait_event("key", |k| k["key"] == "KP7" && k["kind"] == "empty");
    assert!(mirror.keys(WINDOW).is_empty(), "captured keys never reach the system");

    // Back to numpad mode.
    keyboard.tap(KEY_SCROLLLOCK);
    c.wait_event("state", |s| s["macroMode"] == false);
    keyboard.tap(KEY_KP7);
    assert_eq!(mirror.keys(WINDOW), press_release(KEY_KP7));
    assert!(output.keys(Duration::from_millis(200)).is_empty());

    c.ok("quit", Value::Null);
    assert!(daemon.wait_exit().success());
}
