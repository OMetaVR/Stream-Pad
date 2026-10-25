//! Error reporting: positions in the user's code, stacks, library errors and `check_syntax`.

mod common;

use common::{run_script, script, setup, start};
use std::time::{Duration, Instant};
use streampad_script::RunOutcome;

#[test]
fn exceptions_report_the_users_line_and_column() {
    let cases: &[(&str, &str)] = &[
        ("foo();", "ReferenceError: foo is not defined (line 1:1)"),
        ("press('a');\n\nnull.x;", "TypeError: cannot read property 'x' of null (line 3:1)"),
        ("throw new Error('boom');", "boom (line 1:11)"),
        (
            "function inner() {\n  throw new RangeError('deep');\n}\nfunction outer() { inner(); }\nouter();",
            "RangeError: deep (line 2:13)\n    at inner (line 2:13)\n    at outer (line 4:20)\n    at <anonymous> (line 5:1)",
        ),
        ("throw 'plain text';", "plain text"),
        ("throw { code: 3 };", r#"{"code":3}"#),
        ("throw undefined;", "undefined"),
        ("let x = ;", "SyntaxError: unexpected token in expression: ';' (line 1:9)"),
        (
            "if (x) {\n  press('a');",
            "SyntaxError: unexpected end of code (is a closing }, ) or ] missing?) (line 2:14)",
        ),
        ("x = 1;", "ReferenceError: x is not defined (line 1:1)"),
    ];
    for (code, expected) in cases {
        let (outcome, _) = run_script(code);
        assert_eq!(outcome, RunOutcome::Failed(expected.to_string()), "{code}");
    }
}

#[test]
fn errors_in_on_release_are_reported() {
    let (_, engine) = setup();
    let run = start(&engine, 1, script("press('a');\nfunction onRelease() {\n  missing();\n}"));
    run.handle.release();
    assert_eq!(run.outcome(), RunOutcome::Failed("ReferenceError: missing is not defined (line 3:3)".into()));
}

#[test]
fn library_errors_point_at_the_library() {
    let cases: &[(&str, &str, &str)] = &[
        (
            "function helper() {\n  return notThere();\n}",
            "\nhelper();",
            "ReferenceError: notThere is not defined (line 2:1)\n    at helper (library line 2:10)\n    at <anonymous> (line 2:1)",
        ),
        ("throw new Error('lib broke');", "log(1)", "lib broke (library line 1:11)"),
        ("function (", "log(1)", "SyntaxError: function name expected (library line 1:10)"),
    ];
    for (library, code, expected) in cases {
        let (_, engine) = setup();
        engine.set_library(library.to_string());
        assert_eq!(start(&engine, 1, script(code)).outcome(), RunOutcome::Failed(expected.to_string()), "{library}");
    }
}

#[test]
fn check_syntax_accepts_valid_macros() {
    let (host, engine) = setup();
    for code in [
        "",
        "press('a')",
        "return;",
        "if (event.repeat) return;\npress('a');",
        "function onRelease(e) {}",
        "while (true) {}",
    ] {
        assert_eq!(engine.check_syntax(code), Ok(()), "{code:?}");
    }
    assert!(host.calls().is_empty(), "check_syntax must not run code");
}

#[test]
fn check_syntax_reports_line_and_column() {
    let (_, engine) = setup();
    let cases: &[(&str, &str)] = &[
        ("let a = ;", "SyntaxError: unexpected token in expression: ';' (line 1:9)"),
        ("press('a');\nlet = 5;", "SyntaxError: variable name expected (line 2:5)"),
        ("if (x) {\n  press('a')", "SyntaxError: unexpected end of code (is a closing }, ) or ] missing?) (line 2:13)"),
        ("press('a'));", "SyntaxError: expecting ';' (line 1:11)"),
        ("type(`unterminated", "SyntaxError: unexpected end of string (line 1:6)"),
        ("with (x) {}", "SyntaxError: invalid keyword: with (line 1:1)"),
    ];
    for (code, expected) in cases {
        assert_eq!(engine.check_syntax(code), Err(expected.to_string()), "{code}");
    }
}

#[test]
fn check_syntax_is_bounded_even_for_code_that_escapes_the_wrapper() {
    let (host, engine) = setup();
    let started = Instant::now();
    let result = engine.check_syntax("}); while (true) {} (function () {");
    assert!(started.elapsed() < Duration::from_secs(2), "took {:?}", started.elapsed());
    assert!(result.is_err(), "{result:?}");
    assert!(engine.check_syntax("}); press('a'); (function () {").is_err());
    assert!(host.calls().is_empty());
}
