//! The JS API of docs/SPEC.md §6: each function's host mapping, return values and argument checking.

mod common;

use common::{run_script, script, setup, start};
use serde_json::{Value, json};
use streampad_script::RunOutcome;

#[test]
fn every_api_function_maps_to_its_host_method() {
    let cases: Vec<(&str, Vec<(&str, Value)>)> = vec![
        (
            r#"press("ctrl+c", "a")"#,
            vec![("key.press", json!({ "combo": "ctrl+c" })), ("key.press", json!({ "combo": "a" }))],
        ),
        (r#"keyDown("shift")"#, vec![("key.down", json!({ "combo": "shift" }))]),
        (r#"keyUp("shift")"#, vec![("key.up", json!({ "combo": "shift" }))]),
        (r#"type("hi")"#, vec![("key.type", json!({ "text": "hi", "delayMs": 8 }))]),
        (r#"type(42, { delayMs: 0 })"#, vec![("key.type", json!({ "text": "42", "delayMs": 0 }))]),
        (r#"media("volumeUp")"#, vec![("key.media", json!({ "action": "volumeUp" }))]),
        (r#"closeApp("steam")"#, vec![("app.close", json!({ "app": "steam", "force": false }))]),
        (r#"closeApp("steam", { force: true })"#, vec![("app.close", json!({ "app": "steam", "force": true }))]),
        (r#"launch("firefox")"#, vec![("app.launch", json!({ "app": "firefox", "focusIfRunning": true, "args": "" }))]),
        (
            r#"launch("firefox", { focusIfRunning: false, args: "--new-window" })"#,
            vec![("app.launch", json!({ "app": "firefox", "focusIfRunning": false, "args": "--new-window" }))],
        ),
        (r#"openUrl("https://kick.com")"#, vec![("open.url", json!({ "url": "https://kick.com" }))]),
        (r#"openPath("~/Videos")"#, vec![("open.path", json!({ "path": "~/Videos" }))]),
        (r#"run("ls")"#, vec![("run.command", json!({ "command": "ls", "wait": false }))]),
        (r#"run("ls", { wait: true })"#, vec![("run.command", json!({ "command": "ls", "wait": true }))]),
        ("window.active()", vec![("window.active", json!({}))]),
        ("window.list()", vec![("window.list", json!({}))]),
        (r#"window.isActive("firefox")"#, vec![("window.active", json!({}))]),
        (r#"window.isRunning("konsole")"#, vec![("window.list", json!({}))]),
        (r#"window.focus("discord")"#, vec![("window.focus", json!({ "match": "discord", "by": "app" }))]),
        (
            r#"window.focus("YouTube", { by: "title" })"#,
            vec![("window.focus", json!({ "match": "YouTube", "by": "title" }))],
        ),
        (r#"window.moveToOutput("left")"#, vec![("window.moveToOutput", json!({ "target": "left" }))]),
        ("window.minimize()", vec![("window.action", json!({ "action": "minimize" }))]),
        ("window.maximize()", vec![("window.action", json!({ "action": "maximize" }))]),
        ("window.fullscreen()", vec![("window.action", json!({ "action": "fullscreen" }))]),
        ("window.close()", vec![("window.action", json!({ "action": "close" }))]),
        ("window.keepAbove()", vec![("window.action", json!({ "action": "keepAbove" }))]),
        ("window.center()", vec![("window.action", json!({ "action": "center" }))]),
        (r#"desktop.switchTo("left")"#, vec![("desktop.switch", json!({ "target": "left" }))]),
        ("desktop.switchTo(2)", vec![("desktop.switch", json!({ "target": 2 }))]),
        (
            r#"desktop.moveWindow("next", { follow: true })"#,
            vec![("desktop.moveWindow", json!({ "target": "next", "follow": true }))],
        ),
        ("desktop.moveWindow(3)", vec![("desktop.moveWindow", json!({ "target": 3, "follow": false }))]),
        ("desktop.create()", vec![("desktop.create", json!({}))]),
        ("desktop.remove()", vec![("desktop.remove", json!({}))]),
        ("desktop.overview()", vec![("desktop.overview", json!({}))]),
        ("desktop.current()", vec![("desktop.current", json!({}))]),
        (
            r#"kde.shortcut("kwin", "Overview")"#,
            vec![("kde.shortcut", json!({ "component": "kwin", "name": "Overview" }))],
        ),
        ("profile.next()", vec![("profile.set", json!({ "target": "next" }))]),
        ("profile.prev()", vec![("profile.set", json!({ "target": "prev" }))]),
        (r#"profile.set("Web")"#, vec![("profile.set", json!({ "target": "Web" }))]),
        ("profile.set(0)", vec![("profile.set", json!({ "target": 0 }))]),
        ("profile.current()", vec![("profile.current", json!({}))]),
        ("macroMode.get()", vec![("macroMode.get", json!({}))]),
        ("macroMode.set(false)", vec![("macroMode.set", json!({ "on": false }))]),
        ("macroMode.toggle()", vec![("macroMode.toggle", json!({}))]),
        ("overlay.show()", vec![("overlay.set", json!({ "value": "show" }))]),
        ("overlay.hide()", vec![("overlay.set", json!({ "value": "hide" }))]),
        ("overlay.toggle()", vec![("overlay.set", json!({ "value": "toggle" }))]),
        ("overlay.peek()", vec![("overlay.set", json!({ "value": "peek" }))]),
        ("overlay.peek(2500)", vec![("overlay.set", json!({ "value": "peek", "durationMs": 2500 }))]),
        (r#"toast("hi")"#, vec![("toast", json!({ "text": "hi", "durationMs": 1600, "kind": "info" }))]),
        (
            r#"toast("saved", { durationMs: 500, kind: "success" })"#,
            vec![("toast", json!({ "text": "saved", "durationMs": 500, "kind": "success" }))],
        ),
        (r#"notify("Done")"#, vec![("notify", json!({ "title": "Done", "body": "" }))]),
        (r#"notify("Done", "All good")"#, vec![("notify", json!({ "title": "Done", "body": "All good" }))]),
        (r#"vars.get("a")"#, vec![("vars.get", json!({ "name": "a" }))]),
        (r#"vars.set("a", { n: [1, 2] })"#, vec![("vars.set", json!({ "name": "a", "value": { "n": [1, 2] } }))]),
        (r#"vars.set("a")"#, vec![("vars.set", json!({ "name": "a", "value": null }))]),
        (r#"vars.toggle("gameMode")"#, vec![("vars.toggle", json!({ "name": "gameMode" }))]),
        ("clipboard.get()", vec![("clipboard.get", json!({}))]),
        (r#"clipboard.set("copied")"#, vec![("clipboard.set", json!({ "text": "copied" }))]),
        (
            r#"sound.play("/usr/share/sounds/ding.oga")"#,
            vec![("sound.play", json!({ "path": "/usr/share/sounds/ding.oga" }))],
        ),
        (r#"__host("custom.method", '{"x":1}')"#, vec![("custom.method", json!({ "x": 1 }))]),
    ];
    for (code, expected) in cases {
        let (outcome, host) = run_script(code);
        assert_eq!(outcome, RunOutcome::Finished, "{code}");
        let expected: Vec<(String, Value)> = expected.into_iter().map(|(m, a)| (m.to_string(), a)).collect();
        assert_eq!(host.actions(), expected, "{code}");
    }
}

#[test]
fn log_trace_and_console_map_to_host_methods() {
    let (outcome, host) = run_script(
        r#"log("a", 1, true, null); console.warn("w"); console.error("e"); console.log("c"); __node("n1");"#,
    );
    assert_eq!(outcome, RunOutcome::Finished);
    let calls: Vec<(String, Value)> = host.calls().into_iter().map(|c| (c.method, c.args)).collect();
    assert_eq!(
        calls,
        vec![
            ("log".to_string(), json!({ "level": "info", "message": "a 1 true null" })),
            ("log".to_string(), json!({ "level": "warn", "message": "w" })),
            ("log".to_string(), json!({ "level": "error", "message": "e" })),
            ("log".to_string(), json!({ "level": "info", "message": "c" })),
            ("trace".to_string(), json!({ "nodeId": "n1" })),
        ]
    );
}

/// `log(<expr>)` for each case and compare the printed text.
fn assert_logs(cases: &[(&str, &str)]) {
    for (expr, expected) in cases {
        let (outcome, host) = run_script(&format!("log({expr})"));
        assert_eq!(outcome, RunOutcome::Finished, "{expr}");
        assert_eq!(host.logs(), vec![expected.to_string()], "{expr}");
    }
}

#[test]
fn api_return_values() {
    assert_logs(&[
        (r#"window.isActive("FIREFOX")"#, "true"),
        (r#"window.isActive("chrome, zen")"#, "false"),
        (r#"window.isActive(" chrome , mozilla ")"#, "true"),
        (r#"window.isActive("/^org\\.mozilla\\./")"#, "true"),
        (r#"window.isActive("/^Org/")"#, "true"),
        (r#"window.isActive("/^Org/u")"#, "false"),
        (r#"window.isActive("/^org/u")"#, "true"),
        (r#"window.isActive("youtube", { by: "title" })"#, "true"),
        (r#"window.isActive("firefox", { by: "title" })"#, "true"),
        (r#"window.isActive("org.mozilla", { by: "title" })"#, "false"),
        (r#"window.isRunning("konsole,steam")"#, "true"),
        (r#"window.isRunning("steam")"#, "false"),
        (r#"window.isRunning("/zsh$/", { by: "title" })"#, "true"),
        (r#"window.active().appId"#, "org.mozilla.firefox"),
        (r#"window.list().length"#, "2"),
        (r#"window.focus("discord")"#, "true"),
        (r#"run("ls", { wait: true }).stdout"#, "out"),
        (r#"run("ls")"#, "undefined"),
        (r#"vars.get("missing")"#, "null"),
        (r#"vars.get("missing", 5)"#, "5"),
        ("macroMode.get()", "true"),
        ("macroMode.toggle()", "false"),
        ("profile.current().name", "Audio"),
        ("desktop.current().count", "4"),
        ("clipboard.get()", "clip"),
        (r#"isTimeBetween("00:00", "00:00")"#, "true"),
        ("event.key, event.type, event.repeat, event.profile.id, event.profile.name", "KP4 press false p_audio Audio"),
    ]);
}

#[test]
fn vars_round_trip_through_the_host() {
    let (outcome, host) = run_script(
        r#"vars.set("n", 2); log(vars.get("n")); log(vars.toggle("on"), vars.toggle("on")); vars.set("o", { a: [1] }); log(vars.get("o"));"#,
    );
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(host.logs(), vec!["2", "true false", "{ a: [1] }"]);
}

#[test]
fn time_ranges_wrap_midnight() {
    let (outcome, host) = run_script(
        r#"
        const d = new Date();
        const at = (m) => `${String(Math.floor(((m % 1440) + 1440) % 1440 / 60)).padStart(2, "0")}:${String((((m % 1440) + 1440) % 1440) % 60).padStart(2, "0")}`;
        const now = d.getHours() * 60 + d.getMinutes();
        log(isTimeBetween(at(now - 1), at(now + 2)), isTimeBetween(at(now + 1), at(now - 1)), isTimeBetween(at(now + 1), at(now + 2)));
        "#,
    );
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(host.logs(), vec!["true false false"]);
}

#[test]
fn log_pretty_prints() {
    assert_logs(&[
        (r#""plain", "text""#, "plain text"),
        (
            "{ a: 1, b: [1, 2], 'c-d': \"s\", e: undefined, f: null }",
            r#"{ a: 1, b: [1, 2], "c-d": "s", e: undefined, f: null }"#,
        ),
        ("[]", "[]"),
        ("{}", "{}"),
        ("new Map([[\"k\", 1]])", r#"Map(1) { "k" => 1 }"#),
        ("new Set()", "Set(0) {}"),
        ("function named() {}", "[Function named]"),
        ("-0, 1n, Symbol(\"s\")", "-0 1n Symbol(s)"),
        ("(() => { const o = { name: \"o\" }; o.self = o; return o; })()", r#"{ name: "o", self: [Circular] }"#),
        ("{ a: { b: { c: { d: { e: 1 } } } } }", "{ a: { b: { c: { d: [Object] } } } }"),
        (
            "{ title: \"YouTube — Mozilla Firefox\", appId: \"org.mozilla.firefox\", resourceClass: \"firefox\" }",
            "{\n  title: \"YouTube — Mozilla Firefox\",\n  appId: \"org.mozilla.firefox\",\n  resourceClass: \"firefox\"\n}",
        ),
        ("new Error(\"boom\").message", "boom"),
    ]);
    let (_, host) = run_script("log(Array.from({ length: 1000 }, () => 0))");
    let printed = &host.logs()[0];
    assert!(printed.ends_with("  0,\n  … 900 more\n]") && printed.lines().count() == 103, "{printed}");

    let (_, host) = run_script("function f() { return new TypeError(\"bad\"); }\nlog(f());");
    assert_eq!(host.logs(), vec!["TypeError: bad\n    at f (line 1:27)\n    at <anonymous> (line 2:5)"]);
}

#[test]
fn wrong_arguments_throw_helpful_type_errors() {
    let cases: &[(&str, &str)] = &[
        ("press()", "TypeError: press: expected at least one shortcut, e.g. press(\"ctrl+c\") (line 1:1)"),
        ("press(5)", "TypeError: press: expected a shortcut as a string, got 5 (line 1:1)"),
        ("keyDown(\"  \")", "TypeError: keyDown: a shortcut is empty (line 1:1)"),
        ("type()", "TypeError: type: expected the text to type, got undefined (line 1:1)"),
        (
            "type(\"x\", { delayMs: -1 })",
            "TypeError: type: delayMs must be a number of milliseconds (0 or more), got -1 (line 1:1)",
        ),
        (
            "media(\"loud\")",
            "TypeError: media: action must be one of \"playPause\", \"next\", \"prev\", \"stop\", \"volumeUp\", \"volumeDown\", \"mute\", got \"loud\" (line 1:1)",
        ),
        ("launch(\"\")", "TypeError: launch: an app is empty (line 1:1)"),
        ("launch(\"x\", 5)", "TypeError: launch: options must be an object, got 5 (line 1:1)"),
        (
            "window.focus(\"x\", { by: \"pid\" })",
            "TypeError: window.focus: by must be one of \"app\", \"title\", got \"pid\" (line 1:7)",
        ),
        (
            "window.isActive(\"/(/\")",
            "TypeError: window.isActive: invalid regular expression /(/: expecting ')' (line 1:7)",
        ),
        (
            "desktop.switchTo(0)",
            "TypeError: desktop.switchTo: target must be \"next\", \"prev\", \"left\", \"right\", \"up\", \"down\" or a desktop number from 1, got 0 (line 1:8)",
        ),
        ("profile.set(-1)", "TypeError: profile.set: expected a profile id, name or index, got -1 (line 1:8)"),
        ("macroMode.set(\"on\")", "TypeError: macroMode.set: expected true or false, got \"on\" (line 1:10)"),
        (
            "toast(\"x\", { kind: \"warn\" })",
            "TypeError: toast: kind must be one of \"info\", \"success\", \"error\", got \"warn\" (line 1:1)",
        ),
        (
            "vars.set(\"f\", () => 1)",
            "TypeError: vars.set: values must be JSON data, got [Function (anonymous)] (line 1:5)",
        ),
        (
            "isTimeBetween(\"9am\", \"17:00\")",
            "TypeError: isTimeBetween: from must be a time like \"09:30\", got \"9am\" (line 1:1)",
        ),
        ("sleep(\"soon\")", "TypeError: sleep: expected a number of milliseconds, e.g. sleep(100) (line 1:1)"),
    ];
    for (code, expected) in cases {
        let (outcome, _) = run_script(code);
        assert_eq!(outcome, RunOutcome::Failed(expected.to_string()), "{code}");
    }
}

#[test]
fn host_errors_become_catchable_exceptions_with_the_host_message() {
    let (outcome, _) = run_script("press(\"a\");\npress(\"bad\");");
    assert_eq!(outcome, RunOutcome::Failed("unknown key \"bad\" (line 2:1)".into()));

    let (outcome, host) = run_script("try { press(\"bad\") } catch (e) { log(e instanceof Error, e.message) }");
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(host.logs(), vec!["true unknown key \"bad\""]);
}

#[test]
fn api_globals_cannot_be_clobbered() {
    for code in [
        "press = null",
        "window.focus = null",
        "globalThis.press = null",
        "delete globalThis.toast",
        "event = {}",
        "vars.extra = 1",
        "event.key = \"KP1\"",
        "event.profile.id = \"x\"",
    ] {
        let (outcome, _) = run_script(code);
        assert!(matches!(&outcome, RunOutcome::Failed(m) if m.starts_with("TypeError")), "{code}: {outcome:?}");
    }
    let (_, engine) = setup();
    engine.set_library("function press() {}".into());
    let run = start(&engine, 1, script("log(1)"));
    assert!(matches!(run.outcome(), RunOutcome::Failed(m) if m.contains("library line 1")));

    // Shadowing inside the macro is local and harmless.
    let (outcome, host) = run_script("const window = 5; let press = 6; log(window + press);");
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(host.logs(), vec!["11"]);
}
