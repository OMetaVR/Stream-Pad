//! A recording mock `Host` and helpers to start runs and wait for them.

#![allow(dead_code)] // each test binary uses a different subset

use serde_json::{Map, Value, json};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use streampad_core::KeyId;
use streampad_core::compile::CompiledMacro;
use streampad_script::{Host, Invocation, RunContext, RunHandle, RunOutcome, ScriptEngine};

pub const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub run_id: u64,
    pub method: String,
    pub args: Value,
}

#[derive(Default)]
pub struct MockHost {
    calls: Mutex<Vec<Call>>,
    changed: Condvar,
    vars: Mutex<Map<String, Value>>,
}

pub fn firefox() -> Value {
    json!({ "id": "w1", "title": "YouTube — Mozilla Firefox", "appId": "org.mozilla.firefox", "resourceClass": "firefox",
            "desktopFile": "firefox", "pid": 10, "output": "DP-2", "active": true })
}

pub fn konsole() -> Value {
    json!({ "id": "w2", "title": "~ : zsh", "appId": "org.kde.konsole", "resourceClass": "konsole",
            "desktopFile": "org.kde.konsole", "pid": 11, "output": "DP-1", "active": false })
}

impl Host for MockHost {
    fn call(&self, run: RunContext, method: &str, args: Value) -> Result<Value, String> {
        self.calls.lock().unwrap().push(Call { run_id: run.run_id, method: method.into(), args: args.clone() });
        self.changed.notify_all();
        let name = || args["name"].as_str().unwrap_or_default().to_string();
        Ok(match method {
            "key.press" if args["combo"] == "bad" => return Err("unknown key \"bad\"".into()),
            "key.press" if args["combo"] == "panic" => panic!("host exploded"),
            "window.active" => firefox(),
            "window.list" => json!([firefox(), konsole()]),
            "window.focus" => json!(true),
            "run.command" if args["wait"] == true => json!({ "code": 0, "stdout": "out", "stderr": "" }),
            "desktop.current" => json!({ "index": 1, "name": "Desktop 2", "count": 4 }),
            "profile.current" => json!({ "id": "p_audio", "name": "Audio", "index": 0 }),
            "macroMode.get" => json!(true),
            "macroMode.toggle" => json!(false),
            "clipboard.get" => json!("clip"),
            "vars.get" => self.vars.lock().unwrap().get(&name()).cloned().unwrap_or(Value::Null),
            "vars.set" => {
                self.vars.lock().unwrap().insert(name(), args["value"].clone());
                Value::Null
            }
            "vars.toggle" => {
                let mut vars = self.vars.lock().unwrap();
                let on = !vars.get(&name()).is_some_and(truthy);
                vars.insert(name(), json!(on));
                json!(on)
            }
            _ => Value::Null,
        })
    }
}

fn truthy(v: &Value) -> bool {
    !matches!(v, Value::Null | Value::Bool(false)) && v != &json!(0) && v != &json!("")
}

impl MockHost {
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    /// `(method, args)` of every call except logs and traces.
    pub fn actions(&self) -> Vec<(String, Value)> {
        self.calls()
            .into_iter()
            .filter(|c| c.method != "log" && c.method != "trace")
            .map(|c| (c.method, c.args))
            .collect()
    }

    pub fn logs(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter(|c| c.method == "log")
            .map(|c| c.args["message"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    pub fn traces(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter(|c| c.method == "trace")
            .map(|c| c.args["nodeId"].as_str().unwrap_or_default().to_string())
            .collect()
    }

    /// Waits until some recorded call satisfies `pred`.
    pub fn wait_for(&self, pred: impl Fn(&Call) -> bool) -> bool {
        let deadline = Instant::now() + TIMEOUT;
        let mut calls = self.calls.lock().unwrap();
        while !calls.iter().any(&pred) {
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            calls = self.changed.wait_timeout(calls, deadline - now).unwrap().0;
        }
        true
    }

    pub fn wait_for_log(&self, message: &str) -> bool {
        self.wait_for(|c| c.method == "log" && c.args["message"] == message)
    }
}

pub fn invocation(run_id: u64) -> Invocation {
    Invocation {
        run_id,
        key: Some(KeyId::KP4),
        repeat: false,
        profile_id: "p_audio".into(),
        profile_name: "Audio".into(),
    }
}

pub fn script(code: &str) -> Arc<CompiledMacro> {
    Arc::new(CompiledMacro { code: code.into(), has_release: code.contains("onRelease") })
}

pub struct Started {
    pub handle: RunHandle,
    pub done: Receiver<RunOutcome>,
}

impl Started {
    pub fn outcome(&self) -> RunOutcome {
        self.done.recv_timeout(TIMEOUT).expect("run did not finish in time")
    }
}

pub fn start(engine: &ScriptEngine, run_id: u64, code: Arc<CompiledMacro>) -> Started {
    let (tx, done) = channel();
    let handle = engine.spawn(
        code,
        invocation(run_id),
        Box::new(move |outcome| {
            let _ = tx.send(outcome);
        }),
    );
    Started { handle, done }
}

pub fn setup() -> (Arc<MockHost>, ScriptEngine) {
    let host = Arc::new(MockHost::default());
    let engine = ScriptEngine::new(host.clone());
    (host, engine)
}

/// Runs `code` to completion as a fast tap (the release is delivered right away) and returns the outcome
/// with the host that recorded its calls.
pub fn run_script(code: &str) -> (RunOutcome, Arc<MockHost>) {
    let (host, engine) = setup();
    let run = start(&engine, 1, script(code));
    run.handle.release();
    (run.outcome(), host)
}
