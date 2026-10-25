//! Run lifecycle: cancellation, key release / `onRelease`, resource limits, concurrency, library.

mod common;

use common::{TIMEOUT, run_script, script, setup, start};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};
use streampad_core::compile::CompiledMacro;
use streampad_script::{Invocation, RunOutcome};

fn presses(host: &common::MockHost) -> Vec<String> {
    host.actions()
        .into_iter()
        .filter(|(m, _)| m == "key.press")
        .map(|(_, a)| a["combo"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn cancel_interrupts_sleep_within_20ms() {
    let (host, engine) = setup();
    let mut latencies = Vec::new();
    for run_id in 0..5 {
        let run = start(&engine, run_id, script("log('sleeping'); sleep(60000);"));
        assert!(host.wait_for(|c| c.run_id == run_id && c.method == "log"));
        thread::sleep(Duration::from_millis(10));
        let cancelled_at = Instant::now();
        run.handle.cancel();
        assert_eq!(run.outcome(), RunOutcome::Cancelled);
        latencies.push(cancelled_at.elapsed());
        assert!(run.handle.is_finished());
    }
    latencies.sort();
    // Median, so one scheduling hiccup on a busy machine doesn't fail the test.
    assert!(latencies[2] < Duration::from_millis(20), "cancel latencies: {latencies:?}");
}

#[test]
fn cancel_stops_loops_that_never_yield() {
    let loops = [
        "log('go'); while (true) {}",
        "log('go'); for (;;) { try { sleep(1); } catch (e) {} }",
        "log('go'); for (;;) { try { press('a'); } catch {} }",
        "log('go'); const spin = () => { for (;;) {} }; try { spin(); } finally { press('finally'); }",
    ];
    for (run_id, code) in loops.into_iter().enumerate() {
        let (host, engine) = setup();
        let run = start(&engine, run_id as u64, script(code));
        assert!(host.wait_for_log("go"), "{code}");
        thread::sleep(Duration::from_millis(30));
        let cancelled_at = Instant::now();
        run.handle.cancel();
        assert_eq!(run.outcome(), RunOutcome::Cancelled, "{code}");
        assert!(cancelled_at.elapsed() < Duration::from_millis(500), "{code}: took {:?}", cancelled_at.elapsed());
        let after = host.calls().len();
        thread::sleep(Duration::from_millis(20));
        assert_eq!(host.calls().len(), after, "{code}: host calls after the run ended");
        assert!(!presses(&host).contains(&"finally".to_string()), "{code}: ran code after cancel");
    }
}

#[test]
fn on_release_runs_when_the_key_is_released() {
    let (host, engine) = setup();
    let run = start(
        &engine,
        1,
        script("press('down');\nfunction onRelease(e) {\n  press(e.type);\n  log(event.type, event.key);\n}"),
    );
    assert!(host.wait_for(|c| c.args["combo"] == "down"));
    thread::sleep(Duration::from_millis(50));
    assert!(!run.handle.is_finished(), "must wait for the release");
    assert!(run.done.try_recv().is_err());
    run.handle.release();
    assert_eq!(run.outcome(), RunOutcome::Finished);
    assert!(run.handle.is_finished());
    assert_eq!(presses(&host), ["down", "release"]);
    assert_eq!(host.logs(), ["release KP4"]);
}

#[test]
fn release_during_the_press_handler_runs_on_release_afterwards() {
    let (host, engine) = setup();
    let run = start(&engine, 1, script("sleep(80);\npress('a');\nconst onRelease = () => press('b');"));
    run.handle.release();
    assert_eq!(run.outcome(), RunOutcome::Finished);
    assert_eq!(presses(&host), ["a", "b"]);
}

#[test]
fn cancel_while_waiting_for_release_skips_on_release() {
    let (host, engine) = setup();
    let run = start(&engine, 1, script("press('a');\nfunction onRelease() { press('b'); }"));
    assert!(host.wait_for(|c| c.args["combo"] == "a"));
    thread::sleep(Duration::from_millis(20));
    run.handle.cancel();
    assert_eq!(run.outcome(), RunOutcome::Cancelled);
    run.handle.release();
    assert_eq!(presses(&host), ["a"]);
}

#[test]
fn runs_without_on_release_finish_on_their_own() {
    for code in [
        "press('a')",
        "press('a'); return; let onRelease = () => press('b');",
        "press('a'); onRelease = 1;\n var onRelease;",
    ] {
        let (host, engine) = setup();
        let run = start(&engine, 1, script(code));
        assert_eq!(run.outcome(), RunOutcome::Finished, "{code}");
        run.handle.release();
        assert_eq!(presses(&host), ["a"], "{code}");
    }
}

#[test]
fn failed_press_handler_does_not_wait_for_release() {
    let (host, engine) = setup();
    let run = start(&engine, 1, script("throw new Error('nope');\nfunction onRelease() { press('b'); }"));
    assert_eq!(run.outcome(), RunOutcome::Failed("nope (line 1:11)".into()));
    assert!(presses(&host).is_empty());
}

#[test]
fn top_level_return_ends_the_press_handler() {
    let (outcome, host) = run_script("press('a');\nif (event.key === 'KP4') return;\npress('b');");
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(presses(&host), ["a"]);
}

#[test]
fn event_describes_the_invocation() {
    let (host, engine) = setup();
    let (tx, done) = channel();
    let inv = Invocation { run_id: 9, key: None, repeat: true, profile_id: "p_web".into(), profile_name: "Web".into() };
    engine.spawn(
        script("log(JSON.stringify(event))"),
        inv,
        Box::new(move |o| {
            let _ = tx.send(o);
        }),
    );
    assert_eq!(done.recv_timeout(TIMEOUT).unwrap(), RunOutcome::Finished);
    let logged: Value = serde_json::from_str(&host.logs()[0]).unwrap();
    assert_eq!(
        logged,
        json!({ "key": null, "type": "press", "repeat": true, "profile": { "id": "p_web", "name": "Web" } })
    );
    assert!(host.calls().iter().all(|c| c.run_id == 9));
}

#[test]
fn memory_limit_fails_the_run_without_crashing() {
    let (outcome, _) = run_script("const hog = [];\nfor (;;) hog.push(new Array(100000).fill(1.5));");
    let RunOutcome::Failed(message) = outcome else { panic!("expected failure, got {outcome:?}") };
    assert!(message.starts_with("out of memory (a macro can use up to 64 MB) (line 2:"), "{message}");

    let (outcome, _) = run_script("let s = 'x';\nfor (;;) s += s;");
    assert_eq!(outcome, RunOutcome::Failed("InternalError: string too long (line 2:15)".into()));

    let (outcome, _) = run_script("log('still fine')");
    assert_eq!(outcome, RunOutcome::Finished);
}

#[test]
fn deep_recursion_fails_with_a_range_error() {
    let (outcome, _) = run_script("function down(n) { return down(n + 1) + 1; }\ndown(0);");
    let RunOutcome::Failed(message) = outcome else { panic!("expected failure, got {outcome:?}") };
    assert!(message.starts_with("RangeError: Maximum call stack size exceeded (line 1:"), "{message}");
}

#[test]
fn a_hundred_runs_in_parallel() {
    let (host, engine) = setup();
    let started = Instant::now();
    let runs: Vec<_> =
        (0..100).map(|run_id| start(&engine, run_id, script("sleep(200);\npress(`k${event.key}`);"))).collect();
    for run in &runs {
        assert_eq!(run.outcome(), RunOutcome::Finished);
    }
    // Sequentially this would take 20 s.
    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
    let calls = host.calls();
    assert_eq!(calls.len(), 100);
    assert_eq!(calls.iter().map(|c| c.run_id).collect::<HashSet<_>>().len(), 100);
    assert!(calls.iter().all(|c| c.args == json!({ "combo": "kKP4" })));
}

#[test]
fn a_panicking_host_fails_the_run() {
    let (outcome, _) = run_script("press('panic')");
    assert_eq!(outcome, RunOutcome::Failed("internal error: host exploded".into()));
    let (outcome, _) = run_script("press('a')");
    assert_eq!(outcome, RunOutcome::Finished);
}

#[test]
fn library_functions_are_callable_and_updates_apply_to_new_runs() {
    let (host, engine) = setup();
    engine.set_library("function greet(name) {\n  return `hi ${name}`;\n}\nconst ANSWER = 42;".into());
    assert_eq!(start(&engine, 1, script("log(greet('bob'), ANSWER)")).outcome(), RunOutcome::Finished);
    engine.set_library("function greet(name) { return `yo ${name}`; }".into());
    assert_eq!(start(&engine, 2, script("log(greet('bob'), typeof ANSWER)")).outcome(), RunOutcome::Finished);
    assert_eq!(host.logs(), ["hi bob 42", "yo bob undefined"]);
}

#[test]
fn wait_reports_completion() {
    let (_, engine) = setup();
    let run = start(&engine, 1, script("sleep(100)"));
    assert!(!run.handle.wait(Duration::from_millis(10)));
    assert!(run.handle.wait(TIMEOUT));
    assert_eq!(run.handle.run_id(), 1);
    assert_eq!(run.outcome(), RunOutcome::Finished);
}

#[test]
fn compiled_flags_do_not_change_runtime_behaviour() {
    // `has_release` is a hint for the daemon; the runtime looks for onRelease itself.
    let (host, engine) = setup();
    let code = Arc::new(CompiledMacro { code: "function onRelease() { press('b'); }".into(), has_release: false });
    let run = start(&engine, 1, code);
    run.handle.release();
    assert_eq!(run.outcome(), RunOutcome::Finished);
    assert_eq!(presses(&host), ["b"]);
}

#[test]
fn promise_jobs_run_after_each_handler() {
    let (outcome, host) = run_script(
        "Promise.resolve('then').then((c) => press(c));\n\
         async function steps() { press('a'); await null; press('b'); }\n\
         steps();\n\
         Promise.reject(new Error('x')).catch(() => press('caught'));\n\
         function onRelease() { Promise.resolve().then(() => press('released')); }",
    );
    assert_eq!(outcome, RunOutcome::Finished);
    assert_eq!(presses(&host), ["a", "then", "b", "caught", "released"]);
}

#[test]
fn unhandled_rejections_fail_the_run() {
    let (outcome, host) = run_script(
        "async function later() {\n  await null;\n  throw new Error('async boom');\n}\nlater();\npress('sync');",
    );
    assert_eq!(outcome, RunOutcome::Failed("async boom (line 3:13)".into()));
    assert_eq!(presses(&host), ["sync"]);

    let (outcome, _) = run_script("Promise.reject('plain');");
    assert_eq!(outcome, RunOutcome::Failed("plain".into()));
}
