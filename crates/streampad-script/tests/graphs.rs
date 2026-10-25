//! Graph macros end to end: compiled by `streampad_core::compile` and executed against the mock host.

mod common;

use common::{MockHost, setup, start};
use serde_json::{Value, json};
use std::sync::Arc;
use streampad_core::compile::{CompileOptions, compile};
use streampad_core::config::{GraphEdge, Macro, Node, Position};
use streampad_script::RunOutcome;

fn node(id: &str, kind: &str, params: Value) -> Node {
    let Value::Object(params) = params else { panic!("params must be an object") };
    Node { id: id.into(), kind: kind.into(), params, position: Position::default() }
}

fn graph(nodes: Vec<Node>, edges: &[(&str, &str, &str)]) -> Macro {
    let edges = edges
        .iter()
        .enumerate()
        .map(|(i, (s, p, t))| GraphEdge {
            id: format!("e{i}"),
            source: s.to_string(),
            source_port: p.to_string(),
            target: t.to_string(),
        })
        .collect();
    Macro::Graph { nodes, edges }
}

/// A press trigger followed by `nodes` in a chain.
fn chain(nodes: Vec<Node>) -> Macro {
    let ids: Vec<String> = std::iter::once("t".to_string()).chain(nodes.iter().map(|n| n.id.clone())).collect();
    let edges: Vec<(&str, &str, &str)> = ids.windows(2).map(|w| (w[0].as_str(), "out", w[1].as_str())).collect();
    graph(std::iter::once(node("t", "trigger.press", json!({}))).chain(nodes).collect(), &edges)
}

fn run(m: &Macro, trace: bool) -> (RunOutcome, Arc<MockHost>) {
    let compiled = compile(m, &CompileOptions { trace }).expect("graph compiles");
    let (host, engine) = setup();
    let started = start(&engine, 1, Arc::new(compiled));
    started.handle.release();
    (started.outcome(), host)
}

#[test]
fn every_action_node_runs_with_the_right_host_call() {
    let steps: Vec<(Node, Vec<(&str, Value)>)> = vec![
        (node("n1", "key.shortcut", json!({ "combo": "ctrl+c" })), vec![("key.press", json!({ "combo": "ctrl+c" }))]),
        (node("n2", "key.down", json!({ "combo": "shift" })), vec![("key.down", json!({ "combo": "shift" }))]),
        (node("n3", "key.up", json!({ "combo": "shift" })), vec![("key.up", json!({ "combo": "shift" }))]),
        (
            node("n4", "key.type", json!({ "text": "hi", "delayMs": 3 })),
            vec![("key.type", json!({ "text": "hi", "delayMs": 3 }))],
        ),
        (node("n5", "media", json!({ "action": "playPause" })), vec![("key.media", json!({ "action": "playPause" }))]),
        (
            node("n6", "app.launch", json!({ "app": "discord", "focusIfRunning": true })),
            vec![("app.launch", json!({ "app": "discord", "focusIfRunning": true, "args": "" }))],
        ),
        (
            node("n7", "open.url", json!({ "url": "https://github.com" })),
            vec![("open.url", json!({ "url": "https://github.com" }))],
        ),
        (node("n8", "open.path", json!({ "path": "/tmp" })), vec![("open.path", json!({ "path": "/tmp" }))]),
        (
            node("n9", "run.command", json!({ "command": "true", "wait": true })),
            vec![("run.command", json!({ "command": "true", "wait": true }))],
        ),
        (
            node("n10", "window.focus", json!({ "match": "obs", "by": "app" })),
            vec![("window.focus", json!({ "match": "obs", "by": "app" }))],
        ),
        (
            node("n11", "window.moveToOutput", json!({ "target": "right" })),
            vec![("window.moveToOutput", json!({ "target": "right" }))],
        ),
        (
            node("n12", "window.action", json!({ "action": "center" })),
            vec![("window.action", json!({ "action": "center" }))],
        ),
        (node("n13", "desktop.switch", json!({ "target": 2 })), vec![("desktop.switch", json!({ "target": 2 }))]),
        (
            node("n14", "desktop.moveWindow", json!({ "target": "prev", "follow": true })),
            vec![("desktop.moveWindow", json!({ "target": "prev", "follow": true }))],
        ),
        (node("n15", "desktop.create", json!({})), vec![("desktop.create", json!({}))]),
        (node("n16", "desktop.remove", json!({})), vec![("desktop.remove", json!({}))]),
        (node("n17", "desktop.overview", json!({})), vec![("desktop.overview", json!({}))]),
        (
            node("n18", "kde.shortcut", json!({ "component": "kwin", "shortcut": "Overview" })),
            vec![("kde.shortcut", json!({ "component": "kwin", "name": "Overview" }))],
        ),
        (
            node("n19", "profile.switch", json!({ "target": "next" })),
            vec![("profile.set", json!({ "target": "next" }))],
        ),
        (node("n20", "macroMode.set", json!({ "value": "off" })), vec![("macroMode.set", json!({ "on": false }))]),
        (node("n21", "overlay.set", json!({ "value": "peek" })), vec![("overlay.set", json!({ "value": "peek" }))]),
        (
            node("n22", "var.set", json!({ "name": "who", "value": "world" })),
            vec![("vars.set", json!({ "name": "who", "value": "world" }))],
        ),
        (
            node("n23", "toast", json!({ "text": "hello {{who}} `${x}`" })),
            vec![
                ("vars.get", json!({ "name": "who" })),
                ("toast", json!({ "text": "hello world `${x}`", "durationMs": 1600, "kind": "info" })),
            ],
        ),
        (
            node("n24", "notify", json!({ "title": "T", "body": "B" })),
            vec![("notify", json!({ "title": "T", "body": "B" }))],
        ),
        (node("n25", "var.toggle", json!({ "name": "flag" })), vec![("vars.toggle", json!({ "name": "flag" }))]),
        (node("n26", "clipboard.set", json!({ "text": "c" })), vec![("clipboard.set", json!({ "text": "c" }))]),
        (node("n27", "sound.play", json!({ "path": "/s.oga" })), vec![("sound.play", json!({ "path": "/s.oga" }))]),
        (node("n28", "delay", json!({ "ms": 1 })), vec![]),
        (
            node("n29", "script", json!({ "code": "const v = clipboard.get();\ntype(v)" })),
            vec![("clipboard.get", json!({})), ("key.type", json!({ "text": "clip", "delayMs": 8 }))],
        ),
    ];
    let ids: Vec<String> = steps.iter().map(|(n, _)| n.id.clone()).collect();
    let expected: Vec<(String, Value)> =
        steps.iter().flat_map(|(_, calls)| calls.iter().map(|(m, a)| (m.to_string(), a.clone()))).collect();
    let (outcome, host) = run(&chain(steps.into_iter().map(|(n, _)| n).collect()), true);
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(host.actions(), expected);
    assert_eq!(host.traces(), std::iter::once("t".to_string()).chain(ids).collect::<Vec<_>>());
}

fn browser_back(apps: &str) -> Macro {
    graph(
        vec![
            node("t", "trigger.press", json!({})),
            node("if", "flow.if", json!({ "condition": { "type": "activeApp", "match": apps } })),
            node("back", "key.shortcut", json!({ "combo": "alt+left" })),
            node("prev", "media", json!({ "action": "prev" })),
            node("done", "toast", json!({ "text": "done" })),
        ],
        &[
            ("t", "out", "if"),
            ("if", "true", "back"),
            ("if", "false", "prev"),
            ("back", "out", "done"),
            ("prev", "out", "done"),
        ],
    )
}

fn methods(host: &MockHost) -> Vec<String> {
    host.actions()
        .into_iter()
        .map(|(m, a)| if m == "key.press" { format!("press {}", a["combo"].as_str().unwrap()) } else { m })
        .collect()
}

#[test]
fn branches_take_the_right_path_and_rejoin_once() {
    let (outcome, host) = run(&browser_back("firefox,chromium"), false);
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(methods(&host), ["window.active", "press alt+left", "toast"]);

    let (_, host) = run(&browser_back("chromium"), false);
    assert_eq!(methods(&host), ["window.active", "key.media", "toast"]);
}

/// Irregular graph (emitted function-per-node): `if1 → (if2 | x)`, `if2 → (x | y)`, `x, y → z`.
fn crossing(first: &str, second: &str) -> Macro {
    let when = |id: &str, app: &str| node(id, "flow.if", json!({ "condition": { "type": "activeApp", "match": app } }));
    graph(
        vec![
            node("t", "trigger.press", json!({})),
            when("if1", first),
            when("if2", second),
            node("x", "key.shortcut", json!({ "combo": "x" })),
            node("y", "key.shortcut", json!({ "combo": "y" })),
            node("z", "key.shortcut", json!({ "combo": "z" })),
        ],
        &[
            ("t", "out", "if1"),
            ("if1", "true", "if2"),
            ("if1", "false", "x"),
            ("if2", "true", "x"),
            ("if2", "false", "y"),
            ("x", "out", "z"),
            ("y", "out", "z"),
        ],
    )
}

#[test]
fn irregular_graphs_run_every_path_correctly() {
    let presses = |host: &MockHost| methods(host).into_iter().filter(|m| m.starts_with("press")).collect::<Vec<_>>();
    let (outcome, host) = run(&crossing("steam", "-"), true);
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(presses(&host), ["press x", "press z"]);
    assert_eq!(host.traces(), ["t", "if1", "x", "z"]);
    let (_, host) = run(&crossing("firefox", "firefox"), false);
    assert_eq!(presses(&host), ["press x", "press z"]);
    let (_, host) = run(&crossing("firefox", "steam"), false);
    assert_eq!(presses(&host), ["press y", "press z"]);
}

#[test]
fn stop_inside_a_loop_ends_the_whole_macro() {
    let count_to_three = |shared_tail: bool| {
        let mut edges =
            vec![("t", "out", "loop"), ("loop", "body", "count"), ("count", "out", "if"), ("if", "true", "stop")];
        // A tail shared by the loop body and the loop exit forces the function-per-node form.
        edges.push(if shared_tail { ("if", "false", "tick") } else { ("loop", "done", "tick") });
        if shared_tail {
            edges.push(("loop", "done", "tick"));
        }
        graph(
            vec![
                node("t", "trigger.press", json!({})),
                node("loop", "flow.repeat", json!({ "count": 10 })),
                node("count", "script", json!({ "code": "vars.set('n', vars.get('n', 0) + 1);" })),
                node("if", "flow.if", json!({ "condition": { "type": "var", "name": "n", "op": "eq", "value": "3" } })),
                node("stop", "flow.stop", json!({})),
                node("tick", "key.shortcut", json!({ "combo": "t" })),
            ],
            &edges,
        )
    };
    for shared_tail in [false, true] {
        let m = count_to_three(shared_tail);
        let code = compile(&m, &CompileOptions::default()).unwrap().code;
        assert_eq!(code.contains("function step1"), shared_tail, "{code}");
        let (outcome, host) = run(&m, false);
        assert_eq!(outcome, RunOutcome::Finished);
        let sets: Vec<Value> =
            host.actions().into_iter().filter(|(m, _)| m == "vars.set").map(|(_, a)| a["value"].clone()).collect();
        assert_eq!(sets, [json!(1), json!(2), json!(3)], "{code}");
        let ticks = methods(&host).iter().filter(|m| *m == "press t").count();
        assert_eq!(ticks, if shared_tail { 2 } else { 0 }, "{code}");
    }
}

#[test]
fn release_trigger_runs_on_release() {
    let m = graph(
        vec![
            node("t", "trigger.press", json!({})),
            node("down", "key.down", json!({ "combo": "shift" })),
            node("r", "trigger.release", json!({})),
            node("up", "key.up", json!({ "combo": "shift" })),
        ],
        &[("t", "out", "down"), ("r", "out", "up")],
    );
    let (outcome, host) = run(&m, true);
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(methods(&host), ["key.down", "key.up"]);
    assert_eq!(host.traces(), ["t", "down", "r", "up"]);
}

#[test]
fn conditions_evaluate_at_runtime() {
    let cases: &[(Value, bool)] = &[
        (json!({ "type": "activeApp", "match": "/^org\\.mozilla/" }), true),
        (json!({ "type": "activeTitle", "match": "youtube" }), true),
        (json!({ "type": "appRunning", "match": "steam" }), false),
        (json!({ "type": "var", "name": "unset", "op": "falsy" }), true),
        (json!({ "type": "var", "name": "unset", "op": "neq", "value": "1" }), true),
        (json!({ "type": "profile", "profileId": "p_audio" }), true),
        (json!({ "type": "time", "from": "00:00", "to": "00:00" }), true),
        (json!({ "type": "expr", "code": "event.key === \"KP4\" // the key" }), true),
    ];
    for (condition, expected) in cases {
        let m = graph(
            vec![
                node("t", "trigger.press", json!({})),
                node("if", "flow.if", json!({ "condition": condition })),
                node("yes", "key.shortcut", json!({ "combo": "y" })),
                node("no", "key.shortcut", json!({ "combo": "n" })),
            ],
            &[("t", "out", "if"), ("if", "true", "yes"), ("if", "false", "no")],
        );
        let (outcome, host) = run(&m, false);
        assert_eq!(outcome, RunOutcome::Finished, "{condition}");
        let pressed = if *expected { "press y" } else { "press n" };
        assert!(methods(&host).contains(&pressed.to_string()), "{condition}: {:?}", methods(&host));
    }
}
