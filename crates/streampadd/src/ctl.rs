//! Client-side subcommands: `ctl <method> [json]`, `ctl watch` and `list-devices`.

use crate::backend::{InputOverrides, engine_config};
use anyhow::{Context, bail};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::time::Duration;
use streampad_core::ipc::{Incoming, Request};
use streampad_core::paths::Paths;
use streampad_core::combo;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub fn ctl(method: &str, params: Option<&str>) -> anyhow::Result<ExitCode> {
    let params: Value = match params {
        Some(p) => serde_json::from_str(p).with_context(|| format!("params are not valid JSON: {p}"))?,
        None => Value::Null,
    };
    let paths = Paths::resolve();
    let stream = UnixStream::connect(&paths.socket)
        .with_context(|| format!("cannot connect to {} (is streampadd running?)", paths.socket.display()))?;
    if method == "watch" {
        return watch(stream, params);
    }
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    send(&stream, &Request { id: 1, method: method.into(), params })?;
    for line in BufReader::new(&stream).lines() {
        let line = line.context("reading the daemon's answer")?;
        if let Ok(Incoming::Response(r)) = serde_json::from_str::<Incoming>(&line)
            && r.id == 1
        {
            if let Some(e) = r.error {
                let node = e.node_id.map(|n| format!(" (node {n})")).unwrap_or_default();
                eprintln!("error: {}{node}", e.message);
                return Ok(ExitCode::FAILURE);
            }
            println!("{}", serde_json::to_string_pretty(&r.result.unwrap_or(Value::Null))?);
            return Ok(ExitCode::SUCCESS);
        }
    }
    bail!("the daemon closed the connection without answering")
}

/// Subscribe and print every event as one JSON line until the daemon goes away.
fn watch(stream: UnixStream, extra: Value) -> anyhow::Result<ExitCode> {
    let mut hello = json!({ "client": "cli", "subscribe": true });
    if let (Value::Object(h), Value::Object(e)) = (&mut hello, extra) {
        h.extend(e);
    }
    send(&stream, &Request { id: 1, method: "hello".into(), params: hello })?;
    let stdout = std::io::stdout();
    for line in BufReader::new(&stream).lines() {
        let line = line.context("reading events")?;
        match serde_json::from_str::<Incoming>(&line) {
            Ok(Incoming::Event(_)) => {
                let mut out = stdout.lock();
                writeln!(out, "{line}")?;
                out.flush()?;
            }
            Ok(Incoming::Response(r)) if r.error.is_some() => {
                eprintln!("error: {}", r.error.map(|e| e.message).unwrap_or_default());
                return Ok(ExitCode::FAILURE);
            }
            _ => {}
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn send(mut stream: &UnixStream, req: &Request) -> anyhow::Result<()> {
    let mut line = serde_json::to_vec(req)?;
    line.push(b'\n');
    stream.write_all(&line).context("sending the request")
}

/// What `list-devices` reports uses the device settings from config.json, without grabbing anything.
pub fn list_devices(as_json: bool) -> anyhow::Result<ExitCode> {
    let paths = Paths::resolve();
    let cfg = crate::config_store::load(&paths).unwrap_or_else(|e| {
        eprintln!("warning: {e}; using default device settings");
        None
    });
    let cfg = cfg.unwrap_or_default();
    let engine = engine_config(&cfg.settings, &InputOverrides::default(), false);
    let toggle = combo::key_code(&cfg.settings.toggle_key).unwrap_or(streampad_core::keys::KEY_SCROLLLOCK);
    let devices = streampad_input::list_devices(&engine.devices, toggle);
    if as_json {
        println!("{}", serde_json::to_string_pretty(&devices)?);
        return Ok(ExitCode::SUCCESS);
    }
    if devices.is_empty() {
        println!("no input devices found (are you in the `input` group?)");
        return Ok(ExitCode::SUCCESS);
    }
    let path_width = devices.iter().map(|d| d.path.len()).max().unwrap_or(4).max(4);
    println!("{:<path_width$}  {:<8}  {:<6}  ID", "PATH", "ELIGIBLE", "NUMPAD");
    for d in &devices {
        let yes = |b: bool| if b { "yes" } else { "no" };
        let note = d.error.as_deref().map(|e| format!("  ({e})")).unwrap_or_default();
        println!("{:<path_width$}  {:<8}  {:<6}  {}{note}", d.path, yes(d.eligible), yes(d.has_numpad), d.id);
    }
    Ok(ExitCode::SUCCESS)
}
