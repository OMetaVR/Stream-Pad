//! End-to-end over the socket: a sandboxed daemon driven with `simulateKey` and other IPC methods.

mod common;

use common::{Client, Daemon, wait_for};
use serde_json::{Value, json};
use std::io::Read;
use std::time::Duration;
use streampad_core::{Binding, Config, GraphEdge, KeyId, Macro, Node, Position, Profile};

fn graph(kind: &str, params: Value) -> Macro {
    let node = |id: &str, kind: &str, params: Value| Node {
        id: id.into(),
        kind: kind.into(),
        params: params.as_object().cloned().unwrap_or_default(),
        position: Position::default(),
    };
    Macro::Graph {
        nodes: vec![node("press", "trigger.press", json!({})), node("act", kind, params)],
        edges: vec![GraphEdge { id: "e".into(), source: "press".into(), source_port: "out".into(), target: "act".into() }],
    }
}

fn script(source: &str) -> Macro {
    Macro::Script { source: source.into() }
}

fn binding(label: &str, m: Macro) -> Binding {
    Binding { label: label.into(), r#macro: Some(m), ..Binding::default() }
}

/// Profile "Test" (KP1 touches a file, KP2 toggles a var + toasts, KP3 logs on release) and an empty "Second".
fn test_config(marker: &std::path::Path) -> Config {
    let mut test = Profile::new("Test");
    let touch = format!("touch '{}'", marker.display());
    test.keys.insert(KeyId::KP1, binding("Touch", graph("run.command", json!({ "command": touch, "wait": false }))));
    test.keys.insert(KeyId::KP2, binding("Toggle", script("const on = vars.toggle('x'); toast('x is ' + on);")));
    test.keys.insert(KeyId::KP3, binding("Release", script("function onRelease(e) { log('released', e.key); }")));
    Config { profiles: vec![test, Profile::new("Second")], ..Config::default() }
}

#[test]
fn simulated_keys_drive_macros_globals_and_events() {
    let marker_dir = tempfile::tempdir().unwrap();
    let marker = marker_dir.path().join("touched");
    let daemon = Daemon::start(Some(&test_config(&marker)));
    let mut editor = daemon.client();
    let hello = editor.ok("hello", json!({ "client": "editor", "subscribe": true, "trace": true }));
    assert_eq!(hello["version"], streampad_core::VERSION);
    let state = &hello["state"];
    assert_eq!(state["macroMode"], false);
    assert_eq!(state["profiles"].as_array().unwrap().len(), 2);
    assert_eq!(state["profile"]["keys"].as_object().unwrap().len(), 17);
    assert_eq!(state["integrations"]["kwin"], false, "sandboxed: no session bus");

    // Pad keys are ignored outside macro mode, like real (passed-through) keys.
    editor.ok("simulateKey", json!({ "key": "KP1", "pressed": true }));
    editor.ok("simulateKey", json!({ "key": "KP1", "pressed": false }));
    editor.ok("simulateKey", json!({ "key": "Toggle" }));
    editor.wait_event("state", |s| s["macroMode"] == true && s["overlay"]["visible"] == true);
    assert!(!marker.exists());

    // A graph macro: key events, run lifecycle, trace events, and its side effect.
    editor.ok("simulateKey", json!({ "key": "KP1", "pressed": true }));
    editor.ok("simulateKey", json!({ "key": "KP1", "pressed": false }));
    editor.wait_event("key", |k| k == &json!({ "key": "KP1", "pressed": true, "kind": "macro" }));
    let started = editor.wait_event("run", |r| r["status"] == "started" && r["key"] == "KP1");
    let run_id = started["runId"].clone();
    editor.wait_event("trace", |t| t["runId"] == run_id && t["nodeId"] == "act");
    editor.wait_event("run", |r| r["runId"] == run_id && r["status"] == "finished");
    wait_for("the macro's touch", || marker.exists());

    // A script macro using vars + toast.
    editor.ok("simulateKey", json!({ "key": "KP2", "pressed": true }));
    editor.ok("simulateKey", json!({ "key": "KP2", "pressed": false }));
    let toast = editor.wait_event("toast", |_| true);
    assert_eq!((toast["text"].as_str(), toast["kind"].as_str()), (Some("x is true"), Some("info")));
    assert_eq!(editor.ok("getVars", Value::Null)["x"], true);

    // onRelease runs on the key release.
    editor.ok("simulateKey", json!({ "key": "KP3", "pressed": true }));
    editor.ok("simulateKey", json!({ "key": "KP3", "pressed": false }));
    let log = editor.wait_event("log", |l| l["message"] == "released KP3");
    assert_eq!(log["level"], "info");

    // Globals: Numpad + = next profile, announced by a `profile` event before the state.
    editor.ok("simulateKey", json!({ "key": "KPPlus", "pressed": true }));
    editor.ok("simulateKey", json!({ "key": "KPPlus", "pressed": false }));
    editor.wait_event("profile", |p| p == &json!({ "index": 1, "direction": 1 }));
    editor.wait_event("state", |s| s["profileIndex"] == 1 && s["profile"]["name"] == "Second");

    // Numpad Enter tap pins the overlay.
    editor.ok("simulateKey", json!({ "key": "KPEnter", "pressed": true }));
    editor.ok("simulateKey", json!({ "key": "KPEnter", "pressed": false }));
    editor.wait_event("state", |s| s["overlay"]["pinned"] == true);

    // Errors keep their shape.
    let err = editor.call("compileMacro", json!({ "macro": { "type": "graph", "nodes": [], "edges": [] } })).unwrap_err();
    assert!(!err.is_empty());
    assert!(editor.call("noSuchMethod", json!({})).unwrap_err().contains("unknown method"));
    let code = editor.ok("compileMacro", json!({ "macro": serde_json::to_value(graph("toast", json!({ "text": "hi" }))).unwrap() }));
    assert!(code["code"].as_str().unwrap().contains("toast("));
}

#[test]
fn learn_mode_ends_when_its_client_disconnects() {
    let daemon = Daemon::start(Some(&test_config(std::path::Path::new("/nonexistent/marker"))));
    let mut watcher = daemon.client();
    watcher.ok("hello", json!({ "client": "cli", "subscribe": true }));
    watcher.ok("setMacroMode", json!({ "on": true }));

    let mut editor = daemon.client();
    editor.ok("hello", json!({ "client": "editor", "subscribe": true }));
    editor.ok("setLearn", json!({ "on": true }));
    editor.ok("simulateKey", json!({ "key": "KP2", "pressed": true }));
    editor.ok("simulateKey", json!({ "key": "KP2", "pressed": false }));
    editor.wait_event("learn", |l| l == &json!({ "key": "KP2" }));
    assert_eq!(editor.ok("getVars", Value::Null), json!({}), "learn mode never runs macros");

    drop(editor);
    watcher.wait_event("state", |s| s["learn"] == false);
}

#[test]
fn a_client_that_stops_reading_is_dropped_without_stalling_others() {
    let daemon = Daemon::start(Some(&test_config(std::path::Path::new("/nonexistent/marker"))));
    let mut stuck = daemon.client();
    stuck.ok("hello", json!({ "client": "cli", "subscribe": true }));
    let mut healthy = daemon.client();
    healthy.ok("hello", json!({ "client": "editor", "subscribe": true }));

    // Thousands of uncoalesced events; `stuck` never reads.
    let flood = "for (let i = 0; i < 20000; i++) toast('flood ' + i + ' ' + 'x'.repeat(200));";
    healthy.ok("runMacro", json!({ "macro": { "type": "script", "source": flood } }));
    let done = |h: &mut Client| {
        let events = h.drain(Duration::from_millis(200));
        events.iter().any(|(n, d)| n == "run" && d["status"] == "finished")
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while !done(&mut healthy) {
        assert!(std::time::Instant::now() < deadline, "the flood never finished");
    }
    assert_eq!(healthy.ok("getState", Value::Null)["profileIndex"], 0, "the daemon still answers");

    // The stuck client was disconnected: after its buffered data it reads EOF.
    let mut raw = stuck.into_stream();
    raw.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut sink = Vec::new();
    let read = raw.read_to_end(&mut sink);
    assert!(read.is_ok(), "expected EOF after being dropped, got {read:?}");
}

#[test]
fn quit_answers_then_exits_and_persists_state() {
    let mut daemon = Daemon::start(None);
    let mut c = daemon.client();
    let state = c.ok("getState", Value::Null);
    assert_eq!(state["profiles"].as_array().unwrap().len(), 5, "first run writes the default profiles");
    c.ok("setMacroMode", json!({ "on": true }));
    c.ok("setProfile", json!({ "index": 2 }));
    assert_eq!(c.ok("quit", Value::Null), json!({}));
    assert!(daemon.wait_exit().success());
    assert!(!daemon.socket().exists(), "the socket is removed on exit");
    let persisted: Value = serde_json::from_str(&std::fs::read_to_string(daemon.state_file()).unwrap()).unwrap();
    assert_eq!(persisted["macroMode"], true);
    assert_eq!(persisted["profileId"], state["profiles"][2]["id"]);
}
