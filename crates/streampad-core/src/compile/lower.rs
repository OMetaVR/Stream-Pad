//! Node catalog (docs/SPEC.md §3.3): validates a node's params and lowers it to the JS it stands for.

use super::js::{call_stmt, interpolated, json_lit, number_lit, options_object, own_lines, string_lit};
use crate::combo::{ComboError, parse_combo};
use crate::config::Node;
use serde_json::{Map, Value};

pub(super) const PRESS_TRIGGER: &str = "trigger.press";
pub(super) const RELEASE_TRIGGER: &str = "trigger.release";

/// What a node does, ready for emission.
#[derive(Debug, Clone)]
pub(super) enum Op {
    Trigger,
    /// One complete statement, e.g. `press("ctrl+c");`.
    Stmt(String),
    /// Raw user statements (`script` node), emitted as a block.
    Script(String),
    If(Condition),
    /// Loop count, as a JS number literal.
    Repeat(String),
    Stop,
}

impl Op {
    pub(super) fn ports(&self) -> &'static [&'static str] {
        match self {
            Op::Trigger | Op::Stmt(_) | Op::Script(_) => &["out"],
            Op::If(_) => &["true", "false"],
            Op::Repeat(_) => &["body", "done"],
            Op::Stop => &[],
        }
    }
}

/// A boolean JS expression and its negation, both written the way a person would.
#[derive(Debug, Clone)]
pub(super) struct Condition {
    pub(super) expr: String,
    pub(super) negated: String,
}

impl Condition {
    /// For call/member expressions, which can be negated with a bare `!`.
    fn call(expr: String) -> Self {
        Condition { negated: format!("!{expr}"), expr }
    }

    fn compare(lhs: &str, eq: &str, neq: &str, rhs: &str) -> Self {
        Condition { expr: format!("{lhs} {eq} {rhs}"), negated: format!("{lhs} {neq} {rhs}") }
    }
}

const MEDIA_ACTIONS: &[&str] = &["playPause", "next", "prev", "stop", "volumeUp", "volumeDown", "mute"];
const WINDOW_ACTIONS: &[&str] = &["minimize", "maximize", "fullscreen", "close", "keepAbove", "center"];
const DIRECTIONS: &[&str] = &["next", "prev", "left", "right", "up", "down"];

pub(super) fn lower(node: &Node) -> Result<Op, String> {
    let p = Params(&node.params);
    let op = match node.kind.as_str() {
        PRESS_TRIGGER | RELEASE_TRIGGER => Op::Trigger,
        "key.shortcut" => Op::Stmt(call_stmt("press", [p.combo()?])),
        "key.down" => Op::Stmt(call_stmt("keyDown", [p.combo()?])),
        "key.up" => Op::Stmt(call_stmt("keyUp", [p.combo()?])),
        "key.type" => {
            let text = p.text("text").filter(|t| !t.is_empty()).ok_or("Enter the text to type")?;
            let delay = p.number("delayMs", 8.0, "Delay between keys")?;
            let opts = options_object(&[("delayMs", (delay != 8.0).then(|| number_lit(delay)))]);
            Op::Stmt(call_stmt("type", std::iter::once(interpolated(&text)).chain(opts)))
        }
        "media" => Op::Stmt(call_stmt("media", [string_lit(p.choice("action", MEDIA_ACTIONS, "media action")?)])),
        "app.launch" => {
            let app = p.required("app", "Choose an app to launch")?;
            let focus = p.boolean("focusIfRunning", true)?;
            let args = p.text("args").map(|a| a.trim().to_string()).unwrap_or_default();
            let opts = options_object(&[
                ("focusIfRunning", (!focus).then(|| "false".to_string())),
                ("args", (!args.is_empty()).then(|| string_lit(&args))),
            ]);
            Op::Stmt(call_stmt("launch", std::iter::once(string_lit(&app)).chain(opts)))
        }
        "app.close" => {
            let app = p.required("app", "Choose an app to close")?;
            let opts = options_object(&[("force", p.boolean("force", false)?.then(|| "true".to_string()))]);
            Op::Stmt(call_stmt("closeApp", std::iter::once(string_lit(&app)).chain(opts)))
        }
        "open.url" => Op::Stmt(call_stmt("openUrl", [interpolated(&p.required("url", "Enter a URL")?)])),
        "open.path" => Op::Stmt(call_stmt("openPath", [interpolated(&p.required("path", "Choose a file or folder")?)])),
        "run.command" => {
            let command = p.required("command", "Enter a command to run")?;
            let opts = options_object(&[("wait", p.boolean("wait", false)?.then(|| "true".to_string()))]);
            Op::Stmt(call_stmt("run", std::iter::once(interpolated(&command)).chain(opts)))
        }
        "window.focus" => {
            let pattern = p.required("match", "Enter the app or window title to focus")?;
            let by = p.choice_or("by", &["app", "title"], "app", "match mode")?;
            let opts = options_object(&[("by", (by == "title").then(|| string_lit(by)))]);
            Op::Stmt(call_stmt("window.focus", std::iter::once(string_lit(&pattern)).chain(opts)))
        }
        "window.moveToOutput" => {
            Op::Stmt(call_stmt("window.moveToOutput", [string_lit(&p.required("target", "Choose a monitor")?)]))
        }
        "window.action" => {
            let action = p.choice("action", WINDOW_ACTIONS, "window action")?;
            Op::Stmt(format!("window.{action}();"))
        }
        "desktop.switch" => Op::Stmt(call_stmt("desktop.switchTo", [p.desktop_target()?])),
        "desktop.moveWindow" => {
            let target = p.desktop_target()?;
            let opts = options_object(&[("follow", p.boolean("follow", false)?.then(|| "true".to_string()))]);
            Op::Stmt(call_stmt("desktop.moveWindow", std::iter::once(target).chain(opts)))
        }
        "desktop.create" => Op::Stmt("desktop.create();".into()),
        "desktop.remove" => Op::Stmt("desktop.remove();".into()),
        "desktop.overview" => Op::Stmt("desktop.overview();".into()),
        "kde.shortcut" => {
            let component = p.required("component", "Choose a shortcut")?;
            let shortcut = p.required("shortcut", "Choose a shortcut")?;
            Op::Stmt(call_stmt("kde.shortcut", [string_lit(&component), string_lit(&shortcut)]))
        }
        "profile.switch" => Op::Stmt(profile_switch(&p)?),
        "macroMode.set" => Op::Stmt(
            match p.choice("value", &["on", "off", "toggle"], "macro mode value")? {
                "on" => "macroMode.set(true);",
                "off" => "macroMode.set(false);",
                _ => "macroMode.toggle();",
            }
            .into(),
        ),
        "overlay.set" => {
            let value = p.choice("value", &["show", "hide", "toggle", "peek"], "overlay action")?;
            Op::Stmt(format!("overlay.{value}();"))
        }
        "toast" => {
            let text = p.text("text").filter(|t| !t.trim().is_empty()).ok_or("Enter the toast text")?;
            let duration = p.number("durationMs", 1600.0, "Duration")?;
            let opts = options_object(&[("durationMs", (duration != 1600.0).then(|| number_lit(duration)))]);
            Op::Stmt(call_stmt("toast", std::iter::once(interpolated(&text)).chain(opts)))
        }
        "notify" => {
            let title = p.text("title").filter(|t| !t.trim().is_empty()).ok_or("Enter a notification title")?;
            let body = p.text("body").filter(|b| !b.is_empty()).map(|b| interpolated(&b));
            Op::Stmt(call_stmt("notify", std::iter::once(interpolated(&title)).chain(body)))
        }
        "var.set" => {
            let name = p.required("name", "Enter a variable name")?;
            Op::Stmt(call_stmt("vars.set", [string_lit(&name), json_lit(&p.json_value("value"))]))
        }
        "var.toggle" => Op::Stmt(call_stmt("vars.toggle", [string_lit(&p.required("name", "Enter a variable name")?)])),
        "clipboard.set" => Op::Stmt(call_stmt("clipboard.set", [interpolated(&p.text("text").unwrap_or_default())])),
        "sound.play" => Op::Stmt(call_stmt("sound.play", [string_lit(&p.required("path", "Choose a sound file")?)])),
        "delay" => Op::Stmt(call_stmt("sleep", [number_lit(p.number("ms", 100.0, "Delay")?)])),
        "script" => Op::Script(p.text("code").unwrap_or_default()),
        "flow.if" => Op::If(condition(node.params.get("condition"))?),
        "flow.repeat" => Op::Repeat(p.count()?),
        "flow.stop" => Op::Stop,
        other => return Err(format!("Unknown node type \"{other}\"")),
    };
    Ok(op)
}

fn profile_switch(p: &Params) -> Result<String, String> {
    if let Some(Value::Number(n)) = p.0.get("target") {
        return Ok(call_stmt("profile.set", [n.to_string()]));
    }
    Ok(match p.required("target", "Choose a profile")?.as_str() {
        "next" => "profile.next();".into(),
        "prev" => "profile.prev();".into(),
        id => call_stmt("profile.set", [string_lit(id)]),
    })
}

fn condition(v: Option<&Value>) -> Result<Condition, String> {
    let Some(Value::Object(c)) = v else {
        return Err("Choose a condition".into());
    };
    let p = Params(c);
    let pattern = |what: &str| p.required("match", what);
    Ok(match p.text("type").as_deref() {
        Some("activeApp") => {
            Condition::call(call_expr("window.isActive", &[string_lit(&pattern("Enter the app to check for")?)]))
        }
        Some("activeTitle") => Condition::call(call_expr(
            "window.isActive",
            &[string_lit(&pattern("Enter the window title to check for")?), "{ by: \"title\" }".into()],
        )),
        Some("appRunning") => {
            Condition::call(call_expr("window.isRunning", &[string_lit(&pattern("Enter the app to check for")?)]))
        }
        Some("var") => var_condition(&p)?,
        Some("profile") => {
            let id = p.required("profileId", "Choose a profile")?;
            Condition::compare("profile.current().id", "===", "!==", &string_lit(&id))
        }
        Some("time") => {
            let from = p.time("from")?;
            let to = p.time("to")?;
            Condition::call(call_expr("isTimeBetween", &[string_lit(&from), string_lit(&to)]))
        }
        Some("expr") => {
            let code = p.text("code").unwrap_or_default();
            let code = code.trim().trim_end_matches(';').trim_end();
            if code.is_empty() {
                return Err("Enter a condition expression".into());
            }
            let code = if code.contains('\n') || code.contains("//") { own_lines(code) } else { code.to_string() };
            Condition { negated: format!("!({code})"), expr: code }
        }
        Some(other) => return Err(format!("Unknown condition type \"{other}\"")),
        None => return Err("Choose a condition".into()),
    })
}

fn var_condition(p: &Params) -> Result<Condition, String> {
    let name = string_lit(&p.required("name", "Enter a variable name")?);
    let get = format!("vars.get({name})");
    Ok(match p.choice_or("op", &["truthy", "falsy", "eq", "neq"], "truthy", "comparison")? {
        "truthy" => Condition::call(get),
        "falsy" => Condition { expr: format!("!{get}"), negated: get },
        op => {
            let value = p.json_value("value");
            let c = match value {
                Value::Array(_) | Value::Object(_) => {
                    Condition::compare(&format!("JSON.stringify({get})"), "===", "!==", &string_lit(&value.to_string()))
                }
                _ => Condition::compare(&get, "===", "!==", &json_lit(&value)),
            };
            if op == "neq" { Condition { expr: c.negated, negated: c.expr } } else { c }
        }
    })
}

fn call_expr(callee: &str, args: &[String]) -> String {
    format!("{callee}({})", args.join(", "))
}

/// Typed access to a node's `params` object.
struct Params<'a>(&'a Map<String, Value>);

impl Params<'_> {
    /// A text param; numbers are accepted and stringified. Missing / null → `None`.
    fn text(&self, key: &str) -> Option<String> {
        match self.0.get(key)? {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        }
    }

    /// A trimmed, non-empty text param; `missing` is the message when it is absent or blank.
    fn required(&self, key: &str, missing: &str) -> Result<String, String> {
        self.text(key).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).ok_or_else(|| missing.to_string())
    }

    fn combo(&self) -> Result<String, String> {
        let combo = self.required("combo", "Record a shortcut")?;
        match parse_combo(&combo) {
            Ok(_) => Ok(string_lit(&combo)),
            Err(ComboError::Empty) => Err("Record a shortcut".into()),
            Err(e) => Err(format!("Shortcut: {e}")),
        }
    }

    fn boolean(&self, key: &str, default: bool) -> Result<bool, String> {
        match self.0.get(key) {
            None | Some(Value::Null) => Ok(default),
            Some(Value::Bool(b)) => Ok(*b),
            Some(other) => Err(format!("\"{key}\" must be true or false, not {other}")),
        }
    }

    /// A non-negative finite number (numeric strings accepted); missing / null → `default`.
    fn number(&self, key: &str, default: f64, what: &str) -> Result<f64, String> {
        let n = match self.0.get(key) {
            None | Some(Value::Null) => return Ok(default),
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) if s.trim().is_empty() => return Ok(default),
            Some(Value::String(s)) => s.trim().parse::<f64>().ok(),
            Some(_) => None,
        };
        n.filter(|n| n.is_finite() && *n >= 0.0)
            .ok_or_else(|| format!("{what} must be a number of milliseconds (0 or more)"))
    }

    fn count(&self) -> Result<String, String> {
        let n =
            self.number("count", 2.0, "Count").map_err(|_| "Count must be a whole number (0 or more)".to_string())?;
        if n.fract() != 0.0 {
            return Err("Count must be a whole number (0 or more)".into());
        }
        Ok(number_lit(n))
    }

    fn choice<'c>(&self, key: &str, allowed: &[&'c str], what: &str) -> Result<&'c str, String> {
        let article = if what.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
        let value = self.text(key).ok_or_else(|| format!("Choose {article} {what}"))?;
        allowed.iter().find(|a| **a == value).copied().ok_or_else(|| format!("Unknown {what} \"{value}\""))
    }

    fn choice_or<'c>(&self, key: &str, allowed: &[&'c str], default: &'c str, what: &str) -> Result<&'c str, String> {
        if self.text(key).is_none_or(|v| v.is_empty()) {
            return Ok(default);
        }
        self.choice(key, allowed, what)
    }

    /// `"next"`-style direction or a 1-based desktop number, as a JS literal.
    fn desktop_target(&self) -> Result<String, String> {
        let target = self.required("target", "Choose a desktop")?;
        if DIRECTIONS.contains(&target.as_str()) {
            return Ok(string_lit(&target));
        }
        match target.parse::<u32>() {
            Ok(n) if n >= 1 => Ok(n.to_string()),
            _ => {
                Err(format!("Unknown desktop \"{target}\" (use next, prev, left, right, up, down or a number from 1)"))
            }
        }
    }

    /// The `var.set` value convention: a string param holding valid JSON is that JSON value, any other
    /// string is itself; non-string params are used as they are. Missing → `""`.
    fn json_value(&self, key: &str) -> Value {
        match self.0.get(key) {
            None => Value::from(""),
            Some(Value::String(s)) => serde_json::from_str(s).unwrap_or_else(|_| Value::from(s.as_str())),
            Some(v) => v.clone(),
        }
    }

    /// `"HH:MM"` (24 h), normalised to two-digit hours.
    fn time(&self, key: &str) -> Result<String, String> {
        let raw = self.text(key).unwrap_or_default();
        let parsed = raw.trim().split_once(':').and_then(|(h, m)| {
            let (h, m) = (h.parse::<u8>().ok()?, m.parse::<u8>().ok()?);
            (h < 24 && m < 60 && raw.trim().len() <= 5).then(|| format!("{h:02}:{m:02}"))
        });
        parsed.ok_or_else(|| format!("Time \"{}\" must look like 09:30 (24-hour)", raw.trim()))
    }
}
