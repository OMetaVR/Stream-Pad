use super::*;
use crate::config::{GraphEdge, Node, Position};
use serde_json::{Value, json};

fn node(id: &str, kind: &str, params: Value) -> Node {
    let Value::Object(params) = params else { panic!("params must be an object") };
    Node { id: id.into(), kind: kind.into(), params, position: Position::default() }
}

fn graph(nodes: Vec<Node>, edges: &[(&str, &str, &str)]) -> Macro {
    let edges = edges
        .iter()
        .enumerate()
        .map(|(i, (source, port, target))| GraphEdge {
            id: format!("e{i}"),
            source: source.to_string(),
            source_port: port.to_string(),
            target: target.to_string(),
        })
        .collect();
    Macro::Graph { nodes, edges }
}

fn press() -> Node {
    node("t", "trigger.press", json!({}))
}

fn shortcut(id: &str, combo: &str) -> Node {
    node(id, "key.shortcut", json!({ "combo": combo }))
}

fn toast(id: &str, text: &str) -> Node {
    node(id, "toast", json!({ "text": text }))
}

fn when_app(id: &str, apps: &str) -> Node {
    node(id, "flow.if", json!({ "condition": { "type": "activeApp", "match": apps } }))
}

fn js(m: &Macro) -> String {
    compile(m, &CompileOptions::default()).unwrap_or_else(|e| panic!("compile failed: {e:?}")).code
}

fn error(m: &Macro) -> CompileError {
    compile(m, &CompileOptions::default()).expect_err("expected a compile error")
}

/// The single statement a one-node graph compiles to.
fn statement(kind: &str, params: Value) -> Result<String, String> {
    let m = graph(vec![press(), node("n", kind, params)], &[("t", "out", "n")]);
    compile(&m, &CompileOptions::default()).map(|c| c.code.trim_end().to_string()).map_err(|e| e.message)
}

#[test]
fn linear_chain_is_a_statement_list() {
    let m = graph(
        vec![
            press(),
            shortcut("a", "ctrl+c"),
            node("b", "delay", json!({ "ms": 100 })),
            node("c", "key.type", json!({ "text": "Hello {{name}}" })),
        ],
        &[("t", "out", "a"), ("a", "out", "b"), ("b", "out", "c")],
    );
    assert_eq!(js(&m), "press(\"ctrl+c\");\nsleep(100);\ntype(`Hello ${vars.get(\"name\", \"\")}`);\n");
}

#[test]
fn empty_graph_compiles_to_nothing() {
    let compiled = compile(&graph(vec![press()], &[]), &CompileOptions::default()).unwrap();
    assert_eq!(compiled, CompiledMacro { code: String::new(), has_release: false });
}

#[test]
fn release_chain_becomes_on_release() {
    let m = graph(
        vec![
            press(),
            node("down", "key.down", json!({ "combo": "shift" })),
            node("r", "trigger.release", json!({})),
            node("up", "key.up", json!({ "combo": "shift" })),
        ],
        &[("t", "out", "down"), ("r", "out", "up")],
    );
    let compiled = compile(&m, &CompileOptions::default()).unwrap();
    assert!(compiled.has_release);
    assert_eq!(compiled.code, "keyDown(\"shift\");\n\nfunction onRelease(event) {\n  keyUp(\"shift\");\n}\n");
}

#[test]
fn unconnected_release_trigger_defines_no_handler() {
    let m = graph(vec![press(), shortcut("a", "a"), node("r", "trigger.release", json!({}))], &[("t", "out", "a")]);
    let compiled = compile(&m, &CompileOptions::default()).unwrap();
    assert!(!compiled.has_release);
    assert_eq!(compiled.code, "press(\"a\");\n");
}

#[test]
fn if_else_rejoins_at_the_post_dominator() {
    let m = graph(
        vec![
            press(),
            when_app("if", "firefox,chromium"),
            shortcut("back", "alt+left"),
            node("prev", "media", json!({ "action": "prev" })),
            toast("done", "done"),
        ],
        &[
            ("t", "out", "if"),
            ("if", "true", "back"),
            ("if", "false", "prev"),
            ("back", "out", "done"),
            ("prev", "out", "done"),
        ],
    );
    assert_eq!(
        js(&m),
        "if (window.isActive(\"firefox,chromium\")) {\n  press(\"alt+left\");\n} else {\n  media(\"prev\");\n}\ntoast(\"done\");\n"
    );
}

#[test]
fn branch_that_goes_straight_to_the_join_is_omitted() {
    let flag = || node("if", "flow.if", json!({ "condition": { "type": "var", "name": "gameMode", "op": "truthy" } }));
    let only_true = graph(
        vec![press(), flag(), shortcut("a", "a"), toast("j", "j")],
        &[("t", "out", "if"), ("if", "true", "a"), ("if", "false", "j"), ("a", "out", "j")],
    );
    assert_eq!(js(&only_true), "if (vars.get(\"gameMode\")) {\n  press(\"a\");\n}\ntoast(\"j\");\n");

    let only_false = graph(
        vec![press(), flag(), shortcut("b", "b"), toast("j", "j")],
        &[("t", "out", "if"), ("if", "true", "j"), ("if", "false", "b"), ("b", "out", "j")],
    );
    assert_eq!(js(&only_false), "if (!vars.get(\"gameMode\")) {\n  press(\"b\");\n}\ntoast(\"j\");\n");

    let both_direct =
        graph(vec![press(), flag(), toast("j", "j")], &[("t", "out", "if"), ("if", "true", "j"), ("if", "false", "j")]);
    assert_eq!(js(&both_direct), "if (vars.get(\"gameMode\")) {}\ntoast(\"j\");\n");
}

#[test]
fn branches_that_end_separately_keep_their_tails() {
    let m = graph(
        vec![
            press(),
            when_app("if", "obs"),
            shortcut("a", "a"),
            toast("after", "after"),
            node("stop", "flow.stop", json!({})),
        ],
        &[("t", "out", "if"), ("if", "true", "a"), ("a", "out", "after"), ("if", "false", "stop")],
    );
    assert_eq!(
        js(&m),
        "if (window.isActive(\"obs\")) {\n  press(\"a\");\n  toast(\"after\");\n} else {\n  return;\n}\n"
    );
}

#[test]
fn if_inside_repeat_inside_if() {
    let m = graph(
        vec![
            press(),
            when_app("outer", "obs"),
            node("loop", "flow.repeat", json!({ "count": 3 })),
            node(
                "inner",
                "flow.if",
                json!({ "condition": { "type": "var", "name": "mode", "op": "eq", "value": "fast" } }),
            ),
            shortcut("x", "x"),
            shortcut("y", "y"),
            node("wait", "delay", json!({ "ms": 50 })),
            toast("after", "after"),
            node("stop", "flow.stop", json!({})),
        ],
        &[
            ("t", "out", "outer"),
            ("outer", "true", "loop"),
            ("outer", "false", "stop"),
            ("loop", "body", "inner"),
            ("inner", "true", "x"),
            ("inner", "false", "y"),
            ("x", "out", "wait"),
            ("y", "out", "wait"),
            ("loop", "done", "after"),
        ],
    );
    assert_eq!(
        js(&m),
        "\
if (window.isActive(\"obs\")) {
  for (let i = 0; i < 3; i++) {
    if (vars.get(\"mode\") === \"fast\") {
      press(\"x\");
    } else {
      press(\"y\");
    }
    sleep(50);
  }
  toast(\"after\");
} else {
  return;
}
"
    );
}

#[test]
fn nested_repeats_use_distinct_counters() {
    let m = graph(
        vec![
            press(),
            node("outer", "flow.repeat", json!({ "count": 2 })),
            node("inner", "flow.repeat", json!({})),
            node("deepest", "flow.repeat", json!({ "count": "4" })),
            node("deeper", "flow.repeat", json!({ "count": 1 })),
            shortcut("a", "a"),
        ],
        &[
            ("t", "out", "outer"),
            ("outer", "body", "inner"),
            ("inner", "body", "deepest"),
            ("deepest", "body", "deeper"),
            ("deeper", "body", "a"),
        ],
    );
    assert_eq!(
        js(&m),
        "\
for (let i = 0; i < 2; i++) {
  for (let j = 0; j < 2; j++) {
    for (let k = 0; k < 4; k++) {
      for (let i3 = 0; i3 < 1; i3++) {
        press(\"a\");
      }
    }
  }
}
"
    );
}

#[test]
fn else_if_chains() {
    let m = graph(
        vec![
            press(),
            when_app("if1", "firefox"),
            when_app("if2", "code"),
            shortcut("a", "ctrl+t"),
            shortcut("b", "ctrl+n"),
            shortcut("c", "n"),
            toast("j", "j"),
        ],
        &[
            ("t", "out", "if1"),
            ("if1", "true", "a"),
            ("if1", "false", "if2"),
            ("if2", "true", "b"),
            ("if2", "false", "c"),
            ("a", "out", "j"),
            ("b", "out", "j"),
            ("c", "out", "j"),
        ],
    );
    assert_eq!(
        js(&m),
        "\
if (window.isActive(\"firefox\")) {
  press(\"ctrl+t\");
} else if (window.isActive(\"code\")) {
  press(\"ctrl+n\");
} else {
  press(\"n\");
}
toast(\"j\");
"
    );
}

/// `if1 → (if2 | x)`, `if2 → (x | y)`, `x → z`, `y → z`: x is shared by two branches that are not the two
/// sides of one `if`, so structured output would have to duplicate it.
fn crossing_branches() -> Macro {
    graph(
        vec![
            press(),
            when_app("if1", "a"),
            when_app("if2", "b"),
            shortcut("x", "x"),
            shortcut("y", "y"),
            toast("z", "z"),
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
fn irregular_graph_falls_back_to_functions() {
    assert_eq!(
        js(&crossing_branches()),
        "\
// Steps of this flow are shared by paths that plain if/else can't express, so each step is a
// function that runs one node and then the next step. A step returns true to stop the macro.
step1();

function step1() {
  if (window.isActive(\"a\")) {
    return step2();
  }
  return step3();
}

function step2() {
  if (window.isActive(\"b\")) {
    return step3();
  }
  return step5();
}

function step3() {
  press(\"x\");
  return step4();
}

function step4() {
  toast(\"z\");
}

function step5() {
  press(\"y\");
  return step4();
}
"
    );
}

#[test]
fn repeat_body_running_into_the_loop_exit_falls_back() {
    let m = graph(
        vec![
            press(),
            node("loop", "flow.repeat", json!({ "count": 2 })),
            when_app("if", "x"),
            node("stop", "flow.stop", json!({})),
            toast("shared", "shared"),
            node("r", "trigger.release", json!({})),
        ],
        &[
            ("t", "out", "loop"),
            ("loop", "body", "if"),
            ("if", "true", "stop"),
            ("if", "false", "shared"),
            ("loop", "done", "shared"),
            ("r", "out", "shared"),
        ],
    );
    let compiled = compile(&m, &CompileOptions { trace: true }).unwrap();
    assert!(compiled.has_release);
    assert_eq!(
        compiled.code,
        "\
// Steps of this flow are shared by paths that plain if/else can't express, so each step is a
// function that runs one node and then the next step. A step returns true to stop the macro.
__node(\"t\");
step1();

function onRelease(event) {
  __node(\"r\");
  step4();
}

function step1() {
  __node(\"loop\");
  for (let i = 0; i < 2; i++) {
    if (step2()) return true;
  }
  return step4();
}

function step2() {
  __node(\"if\");
  if (window.isActive(\"x\")) {
    return step3();
  }
  return step4();
}

function step3() {
  __node(\"stop\");
  return true;
}

function step4() {
  __node(\"shared\");
  toast(\"shared\");
}
"
    );
}

#[test]
fn trace_marks_every_node() {
    let m = graph(
        vec![
            press(),
            shortcut("a", "a"),
            when_app("if", "x"),
            shortcut("b", "b"),
            node("s", "script", json!({ "code": "log(1)" })),
        ],
        &[("t", "out", "a"), ("a", "out", "if"), ("if", "true", "b"), ("if", "false", "s")],
    );
    let compiled = compile(&m, &CompileOptions { trace: true }).unwrap();
    assert_eq!(
        compiled.code,
        "\
__node(\"t\");
__node(\"a\");
press(\"a\");
__node(\"if\");
if (window.isActive(\"x\")) {
  __node(\"b\");
  press(\"b\");
} else {
  __node(\"s\");
  {
    log(1)
  }
}
"
    );
}

#[test]
fn every_node_kind_compiles() {
    let cases: &[(&str, Value, &str)] = &[
        ("key.shortcut", json!({ "combo": "ctrl+shift+t" }), "press(\"ctrl+shift+t\");"),
        ("key.down", json!({ "combo": "shift" }), "keyDown(\"shift\");"),
        ("key.up", json!({ "combo": "shift" }), "keyUp(\"shift\");"),
        ("key.type", json!({ "text": "hi" }), "type(\"hi\");"),
        ("key.type", json!({ "text": "hi", "delayMs": 20 }), "type(\"hi\", { delayMs: 20 });"),
        ("media", json!({ "action": "volumeUp" }), "media(\"volumeUp\");"),
        ("app.launch", json!({ "app": "org.kde.dolphin", "focusIfRunning": true }), "launch(\"org.kde.dolphin\");"),
        ("app.close", json!({ "app": "steam" }), "closeApp(\"steam\");"),
        ("app.close", json!({ "app": "steam", "force": true }), "closeApp(\"steam\", { force: true });"),
        (
            "app.launch",
            json!({ "app": "firefox", "focusIfRunning": false, "args": "--private-window" }),
            "launch(\"firefox\", { focusIfRunning: false, args: \"--private-window\" });",
        ),
        ("open.url", json!({ "url": "https://youtube.com" }), "openUrl(\"https://youtube.com\");"),
        ("open.path", json!({ "path": "~/Videos/{{folder}}" }), "openPath(`~/Videos/${vars.get(\"folder\", \"\")}`);"),
        ("run.command", json!({ "command": "notify-send hi" }), "run(\"notify-send hi\");"),
        ("run.command", json!({ "command": "make", "wait": true }), "run(\"make\", { wait: true });"),
        ("window.focus", json!({ "match": "discord", "by": "app" }), "window.focus(\"discord\");"),
        ("window.focus", json!({ "match": "YouTube", "by": "title" }), "window.focus(\"YouTube\", { by: \"title\" });"),
        ("window.moveToOutput", json!({ "target": "DP-1" }), "window.moveToOutput(\"DP-1\");"),
        ("window.action", json!({ "action": "keepAbove" }), "window.keepAbove();"),
        ("desktop.switch", json!({ "target": "left" }), "desktop.switchTo(\"left\");"),
        ("desktop.switch", json!({ "target": 2 }), "desktop.switchTo(2);"),
        (
            "desktop.moveWindow",
            json!({ "target": "next", "follow": true }),
            "desktop.moveWindow(\"next\", { follow: true });",
        ),
        ("desktop.moveWindow", json!({ "target": "3" }), "desktop.moveWindow(3);"),
        ("desktop.create", json!({}), "desktop.create();"),
        ("desktop.remove", json!({}), "desktop.remove();"),
        ("desktop.overview", json!({}), "desktop.overview();"),
        (
            "kde.shortcut",
            json!({ "component": "kwin", "shortcut": "Overview" }),
            "kde.shortcut(\"kwin\", \"Overview\");",
        ),
        ("profile.switch", json!({ "target": "next" }), "profile.next();"),
        ("profile.switch", json!({ "target": "prev" }), "profile.prev();"),
        ("profile.switch", json!({ "target": "p_3f9c2a71" }), "profile.set(\"p_3f9c2a71\");"),
        ("macroMode.set", json!({ "value": "on" }), "macroMode.set(true);"),
        ("macroMode.set", json!({ "value": "off" }), "macroMode.set(false);"),
        ("macroMode.set", json!({ "value": "toggle" }), "macroMode.toggle();"),
        ("overlay.set", json!({ "value": "peek" }), "overlay.peek();"),
        ("overlay.set", json!({ "value": "show" }), "overlay.show();"),
        ("toast", json!({ "text": "Game mode: {{gameMode}}" }), "toast(`Game mode: ${vars.get(\"gameMode\", \"\")}`);"),
        ("toast", json!({ "text": "Saved", "durationMs": 3000 }), "toast(\"Saved\", { durationMs: 3000 });"),
        ("notify", json!({ "title": "Done", "body": "" }), "notify(\"Done\");"),
        (
            "notify",
            json!({ "title": "Done", "body": "Took {{secs}}s" }),
            "notify(\"Done\", `Took ${vars.get(\"secs\", \"\")}s`);",
        ),
        ("var.set", json!({ "name": "count", "value": "3" }), "vars.set(\"count\", 3);"),
        ("var.set", json!({ "name": "mode", "value": "gaming" }), "vars.set(\"mode\", \"gaming\");"),
        ("var.set", json!({ "name": "cfg", "value": "{\"a\": [1, true]}" }), "vars.set(\"cfg\", {\"a\":[1,true]});"),
        ("var.set", json!({ "name": "on", "value": true }), "vars.set(\"on\", true);"),
        ("var.toggle", json!({ "name": "gameMode" }), "vars.toggle(\"gameMode\");"),
        ("clipboard.set", json!({ "text": "" }), "clipboard.set(\"\");"),
        ("sound.play", json!({ "path": "/usr/share/sounds/ding.oga" }), "sound.play(\"/usr/share/sounds/ding.oga\");"),
        ("delay", json!({}), "sleep(100);"),
        ("delay", json!({ "ms": 12.5 }), "sleep(12.5);"),
        (
            "script",
            json!({ "code": "const t = clipboard.get();\ntype(t.trim());" }),
            "{\n  const t = clipboard.get();\n  type(t.trim());\n}",
        ),
        ("script", json!({ "code": "  " }), ""),
        ("flow.repeat", json!({ "count": 0 }), "for (let i = 0; i < 0; i++) {\n}"),
        ("flow.stop", json!({}), "return;"),
    ];
    for (kind, params, expected) in cases {
        assert_eq!(statement(kind, params.clone()).as_deref(), Ok(*expected), "{kind} {params}");
    }
}

#[test]
fn every_condition_type_compiles() {
    let cases: &[(Value, &str)] = &[
        (json!({ "type": "activeApp", "match": "firefox, zen" }), "window.isActive(\"firefox, zen\")"),
        (json!({ "type": "activeTitle", "match": "/youtube/i" }), "window.isActive(\"/youtube/i\", { by: \"title\" })"),
        (json!({ "type": "appRunning", "match": "obs" }), "window.isRunning(\"obs\")"),
        (json!({ "type": "var", "name": "on" }), "vars.get(\"on\")"),
        (json!({ "type": "var", "name": "on", "op": "falsy" }), "!vars.get(\"on\")"),
        (json!({ "type": "var", "name": "n", "op": "eq", "value": "3" }), "vars.get(\"n\") === 3"),
        (json!({ "type": "var", "name": "n", "op": "neq", "value": "abc" }), "vars.get(\"n\") !== \"abc\""),
        (
            json!({ "type": "var", "name": "n", "op": "eq", "value": "[1,2]" }),
            "JSON.stringify(vars.get(\"n\")) === \"[1,2]\"",
        ),
        (json!({ "type": "profile", "profileId": "p_1" }), "profile.current().id === \"p_1\""),
        (json!({ "type": "time", "from": "22:00", "to": "6:30" }), "isTimeBetween(\"22:00\", \"06:30\")"),
        (json!({ "type": "expr", "code": " clipboard.get().length > 3; " }), "clipboard.get().length > 3"),
        (json!({ "type": "expr", "code": "a > 1 // big" }), "\n  a > 1 // big\n"),
    ];
    for (cond, expected) in cases {
        let code = statement("flow.if", json!({ "condition": cond })).unwrap();
        assert_eq!(code, format!("if ({expected}) {{}}"), "{cond}");
    }
}

#[test]
fn negated_conditions_read_naturally() {
    let cases: &[(Value, &str)] = &[
        (json!({ "type": "activeApp", "match": "obs" }), "!window.isActive(\"obs\")"),
        (json!({ "type": "var", "name": "on", "op": "falsy" }), "vars.get(\"on\")"),
        (json!({ "type": "var", "name": "n", "op": "eq", "value": "1" }), "vars.get(\"n\") !== 1"),
        (json!({ "type": "profile", "profileId": "p" }), "profile.current().id !== \"p\""),
        (json!({ "type": "expr", "code": "a || b" }), "!(a || b)"),
    ];
    for (cond, expected) in cases {
        let m = graph(
            vec![press(), node("if", "flow.if", json!({ "condition": cond })), shortcut("a", "a")],
            &[("t", "out", "if"), ("if", "false", "a")],
        );
        assert_eq!(js(&m), format!("if ({expected}) {{\n  press(\"a\");\n}}\n"), "{cond}");
    }
}

#[test]
fn text_params_are_literals_not_code() {
    let hostile = "\"); run(\"rm -rf ~\"); (\"";
    assert_eq!(
        statement("key.type", json!({ "text": hostile })).unwrap(),
        format!("type({});", serde_json::to_string(hostile).unwrap())
    );
    assert_eq!(
        statement("open.url", json!({ "url": "`${evil()}` {{x}}" })).unwrap(),
        "openUrl(`\\`\\${evil()}\\` ${vars.get(\"x\", \"\")}`);"
    );
    assert_eq!(
        statement("key.type", json!({ "text": "line1\nline2 \\ {{ user name }}" })).unwrap(),
        "type(`line1\\nline2 \\\\ ${vars.get(\"user name\", \"\")}`);"
    );
    assert_eq!(statement("var.toggle", json!({ "name": "a\"b" })).unwrap(), "vars.toggle(\"a\\\"b\");");
}

#[test]
fn script_code_with_template_literals_is_kept_verbatim() {
    let m = graph(
        vec![press(), when_app("if", "x"), node("s", "script", json!({ "code": "  type(`a\n  b`);" }))],
        &[("t", "out", "if"), ("if", "true", "s")],
    );
    assert_eq!(js(&m), "if (window.isActive(\"x\")) {\n  {\n    type(`a\n  b`);\n  }\n}\n");
}

#[test]
fn unreachable_nodes_are_ignored() {
    let m = graph(
        vec![
            press(),
            shortcut("a", "a"),
            node("orphan", "no.such.kind", json!({})),
            shortcut("bad", ""),
            toast("island", "never"),
        ],
        &[("t", "out", "a"), ("bad", "out", "island"), ("orphan", "weird", "island")],
    );
    assert_eq!(js(&m), "press(\"a\");\n");
}

#[test]
fn dangling_edges_are_ignored() {
    let m =
        graph(vec![press(), shortcut("a", "a")], &[("t", "out", "a"), ("a", "out", "deleted"), ("ghost", "out", "a")]);
    assert_eq!(js(&m), "press(\"a\");\n");
}

#[test]
fn graph_rule_errors() {
    let e = error(&graph(vec![shortcut("a", "a")], &[]));
    assert_eq!(e.node_id, None);
    assert!(e.message.contains("Press trigger"), "{}", e.message);

    let e = error(&graph(vec![press(), node("t2", "trigger.press", json!({}))], &[]));
    assert_eq!(e.node_id.as_deref(), Some("t2"));

    let release = |id: &str| node(id, "trigger.release", json!({}));
    let e = error(&graph(vec![press(), release("r1"), release("r2")], &[]));
    assert_eq!(e.node_id.as_deref(), Some("r2"));

    let e = error(&graph(vec![press(), shortcut("a", "a"), shortcut("a", "b")], &[]));
    assert_eq!(e.node_id.as_deref(), Some("a"));
    assert!(e.message.contains("share the id"), "{}", e.message);

    let e =
        error(&graph(vec![press(), shortcut("a", "a"), shortcut("b", "b")], &[("t", "out", "a"), ("t", "out", "b")]));
    assert_eq!(e.node_id.as_deref(), Some("t"));
    assert!(e.message.contains("only have one connection"), "{}", e.message);

    let e = error(&graph(vec![press(), shortcut("a", "a")], &[("t", "true", "a")]));
    assert_eq!(e.node_id.as_deref(), Some("t"));
    assert!(e.message.contains("\"true\""), "{}", e.message);

    let e = error(&graph(
        vec![press(), when_app("if", "x"), shortcut("a", "a")],
        &[("t", "out", "if"), ("if", "out", "a")],
    ));
    assert_eq!(e.node_id.as_deref(), Some("if"));

    let e = error(&graph(vec![press(), shortcut("a", "a")], &[("t", "out", "a"), ("a", "out", "t")]));
    assert_eq!(e.node_id.as_deref(), Some("a"));
    assert!(e.message.contains("Triggers"), "{}", e.message);

    let e = error(&graph(
        vec![press(), shortcut("a", "a"), shortcut("b", "b"), shortcut("c", "c")],
        &[("t", "out", "a"), ("a", "out", "b"), ("b", "out", "c"), ("c", "out", "a")],
    ));
    assert_eq!(e.node_id.as_deref(), Some("c"));
    assert!(e.message.contains("loop"), "{}", e.message);

    let e = error(&graph(
        vec![press(), node("loop", "flow.repeat", json!({})), shortcut("a", "a")],
        &[("t", "out", "loop"), ("loop", "body", "a"), ("a", "out", "loop")],
    ));
    assert_eq!(e.node_id.as_deref(), Some("a"));
}

#[test]
fn param_errors_name_the_node() {
    let cases: &[(&str, Value, &str)] = &[
        ("no.such.kind", json!({}), "Unknown node type \"no.such.kind\""),
        ("key.shortcut", json!({}), "Record a shortcut"),
        ("key.shortcut", json!({ "combo": "  " }), "Record a shortcut"),
        ("key.shortcut", json!({ "combo": "ctrl+nope" }), "Shortcut: unknown key \"nope\""),
        ("key.up", json!({ "combo": "hyper" }), "Shortcut: unknown key \"hyper\""),
        ("key.type", json!({ "text": "" }), "Enter the text to type"),
        (
            "key.type",
            json!({ "text": "x", "delayMs": -1 }),
            "Delay between keys must be a number of milliseconds (0 or more)",
        ),
        ("media", json!({ "action": "louder" }), "Unknown media action \"louder\""),
        ("media", json!({}), "Choose a media action"),
        ("app.launch", json!({ "app": " " }), "Choose an app to launch"),
        (
            "app.launch",
            json!({ "app": "x", "focusIfRunning": "yes" }),
            "\"focusIfRunning\" must be true or false, not \"yes\"",
        ),
        ("open.url", json!({ "url": "" }), "Enter a URL"),
        ("open.path", json!({}), "Choose a file or folder"),
        ("run.command", json!({ "command": "" }), "Enter a command to run"),
        ("window.focus", json!({ "match": "" }), "Enter the app or window title to focus"),
        ("window.focus", json!({ "match": "x", "by": "pid" }), "Unknown match mode \"pid\""),
        ("window.action", json!({ "action": "explode" }), "Unknown window action \"explode\""),
        (
            "desktop.switch",
            json!({ "target": 0 }),
            "Unknown desktop \"0\" (use next, prev, left, right, up, down or a number from 1)",
        ),
        ("kde.shortcut", json!({ "component": "kwin" }), "Choose a shortcut"),
        ("profile.switch", json!({}), "Choose a profile"),
        ("macroMode.set", json!({ "value": "maybe" }), "Unknown macro mode value \"maybe\""),
        ("overlay.set", json!({}), "Choose an overlay action"),
        ("toast", json!({ "text": " " }), "Enter the toast text"),
        ("notify", json!({ "body": "x" }), "Enter a notification title"),
        ("var.set", json!({ "value": "1" }), "Enter a variable name"),
        ("var.toggle", json!({ "name": "" }), "Enter a variable name"),
        ("sound.play", json!({}), "Choose a sound file"),
        ("delay", json!({ "ms": "soon" }), "Delay must be a number of milliseconds (0 or more)"),
        ("flow.repeat", json!({ "count": 1.5 }), "Count must be a whole number (0 or more)"),
        ("flow.repeat", json!({ "count": -2 }), "Count must be a whole number (0 or more)"),
        ("flow.if", json!({}), "Choose a condition"),
        ("flow.if", json!({ "condition": { "type": "weather" } }), "Unknown condition type \"weather\""),
        ("flow.if", json!({ "condition": { "type": "activeApp", "match": "" } }), "Enter the app to check for"),
        ("flow.if", json!({ "condition": { "type": "var", "name": "x", "op": "gt" } }), "Unknown comparison \"gt\""),
        (
            "flow.if",
            json!({ "condition": { "type": "time", "from": "25:00", "to": "01:00" } }),
            "Time \"25:00\" must look like 09:30 (24-hour)",
        ),
        ("flow.if", json!({ "condition": { "type": "expr", "code": " ; " } }), "Enter a condition expression"),
    ];
    for (kind, params, message) in cases {
        let m = graph(vec![press(), node("n", kind, params.clone())], &[("t", "out", "n")]);
        let e = error(&m);
        assert_eq!((e.node_id.as_deref(), e.message.as_str()), (Some("n"), *message), "{kind} {params}");
    }
}

#[test]
fn scripts_pass_through() {
    let m = Macro::Script { source: "press('a');\nfunction onRelease() { press('b') }".into() };
    let compiled = compile(&m, &CompileOptions { trace: true }).unwrap();
    assert_eq!(compiled.code, "press('a');\nfunction onRelease() { press('b') }");
    assert!(compiled.has_release);
    assert!(!compile(&Macro::Script { source: "press('a')".into() }, &CompileOptions::default()).unwrap().has_release);
}
