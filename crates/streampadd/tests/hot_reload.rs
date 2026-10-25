//! config.json hot reload: atomic and in-place writes are picked up; invalid files report `configError` and
//! keep the previous config.

mod common;

use common::Daemon;
use serde_json::{Value, json};
use streampad_core::{Config, Profile};

fn config_named(names: &[&str]) -> Config {
    Config { profiles: names.iter().map(|n| Profile::new(n)).collect(), ..Config::default() }
}

fn names(state: &Value) -> Vec<String> {
    state["profiles"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn edits_are_applied_and_broken_files_are_rejected() {
    let daemon = Daemon::start(Some(&config_named(&["One"])));
    let mut c = daemon.client();
    c.ok("hello", json!({ "client": "editor", "subscribe": true }));

    // Atomic replace (what the editor does).
    config_named(&["One", "Two"]).save_atomic(&daemon.config_file()).unwrap();
    let state = c.wait_event("state", |s| s["profiles"].as_array().is_some_and(|p| p.len() == 2));
    assert_eq!(names(&state), ["One", "Two"]);

    // Broken file: error event, previous config kept.
    std::fs::write(daemon.config_file(), "{ \"profiles\": [ oops").unwrap();
    let err = c.wait_event("configError", |_| true);
    assert!(err["message"].as_str().unwrap().contains("invalid config.json"));
    let state = c.ok("getState", Value::Null);
    assert_eq!(names(&state), ["One", "Two"]);
    assert!(state["configError"].is_string());
    assert!(c.call("reload", Value::Null).is_err(), "reload reports the same error");

    // In-place rewrite with a fix clears the error.
    let fixed = serde_json::to_string_pretty(&config_named(&["Three"])).unwrap();
    std::fs::write(daemon.config_file(), fixed).unwrap();
    let state = c.wait_event("state", |s| s["configError"].is_null() && s["profiles"][0]["name"] == "Three");
    assert_eq!(names(&state), ["Three"]);
}

#[test]
fn reload_keeps_the_current_profile_and_unknown_toggle_keys_are_errors() {
    let cfg = config_named(&["A", "B"]);
    let b_id = cfg.profiles[1].id.clone();
    let daemon = Daemon::start(Some(&cfg));
    let mut c = daemon.client();
    c.ok("hello", json!({ "client": "editor", "subscribe": true }));
    c.ok("setProfile", json!({ "id": b_id }));

    let mut reordered = cfg.clone();
    reordered.profiles.reverse();
    reordered.save_atomic(&daemon.config_file()).unwrap();
    c.wait_event("state", |s| s["profiles"][0]["name"] == "B" && s["profileIndex"] == 0);

    let mut bad = reordered.clone();
    bad.settings.toggle_key = "NotAKey".into();
    bad.save_atomic(&daemon.config_file()).unwrap();
    let err = c.wait_event("configError", |_| true);
    assert!(err["message"].as_str().unwrap().contains("NotAKey"));
}
