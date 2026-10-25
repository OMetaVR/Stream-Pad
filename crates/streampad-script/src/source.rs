//! How macro code is embedded in what QuickJS evaluates, and how QuickJS positions map back to it.
//!
//! The macro body becomes the body of a function so that a top-level `return` ends the press handler
//! (compiled `flow.stop`, early exits in scripts). Declarations therefore live in that function's scope,
//! so the prefix hands the runtime a closure that later looks `onRelease` up from inside it. The prefix
//! shares line 1 with the user's first line, which keeps every line number intact; only columns on line 1
//! shift, by `PREFIX.len()`.

/// File names QuickJS reports in stack frames.
pub(crate) const MACRO_FILE: &str = "macro";
pub(crate) const LIBRARY_FILE: &str = "library";
pub(crate) const PRELUDE_FILE: &str = "prelude";

// An arrow function rather than `(function … })`: the suffix is then a lone `}`, so an unclosed block in
// the user's code is reported as an unexpected end of their code, not as a parenthesis they never wrote.
pub(crate) const PREFIX: &str =
    "(__streampadExport) => { __streampadExport(() => typeof onRelease === \"function\" ? onRelease : undefined); ";
const SUFFIX: &str = "\n}";

/// The expression QuickJS evaluates for `code`: a function taking the export callback the prelude's
/// `runPress` passes in.
pub(crate) fn wrap(code: &str) -> String {
    let mut wrapped = String::with_capacity(PREFIX.len() + code.len() + SUFFIX.len());
    wrapped.push_str(PREFIX);
    wrapped.push_str(code);
    wrapped.push_str(SUFFIX);
    wrapped
}

/// Maps a 1-based (line, column) in the wrapped macro back into `code`. Positions in the closing suffix
/// (an unclosed block noticed at the very end) are reported just past the user's last character.
pub(crate) fn user_position(code: &str, line: u32, col: u32) -> (u32, u32) {
    let line_count = code.split('\n').count() as u32;
    if line > line_count {
        let last = code.rsplit('\n').next().unwrap_or("");
        return (line_count, last.chars().count() as u32 + 1);
    }
    let col = if line == 1 { col.saturating_sub(PREFIX.len() as u32).max(1) } else { col };
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_map_back() {
        let code = "press('a');\nfoo(";
        assert_eq!(user_position(code, 1, PREFIX.len() as u32 + 1), (1, 1));
        assert_eq!(user_position(code, 2, 4), (2, 4));
        assert_eq!(user_position(code, 3, 2), (2, 5));
        assert_eq!(user_position("", 2, 1), (1, 1));
    }

    #[test]
    fn wrapper_keeps_line_numbers() {
        let code = "a\nb\nc";
        assert_eq!(wrap(code).lines().count(), 4);
        assert!(wrap(code).lines().nth(1) == Some("b"));
    }
}
