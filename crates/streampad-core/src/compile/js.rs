//! JavaScript source helpers: literals built through JSON (never by pasting user text into code),
//! `{{var}}` template literals, and an indenting line writer.

use serde_json::Value;

/// `"text"` as a JS string literal (JSON string syntax is valid JS).
pub(super) fn string_lit(s: &str) -> String {
    Value::from(s).to_string()
}

/// Any JSON value as a JS expression (`3`, `true`, `null`, `{"a":1}`).
pub(super) fn json_lit(v: &Value) -> String {
    v.to_string()
}

/// A finite number as a JS literal (`100`, `12.5`).
pub(super) fn number_lit(n: f64) -> String {
    format!("{n}")
}

/// A text parameter that supports `{{name}}` interpolation: a plain string literal when it has no
/// placeholders, otherwise a template literal reading each variable with `vars.get(name, "")`.
pub(super) fn interpolated(text: &str) -> String {
    let parts = split_placeholders(text);
    if !parts.iter().any(|p| matches!(p, Part::Var(_))) {
        return string_lit(text);
    }
    let mut out = String::from("`");
    for part in parts {
        match part {
            Part::Text(t) => push_template_text(&mut out, t),
            Part::Var(name) => {
                out.push_str("${vars.get(");
                out.push_str(&string_lit(name));
                out.push_str(", \"\")}");
            }
        }
    }
    out.push('`');
    out
}

#[derive(Debug, PartialEq)]
enum Part<'a> {
    Text(&'a str),
    Var(&'a str),
}

/// Splits `"Hi {{ user }}!"` into text and variable parts. `{{` without a valid closing `}}` (or with an
/// empty / multi-line / brace-containing name) is kept as literal text.
fn split_placeholders(text: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let mut literal_start = 0;
    let mut i = 0;
    while let Some(open) = text[i..].find("{{").map(|o| i + o) {
        let name_start = open + 2;
        let placeholder =
            text[name_start..].find("}}").map(|c| (name_start + c, text[name_start..name_start + c].trim()));
        match placeholder {
            Some((close, name)) if !name.is_empty() && !name.contains(['{', '}', '\n', '\r']) => {
                if open > literal_start {
                    parts.push(Part::Text(&text[literal_start..open]));
                }
                parts.push(Part::Var(name));
                i = close + 2;
                literal_start = i;
            }
            _ => i = open + 1,
        }
    }
    if literal_start < text.len() {
        parts.push(Part::Text(&text[literal_start..]));
    }
    parts
}

fn push_template_text(out: &mut String, text: &str) {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '`' => out.push_str("\\`"),
            '$' if chars.peek() == Some(&'{') => out.push_str("\\$"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}

/// `{ key: value, … }` from the entries that differ from their defaults; `None` when all are default.
pub(super) fn options_object(entries: &[(&str, Option<String>)]) -> Option<String> {
    let set: Vec<String> = entries.iter().filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}: {v}"))).collect();
    (!set.is_empty()).then(|| format!("{{ {} }}", set.join(", ")))
}

/// `name(a, b);`
pub(super) fn call_stmt(callee: &str, args: impl IntoIterator<Item = String>) -> String {
    let args: Vec<String> = args.into_iter().collect();
    format!("{callee}({});", args.join(", "))
}

/// Whether indenting the lines of `code` keeps its meaning. Not when it may hold a multi-line template
/// literal or a line continuation: their text would absorb the added spaces.
fn reindent_safe(code: &str) -> bool {
    !code.contains('`') && !code.lines().any(|l| l.ends_with('\\'))
}

/// User code without surrounding blank lines and, when [`reindent_safe`], without its common indentation
/// and trailing spaces.
fn dedent(code: &str) -> String {
    let code = trim_blank_lines(code);
    if !reindent_safe(code) {
        return code.trim_start().to_string();
    }
    let lines: Vec<&str> = code.lines().map(str::trim_end).collect();
    let common = common_indent(&lines);
    lines.iter().map(|l| l.get(common..).unwrap_or("")).collect::<Vec<_>>().join("\n")
}

/// A user expression that spans lines or ends in a comment, on lines of its own (indented one level when
/// safe) so the code around it — like a closing parenthesis — can't end up inside a comment.
pub(super) fn own_lines(code: &str) -> String {
    let code = dedent(code);
    let body = if reindent_safe(&code) {
        code.lines().map(|l| if l.is_empty() { String::new() } else { format!("  {l}") }).collect::<Vec<_>>().join("\n")
    } else {
        code
    };
    format!("\n{body}\n")
}

fn trim_blank_lines(code: &str) -> &str {
    let start =
        code.find(|c: char| !c.is_whitespace()).map_or(code.len(), |i| code[..i].rfind('\n').map_or(0, |n| n + 1));
    code[start..].trim_end()
}

/// Length (in bytes) of the leading whitespace shared by every non-blank line.
fn common_indent(lines: &[&str]) -> usize {
    let mut common: Option<&str> = None;
    for l in lines.iter().filter(|l| !l.trim().is_empty()) {
        let indent = &l[..l.len() - l.trim_start().len()];
        common = Some(match common {
            None => indent,
            Some(c) => {
                let n: usize =
                    c.chars().zip(indent.chars()).take_while(|(a, b)| a == b).map(|(a, _)| a.len_utf8()).sum();
                &c[..n]
            }
        });
    }
    common.map_or(0, str::len)
}

/// Line-based code builder with two-space indentation.
#[derive(Default)]
pub(super) struct Writer {
    out: String,
    depth: usize,
}

impl Writer {
    /// Writes `s` at the current depth. Embedded newlines (multi-line user code) get the indentation too
    /// when that is safe; otherwise continuation lines are copied as they are.
    pub(super) fn line(&mut self, s: &str) {
        let safe = reindent_safe(s);
        for (i, l) in s.split('\n').enumerate() {
            if i > 0 {
                self.out.push('\n');
            }
            if !l.is_empty() && (i == 0 || safe) {
                for _ in 0..self.depth {
                    self.out.push_str("  ");
                }
            }
            self.out.push_str(l);
        }
        self.out.push('\n');
    }

    /// A line that opens a block (`if (…) {`); following lines are indented.
    pub(super) fn open(&mut self, s: &str) {
        self.line(s);
        self.depth += 1;
    }

    /// A line that closes a block.
    pub(super) fn close(&mut self, s: &str) {
        self.depth = self.depth.saturating_sub(1);
        self.line(s);
    }

    /// A line that closes one block and opens the next (`} else {`).
    pub(super) fn reopen(&mut self, s: &str) {
        self.close(s);
        self.depth += 1;
    }

    pub(super) fn blank(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with("\n\n") {
            self.out.push('\n');
        }
    }

    /// Hand-written statements as a `{ … }` block, so their declarations stay local to it.
    pub(super) fn code_block(&mut self, code: &str) {
        self.open("{");
        self.line(&dedent(code));
        self.close("}");
    }

    pub(super) fn finish(self) -> String {
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders() {
        assert_eq!(split_placeholders("Hi {{ user }}!"), vec![Part::Text("Hi "), Part::Var("user"), Part::Text("!")]);
        assert_eq!(split_placeholders("{{}} {{a"), vec![Part::Text("{{}} {{a")]);
        assert_eq!(split_placeholders("{{{x}}}"), vec![Part::Text("{"), Part::Var("x"), Part::Text("}")]);
        assert_eq!(split_placeholders("{{a\nb}}"), vec![Part::Text("{{a\nb}}")]);
    }

    #[test]
    fn interpolation_escapes_template_syntax() {
        assert_eq!(interpolated("plain `x` ${y}"), r#""plain `x` ${y}""#);
        assert_eq!(
            interpolated("`{{name}}` costs ${{price}} \\ $5\n"),
            r#"`\`${vars.get("name", "")}\` costs $${vars.get("price", "")} \\ $5\n`"#
        );
        assert_eq!(interpolated("${a} {{b}}"), r#"`\${a} ${vars.get("b", "")}`"#);
        assert_eq!(interpolated("{{a}}{{b}}"), r#"`${vars.get("a", "")}${vars.get("b", "")}`"#);
        assert_eq!(interpolated("{{ my \"var\" }}"), r#"`${vars.get("my \"var\"", "")}`"#);
        assert_eq!(interpolated("{{x}}\u{7}\r"), r#"`${vars.get("x", "")}\u0007\r`"#);
    }

    #[test]
    fn code_block_reindents_only_when_safe() {
        let mut w = Writer::default();
        w.open("if (x) {");
        w.code_block("\n    const a = 1;\n      log(a);\n\n    log(2);   \n");
        w.code_block("  type(`a\n  b`);");
        w.line(&format!("if ({}) {{}}", own_lines("a ||\n  b // why")));
        w.close("}");
        assert_eq!(
            w.finish(),
            "if (x) {\n  {\n    const a = 1;\n      log(a);\n\n    log(2);\n  }\n  {\n    type(`a\n  b`);\n  }\n  if (\n    a ||\n      b // why\n  ) {}\n}\n"
        );
    }
}
