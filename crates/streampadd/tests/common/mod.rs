//! Test harness: a sandboxed `streampadd --no-input --no-overlay` and a small JSON-lines client.
//!
//! The daemon gets private config/data/state/runtime dirs and an unreachable session bus, so tests never touch
//! the user's StreamPad files, desktop or KWin.

#![allow(dead_code)]

pub mod uinput;

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};
use streampad_core::Config;

pub const WAIT: Duration = Duration::from_secs(5);

pub struct Daemon {
    pub dir: tempfile::TempDir,
    args: Vec<String>,
    env: Vec<(String, String)>,
    child: Option<Child>,
}

impl Daemon {
    /// `--no-input --no-overlay` with `config` written as config.json first (or the first-run defaults when
    /// `None`).
    pub fn start(config: Option<&Config>) -> Daemon {
        Daemon::start_with(config, &["--no-input", "--no-overlay"])
    }

    pub fn start_with(config: Option<&Config>, args: &[&str]) -> Daemon {
        Daemon::start_with_env(config, args, &[])
    }

    /// Extra environment (applied last), e.g. a `PATH` with fake helper programs or a private session bus.
    pub fn start_with_env(config: Option<&Config>, args: &[&str], env: &[(&str, &str)]) -> Daemon {
        let dir = tempfile::tempdir().expect("temp dir");
        let args = args.iter().map(|a| a.to_string()).collect();
        let env = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        let mut d = Daemon { dir, args, env, child: None };
        if let Some(cfg) = config {
            cfg.save_atomic(&d.config_file()).expect("write config");
        }
        d.spawn();
        d
    }

    /// Start again on the same directories (after `wait_exit` or `kill`).
    pub fn restart(&mut self) {
        assert!(self.child.is_none(), "still running");
        self.spawn();
    }

    pub fn pid(&self) -> u32 {
        self.child.as_ref().expect("running").id()
    }

    /// SIGKILL: no chance to clean up.
    pub fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().expect("kill");
            child.wait().expect("wait");
        }
    }

    fn spawn(&mut self) {
        let root = self.dir.path();
        let child = Command::new(env!("CARGO_BIN_EXE_streampadd"))
            .args(&self.args)
            .env("STREAMPAD_CONFIG_DIR", root.join("config"))
            .env("STREAMPAD_DATA_DIR", root.join("data"))
            .env("STREAMPAD_STATE_DIR", root.join("state"))
            .env("STREAMPAD_RUNTIME_DIR", root.join("run"))
            .env("STREAMPAD_SOCKET", self.socket())
            .env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}", root.join("no-bus").display()))
            .env("RUST_LOG", "warn")
            .envs(self.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .stdin(Stdio::null())
            .spawn()
            .expect("spawn streampadd");
        self.child = Some(child);
        let deadline = Instant::now() + WAIT;
        while UnixStream::connect(self.socket()).is_err() {
            assert!(Instant::now() < deadline, "daemon did not open its socket");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn socket(&self) -> PathBuf {
        self.dir.path().join("run/daemon.sock")
    }

    pub fn config_file(&self) -> PathBuf {
        self.dir.path().join("config/config.json")
    }

    pub fn state_file(&self) -> PathBuf {
        self.dir.path().join("state/state.json")
    }

    pub fn client(&self) -> Client {
        Client::connect(&self.socket())
    }

    /// Wait for the daemon to exit on its own.
    pub fn wait_exit(&mut self) -> ExitStatus {
        let child = self.child.as_mut().expect("running");
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(status) = child.try_wait().expect("wait") {
                self.child = None;
                return status;
            }
            assert!(Instant::now() < deadline, "daemon did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub struct Client {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: u64,
    /// Events received while waiting for responses.
    pub events: Vec<(String, Value)>,
}

impl Client {
    pub fn connect(socket: &Path) -> Client {
        let stream = UnixStream::connect(socket).expect("connect");
        stream.set_read_timeout(Some(Duration::from_millis(100))).expect("timeout");
        let reader = BufReader::new(stream.try_clone().expect("clone"));
        Client { stream, reader, next_id: 0, events: Vec::new() }
    }

    /// The raw socket, for tests that read it directly.
    pub fn into_stream(self) -> UnixStream {
        self.stream
    }

    pub fn send(&mut self, method: &str, params: Value) -> u64 {
        self.next_id += 1;
        let mut line = serde_json::to_vec(&json!({ "id": self.next_id, "method": method, "params": params })).unwrap();
        line.push(b'\n');
        self.stream.write_all(&line).expect("send");
        self.next_id
    }

    /// Request → `Ok(result)` or `Err(error message)`.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.send(method, params);
        let deadline = Instant::now() + WAIT;
        loop {
            assert!(Instant::now() < deadline, "no answer to {method}");
            let Some(msg) = self.read() else { continue };
            if msg.get("id").and_then(Value::as_u64) == Some(id) {
                return match msg.get("error") {
                    Some(e) => Err(e["message"].as_str().unwrap_or_default().to_string()),
                    None => Ok(msg["result"].clone()),
                };
            }
        }
    }

    pub fn ok(&mut self, method: &str, params: Value) -> Value {
        self.call(method, params).unwrap_or_else(|e| panic!("{method} failed: {e}"))
    }

    fn read(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => panic!("daemon closed the connection"),
            Ok(_) => {}
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => return None,
            Err(e) => panic!("read: {e}"),
        }
        let msg: Value = serde_json::from_str(&line).expect("daemon sent invalid JSON");
        if let Some(event) = msg.get("event").and_then(Value::as_str) {
            self.events.push((event.to_string(), msg["data"].clone()));
        }
        Some(msg)
    }

    /// Wait until an event `name` whose data satisfies `pred` arrives (also checks already-received ones,
    /// consuming everything up to and including the match).
    pub fn wait_event(&mut self, name: &str, pred: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(pos) = self.events.iter().position(|(n, d)| n == name && pred(d)) {
                let found = self.events[pos].1.clone();
                self.events.drain(..=pos);
                return found;
            }
            assert!(Instant::now() < deadline, "no {name} event matched; got {:?}", self.events);
            self.read();
        }
    }

    /// Collect events for `period`.
    pub fn drain(&mut self, period: Duration) -> Vec<(String, Value)> {
        let deadline = Instant::now() + period;
        while Instant::now() < deadline {
            self.read();
        }
        std::mem::take(&mut self.events)
    }
}

pub fn wait_for(what: &str, cond: impl Fn() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
