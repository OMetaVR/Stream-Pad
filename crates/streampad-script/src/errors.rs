//! Turns a JS failure into the text of `RunOutcome::Failed` / `check_syntax` errors:
//! `"<message> (line L:C)"`, followed by the stack (one `    at …` line per frame) when there is more
//! than one frame worth showing. Positions refer to the user's macro code; prelude frames are hidden.

use crate::source::{LIBRARY_FILE, MACRO_FILE, PRELUDE_FILE, user_position};
use crate::worker::MEMORY_LIMIT;
use rquickjs::convert::Coerced;
use rquickjs::{CaughtError, Ctx, Exception, FromJs, Value};
use std::fmt;

pub(crate) fn describe<'js>(ctx: &Ctx<'js>, err: rquickjs::Error, code: &str) -> String {
    match CaughtError::from_error(ctx, err) {
        CaughtError::Exception(e) => describe_exception(&e, code),
        CaughtError::Value(v) => describe_value(ctx, v),
        CaughtError::Error(e) => e.to_string(),
    }
}

/// A thrown or rejected value (an Error object or anything else).
pub(crate) fn describe_thrown<'js>(ctx: &Ctx<'js>, v: Value<'js>, code: &str) -> String {
    match v.as_object().cloned().and_then(Exception::from_object) {
        Some(e) => describe_exception(&e, code),
        None => describe_value(ctx, v),
    }
}

fn describe_exception(e: &Exception, code: &str) -> String {
    let name: Option<String> = e.get("name").ok().flatten();
    let stack = e.stack().unwrap_or_default();
    let frames: Vec<Frame> =
        stack.lines().filter_map(|l| Frame::parse(l, code)).filter(|f| f.file != PRELUDE_FILE).collect();
    let mut text = summary(name.as_deref(), &e.message().unwrap_or_default());
    if let Some(at) = frames.iter().find(|f| f.file == MACRO_FILE).or(frames.first()) {
        text.push_str(&format!(" ({})", at.position()));
    }
    if frames.len() > 1 {
        for f in &frames {
            text.push_str(&format!("\n    at {f}"));
        }
    }
    text
}

fn summary(name: Option<&str>, message: &str) -> String {
    match name {
        Some("InternalError") if message == "out of memory" => {
            format!("out of memory (a macro can use up to {} MB)", MEMORY_LIMIT >> 20)
        }
        // QuickJS's wording for running out of input mid-statement.
        Some("SyntaxError") if message == "unexpected token in expression: ''" => {
            "SyntaxError: unexpected end of code (is a closing }, ) or ] missing?)".into()
        }
        None | Some("" | "Error") if message.is_empty() => "error".into(),
        None | Some("" | "Error") => message.into(),
        Some(name) if message.is_empty() => name.into(),
        Some(name) => format!("{name}: {message}"),
    }
}

/// `throw "text"` / `throw {…}`: thrown values that are not Error objects carry no position.
fn describe_value<'js>(ctx: &Ctx<'js>, v: Value<'js>) -> String {
    if v.is_object()
        && let Ok(Some(json)) = ctx.json_stringify(v.clone())
        && let Ok(json) = json.to_string()
    {
        return json;
    }
    Coerced::<String>::from_js(ctx, v).map(|s| s.0).unwrap_or_else(|_| "uncaught exception".into())
}

struct Frame {
    function: Option<String>,
    file: String,
    line: u32,
    col: u32,
}

impl Frame {
    /// Parses a QuickJS stack line: `    at name (file:L:C)` or `    at file:L:C`. Native frames have no
    /// position and are dropped.
    fn parse(line: &str, code: &str) -> Option<Frame> {
        let rest = line.trim().strip_prefix("at ")?;
        let (function, location) = match rest.strip_suffix(')').and_then(|r| r.rsplit_once(" (")) {
            Some((function, location)) => (Some(function.to_string()), location),
            None => (None, rest),
        };
        let mut parts = location.rsplitn(3, ':');
        let col: u32 = parts.next()?.parse().ok()?;
        let line: u32 = parts.next()?.parse().ok()?;
        let file = parts.next()?.to_string();
        let (line, col) = if file == MACRO_FILE { user_position(code, line, col) } else { (line, col) };
        Some(Frame { function, file, line, col })
    }

    fn position(&self) -> String {
        match self.file.as_str() {
            MACRO_FILE => format!("line {}:{}", self.line, self.col),
            LIBRARY_FILE => format!("library line {}:{}", self.line, self.col),
            file => format!("{file} {}:{}", self.line, self.col),
        }
    }
}

impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.function {
            Some(function) => write!(f, "{function} ({})", self.position()),
            None => f.write_str(&self.position()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_parse_and_map() {
        let f = Frame::parse("    at helper (library:2:10)", "").unwrap();
        assert_eq!(f.to_string(), "helper (library line 2:10)");
        let f = Frame::parse("    at macro:3:5", "a\nb\nccccc").unwrap();
        assert_eq!(f.to_string(), "line 3:5");
        assert!(Frame::parse("    at push (native)", "").is_none());
        assert!(Frame::parse("garbage", "").is_none());
    }

    #[test]
    fn summaries() {
        assert_eq!(summary(Some("Error"), "boom"), "boom");
        assert_eq!(summary(Some("TypeError"), "x is not a function"), "TypeError: x is not a function");
        assert_eq!(summary(None, ""), "error");
        assert_eq!(summary(Some("InternalError"), "out of memory"), "out of memory (a macro can use up to 64 MB)");
        assert_eq!(
            summary(Some("SyntaxError"), "unexpected token in expression: ''"),
            "SyntaxError: unexpected end of code (is a closing }, ) or ] missing?)"
        );
    }
}
