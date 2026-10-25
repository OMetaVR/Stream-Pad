//! The KWin scripts (prelude.js, ops.js, watcher.js) run in QuickJS against a mock of KWin's scripting API, so
//! window and desktop actions are tested without touching a real desktop.

use super::{OPS_JS, WATCHER_JS, compose_script};
use rquickjs::{Context, Runtime};
use serde_json::{Value, json};

/// A two-monitor, four-desktop (2×2 grid) KWin with recordable `callDBus` and connectable signals.
const MOCK_KWIN: &str = r#"
var __calls = [];
function callDBus() { __calls.push(Array.prototype.slice.call(arguments)); }
function signal() {
    var handlers = [];
    var s = function () { var a = arguments; handlers.slice().forEach(function (h) { h.apply(null, a); }); };
    s.connect = function (h) { handlers.push(h); };
    s.disconnect = function (h) { var i = handlers.indexOf(h); if (i >= 0) handlers.splice(i, 1); };
    s.count = function () { return handlers.length; };
    return s;
}
var KWin = { MaximizeArea: 2 };
function rect(x, y, w, h) { return { x: x, y: y, width: w, height: h }; }
function output(name, x) { return { name: name, geometry: rect(x, 0, 2560, 1440), devicePixelRatio: 1, model: "M27" }; }
var DP1 = output("DP-1", 0), DP2 = output("DP-2", 2560);
function win(id, app, out, extra) {
    var w = { internalId: "{" + id + "}", caption: app + " window", resourceName: app, resourceClass: app,
        desktopFileName: app, pid: 42, output: out, active: false, normalWindow: true, dialog: false,
        specialWindow: false, skipTaskbar: false, deleted: false, minimized: false, fullScreen: false,
        keepAbove: false, maximizeMode: 0, frameGeometry: rect(100, 100, 800, 600), desktops: [],
        captionChanged: signal() };
    for (var k in extra) w[k] = extra[k];
    w.setMaximize = function (v, h) { this.maximizeMode = (v ? 1 : 0) | (h ? 2 : 0); };
    w.closeWindow = function () { this.closed = true; };
    return w;
}
var D = [{ name: "Desktop 1" }, { name: "Desktop 2" }, { name: "Desktop 3" }, { name: "Desktop 4" }];
var workspace = {
    screens: [DP1, DP2], screenOrder: [DP2, DP1],
    desktops: D, currentDesktop: D[0], desktopGridWidth: 2, virtualDesktopNavigationWrapsAround: false,
    stackingOrder: [], activeWindow: null, cursorPos: { x: 3000, y: 10 },
    screenAt: function (p) { return p.x >= 2560 ? DP2 : DP1; },
    clientArea: function (option, out) { return option === 2 ? rect(out.geometry.x, 0, 2560, 1400) : null; },
    sendClientToScreen: function (w, o) { w.output = o; },
    createDesktop: function (pos, name) { this.desktops.splice(pos, 0, { name: name }); },
    removeDesktop: function (d) { this.desktops.splice(this.desktops.indexOf(d), 1); },
    windowActivated: signal(), screensChanged: signal(), virtualScreenGeometryChanged: signal(),
    screenOrderChanged: signal()
};
var dolphin = win("1", "org.kde.dolphin", DP1);
var zen = win("2", "zen", DP2);
var panel = win("3", "plasmashell", DP1, { specialWindow: true, normalWindow: false, skipTaskbar: true });
workspace.stackingOrder = [panel, dolphin, zen];
"#;

struct Kwin {
    _rt: Runtime,
    ctx: Context,
}

impl Kwin {
    /// The mock plus `setup` (JS run after it, e.g. to pick the active window).
    fn new(setup: &str) -> Kwin {
        let rt = Runtime::new().unwrap();
        let ctx = Context::full(&rt).unwrap();
        let kwin = Kwin { _rt: rt, ctx };
        kwin.exec(MOCK_KWIN);
        kwin.exec(setup);
        kwin
    }

    fn exec(&self, js: &str) {
        self.ctx.with(|ctx| {
            if let Err(e) = ctx.eval::<(), _>(js) {
                let detail = ctx.catch().into_exception().map(|ex| format!("{ex}")).unwrap_or_default();
                panic!("script failed: {e} {detail}");
            }
        });
    }

    fn eval(&self, expr: &str) -> Value {
        let json: String = self.ctx.with(|ctx| ctx.eval(format!("JSON.stringify({expr})")).unwrap());
        serde_json::from_str(&json).unwrap()
    }

    /// Run one op and return its decoded answer (`{"ok": …}` or `{"error": …}`).
    fn op(&self, op: &str, args: Value) -> Value {
        self.exec("__calls = [];");
        self.exec(&compose_script(":1.42", op, "tok", &args, OPS_JS));
        let call = self.eval("__calls[__calls.length - 1]");
        assert_eq!(call[0], ":1.42", "answers go to the daemon's unique name");
        assert_eq!((call[3].as_str(), call[4].as_str()), (Some("Reply"), Some("tok")));
        serde_json::from_str(call[5].as_str().unwrap()).unwrap()
    }

    fn ok(&self, op: &str, args: Value) -> Value {
        let answer = self.op(op, args);
        assert!(answer.get("error").is_none(), "{op} failed: {answer}");
        answer["ok"].clone()
    }
}

#[test]
fn queries_report_app_windows_outputs_and_cursor() {
    let k = Kwin::new("workspace.activeWindow = zen; zen.active = true;");
    let windows = k.ok("windows", json!({}));
    let ids: Vec<&str> = windows.as_array().unwrap().iter().map(|w| w["appId"].as_str().unwrap()).collect();
    assert_eq!(ids, ["zen", "org.kde.dolphin"], "topmost first, panels excluded");
    assert_eq!(
        windows[0],
        json!({ "id": "{2}", "title": "zen window", "appId": "zen", "resourceClass": "zen", "desktopFile": "zen",
                "pid": 42, "output": "DP-2", "active": true })
    );
    assert_eq!(k.ok("cursorOutput", json!({})), "DP-2");
    let outputs = k.ok("outputs", json!({}));
    assert_eq!(outputs[0], json!({ "name": "DP-1", "x": 0, "y": 0, "width": 2560, "height": 1440, "scale": 1, "primary": false, "model": "M27" }));
    assert_eq!(outputs[1]["primary"], true, "primary = first in screen order");
    assert_eq!(k.ok("desktopCurrent", json!({})), json!({ "index": 1, "name": "Desktop 1", "count": 4 }));
}

#[test]
fn focus_matches_like_conditions_and_cycles() {
    let k = Kwin::new("workspace.activeWindow = dolphin;");
    assert_eq!(k.ok("focus", json!({ "match": "firefox, ZEN", "by": "app" })), true);
    assert_eq!(k.eval("workspace.activeWindow.resourceName"), "zen");
    assert_eq!(k.ok("focus", json!({ "match": "/^org\\.kde\\./", "by": "app" })), true, "regex form");
    assert_eq!(k.eval("workspace.activeWindow.resourceName"), "org.kde.dolphin");
    assert_eq!(k.ok("focus", json!({ "match": "zen window", "by": "title" })), true);
    assert_eq!(k.ok("focus", json!({ "match": "nothing", "by": "app" })), false);
    assert_eq!(k.ok("focus", json!({ "match": "plasmashell", "by": "app" })), false, "never focuses panels");

    // Two windows of one app: repeated presses cycle through them.
    let k = Kwin::new("var zen2 = win('4', 'zen', DP1); workspace.stackingOrder.push(zen2); workspace.activeWindow = zen2;");
    k.ok("focusApp", json!({ "ids": ["ZEN"] }));
    assert_eq!(k.eval("workspace.activeWindow.internalId"), "{2}");
}

#[test]
fn focus_app_unminimizes_and_matches_ids_exactly() {
    let k = Kwin::new("dolphin.minimized = true; workspace.activeWindow = zen;");
    assert_eq!(k.ok("focusApp", json!({ "ids": ["dolphin-not", "org.kde.dolphin"] })), true);
    assert_eq!(k.eval("[workspace.activeWindow.resourceName, dolphin.minimized]"), json!(["org.kde.dolphin", false]));
    assert_eq!(k.ok("focusApp", json!({ "ids": ["dolph"] })), false, "ids are not substrings");
}

#[test]
fn window_actions_change_the_active_window() {
    let k = Kwin::new("workspace.activeWindow = dolphin;");
    k.ok("windowAction", json!({ "action": "maximize" }));
    assert_eq!(k.eval("dolphin.maximizeMode"), 3);
    k.ok("windowAction", json!({ "action": "maximize" }));
    assert_eq!(k.eval("dolphin.maximizeMode"), 0, "maximize toggles");
    for action in ["fullscreen", "keepAbove", "minimize", "close"] {
        k.ok("windowAction", json!({ "action": action }));
    }
    assert_eq!(k.eval("[dolphin.fullScreen, dolphin.keepAbove, dolphin.minimized, dolphin.closed]"), json!([true, true, true, true]));
    k.ok("windowAction", json!({ "action": "center" }));
    assert_eq!(k.eval("dolphin.frameGeometry"), json!({ "x": 880, "y": 400, "width": 800, "height": 600 }));

    let none = Kwin::new("workspace.activeWindow = panel;");
    assert_eq!(none.op("windowAction", json!({ "action": "close" }))["error"], "no active window");
}

#[test]
fn move_to_output_by_direction_order_and_name() {
    let k = Kwin::new("workspace.activeWindow = dolphin;");
    assert_eq!(k.ok("moveToOutput", json!({ "target": "right" })), "DP-2");
    assert_eq!(k.eval("dolphin.output.name"), "DP-2");
    assert_eq!(k.ok("moveToOutput", json!({ "target": "right" })), Value::Null, "nothing further right: no-op");
    assert_eq!(k.ok("moveToOutput", json!({ "target": "up" })), Value::Null);
    assert_eq!(k.ok("moveToOutput", json!({ "target": "next" })), "DP-1", "next wraps");
    assert_eq!(k.ok("moveToOutput", json!({ "target": "DP-2" })), "DP-2");
    assert_eq!(k.op("moveToOutput", json!({ "target": "HDMI-1" }))["error"], "no monitor named HDMI-1");
}

#[test]
fn desktop_navigation_follows_the_grid_and_wrap_setting() {
    let k = Kwin::new("");
    let current = |k: &Kwin| k.eval("workspace.desktops.indexOf(workspace.currentDesktop)");
    k.ok("desktopSwitch", json!({ "target": "right" }));
    assert_eq!(current(&k), 1);
    k.ok("desktopSwitch", json!({ "target": "down" }));
    assert_eq!(current(&k), 3);
    k.ok("desktopSwitch", json!({ "target": "next" }));
    assert_eq!(current(&k), 3, "no wrap-around unless KDE is set to wrap");
    k.exec("workspace.virtualDesktopNavigationWrapsAround = true;");
    k.ok("desktopSwitch", json!({ "target": "next" }));
    assert_eq!(current(&k), 0);
    k.ok("desktopSwitch", json!({ "target": "up" }));
    assert_eq!(current(&k), 2, "up from the top row wraps to the bottom of the column");
    k.ok("desktopSwitch", json!({ "target": 2 }));
    assert_eq!(current(&k), 1);
    assert_eq!(k.op("desktopSwitch", json!({ "target": 9 }))["error"], "there is no desktop 9");
    assert!(k.op("desktopSwitch", json!({ "target": "sideways" }))["error"].as_str().unwrap().contains("unknown desktop target"));
}

#[test]
fn desktop_window_moves_create_and_remove() {
    let k = Kwin::new("workspace.activeWindow = zen;");
    k.ok("desktopMoveWindow", json!({ "target": "next", "follow": false }));
    assert_eq!(k.eval("[zen.desktops[0].name, workspace.currentDesktop.name]"), json!(["Desktop 2", "Desktop 1"]));
    k.ok("desktopMoveWindow", json!({ "target": 4, "follow": true }));
    assert_eq!(k.eval("[zen.desktops[0].name, workspace.currentDesktop.name, workspace.activeWindow.resourceName]"), json!(["Desktop 4", "Desktop 4", "zen"]));

    k.ok("desktopCreate", json!({}));
    assert_eq!(k.ok("desktopCurrent", json!({})), json!({ "index": 5, "name": "Desktop 5", "count": 5 }), "a new desktop is switched to");
    k.ok("desktopRemove", json!({}));
    assert_eq!(k.eval("workspace.desktops.length"), 4);

    let single = Kwin::new("workspace.desktops = [D[0]];");
    assert_eq!(single.op("desktopRemove", json!({}))["error"], "can't remove the only desktop");
}

#[test]
fn unknown_ops_answer_with_an_error() {
    let k = Kwin::new("");
    assert_eq!(k.op("selfDestruct", json!({}))["error"], "unknown operation selfDestruct");
}

#[test]
fn watcher_reports_activation_title_and_output_changes() {
    let k = Kwin::new("workspace.activeWindow = dolphin;");
    k.exec(&compose_script(":1.42", "watch", "", &Value::Null, WATCHER_JS));
    let events = |k: &Kwin| -> Vec<Value> {
        let calls = k.eval("__calls.splice(0)");
        calls.as_array().unwrap().iter().map(|c| {
            assert_eq!(c[3], "Event");
            serde_json::from_str(c[4].as_str().unwrap()).unwrap()
        }).collect()
    };
    let initial = events(&k);
    assert_eq!(initial[0]["type"], "activeWindow");
    assert_eq!(initial[0]["window"]["appId"], "org.kde.dolphin");
    assert_eq!(initial[1]["type"], "outputs");
    assert_eq!(initial[1]["outputs"].as_array().unwrap().len(), 2);

    k.exec("workspace.windowActivated(zen);");
    assert_eq!(events(&k)[0]["window"]["appId"], "zen");
    k.exec("zen.caption = 'New tab'; zen.captionChanged();");
    assert_eq!(events(&k)[0]["window"]["title"], "New tab", "title changes of the active window are reported");
    assert_eq!(k.eval("[dolphin.captionChanged.count(), zen.captionChanged.count()]"), json!([0, 1]), "only the active window is followed");
    k.exec("workspace.windowActivated(null);");
    assert_eq!(events(&k)[0]["window"], Value::Null);
    k.exec("workspace.screensChanged();");
    assert_eq!(events(&k)[0]["type"], "outputs");
}
