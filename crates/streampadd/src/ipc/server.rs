//! Unix-socket server (docs/SPEC.md §4): newline-delimited JSON, any number of clients. Core methods are
//! forwarded to the event loop in order (one in flight per client); slow methods (KWin, compiling, launching
//! the editor) run on their own tasks and answer whenever they finish.

use super::hub::{Hub, Registration};
use crate::backend::MacroRunner;
use crate::command::{Command, CommandTx};
use crate::core::ClientId;
use crate::host::launch::Launcher;
use crate::kwin::{Kwin, UNSUPPORTED};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use streampad_core::Macro;
use streampad_core::compile::{CompileOptions, compile};
use streampad_core::ipc::{ErrorBody, Request, Response};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, oneshot};

/// Longest accepted request line (a `runMacro` with a big script stays far below this).
pub const MAX_LINE: usize = 8 << 20;
const SHUTTING_DOWN: &str = "StreamPad is shutting down";

pub struct ServerCtx {
    pub hub: Arc<Hub>,
    pub core: CommandTx,
    pub kwin: Option<Arc<Kwin>>,
    pub runner: Arc<dyn MacroRunner>,
    pub launcher: Arc<Launcher>,
}

pub async fn serve(listener: UnixListener, ctx: Arc<ServerCtx>) {
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(connection(stream, ctx.clone()));
            }
            Err(e) => {
                // e.g. EMFILE: back off instead of spinning.
                log::warn!("ipc: accept failed: {e}");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn connection(stream: UnixStream, ctx: Arc<ServerCtx>) {
    let Registration { id, outbox, kicked } = ctx.hub.register();
    log::debug!("ipc: client {id} connected");
    let (read, write) = stream.into_split();
    let mut writer = tokio::spawn(write_loop(write, outbox));
    let writer_done = tokio::select! {
        () = read_loop(read, id, &ctx) => false,
        () = kicked.notified() => false,
        _ = &mut writer => true,
    };
    ctx.hub.unregister(id);
    // Unregistering closed the queue: let what is already queued go out, but never wait on a stuck peer.
    if !writer_done && tokio::time::timeout(Duration::from_millis(500), &mut writer).await.is_err() {
        writer.abort();
    }
    let _ = ctx.core.send(Command::ClientGone(id));
    log::debug!("ipc: client {id} disconnected");
}

async fn write_loop(mut write: OwnedWriteHalf, mut outbox: mpsc::Receiver<Arc<str>>) {
    while let Some(line) = outbox.recv().await {
        if write.write_all(line.as_bytes()).await.is_err() {
            return;
        }
    }
}

async fn read_loop(read: OwnedReadHalf, client: ClientId, ctx: &Arc<ServerCtx>) {
    let mut reader = BufReader::new(read);
    let mut line = Vec::new();
    loop {
        line.clear();
        match (&mut reader).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut line).await {
            Ok(0) => return,
            Ok(_) if line.len() > MAX_LINE => {
                ctx.hub.respond(client, &Response::err(0, format!("request longer than {MAX_LINE} bytes")));
                return;
            }
            Ok(_) => {}
            Err(e) => {
                log::debug!("ipc: client {client}: {e}");
                return;
            }
        }
        let text = line.trim_ascii();
        if !text.is_empty() {
            handle(text, client, ctx).await;
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Subscription {
    subscribe: bool,
    trace: bool,
}

async fn handle(text: &[u8], client: ClientId, ctx: &Arc<ServerCtx>) {
    let Request { id, method, params } = match serde_json::from_slice::<Request>(text) {
        Ok(req) => req,
        Err(e) => {
            let id = serde_json::from_slice::<Value>(text).ok().and_then(|v| v.get("id")?.as_u64()).unwrap_or(0);
            ctx.hub.respond(client, &Response::err(id, format!("invalid request: {e}")));
            return;
        }
    };
    match method.as_str() {
        "compileMacro" => {
            let ctx = ctx.clone();
            tokio::spawn(async move {
                let runner = ctx.runner.clone();
                let result = tokio::task::spawn_blocking(move || compile_macro(runner.as_ref(), params))
                    .await
                    .unwrap_or_else(|e| Err(fail(format!("compiler crashed: {e}"))));
                ctx.hub.respond(client, &to_response(id, result));
            });
        }
        "listWindows" => {
            let ctx = ctx.clone();
            tokio::spawn(async move {
                let result = match ctx.kwin.as_ref().filter(|k| k.available()) {
                    Some(kwin) => kwin.windows().await.map(|w| json!(w)).map_err(fail),
                    None => Err(fail(UNSUPPORTED)),
                };
                ctx.hub.respond(client, &to_response(id, result));
            });
        }
        "openEditor" => {
            let ctx = ctx.clone();
            tokio::spawn(async move {
                let launcher = ctx.launcher.clone();
                let result = tokio::task::spawn_blocking(move || launcher.open_editor())
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()))
                    .map(|()| json!({}))
                    .map_err(fail);
                ctx.hub.respond(client, &to_response(id, result));
            });
        }
        _ => {
            if method == "hello" {
                let sub: Subscription = serde_json::from_value(params.clone()).unwrap_or_default();
                ctx.hub.subscribe(client, sub.subscribe, sub.trace);
            }
            let (reply, answer) = oneshot::channel();
            let result = if ctx.core.send(Command::Request { client, method, params, reply }).is_ok() {
                answer.await.unwrap_or_else(|_| Err(fail(SHUTTING_DOWN)))
            } else {
                Err(fail(SHUTTING_DOWN))
            };
            ctx.hub.respond(client, &to_response(id, result));
        }
    }
}

fn fail(message: impl Into<String>) -> ErrorBody {
    ErrorBody { message: message.into(), node_id: None }
}

fn to_response(id: u64, result: Result<Value, ErrorBody>) -> Response {
    match result {
        Ok(v) => Response::ok(id, v),
        Err(e) => Response { id, result: None, error: Some(e) },
    }
}

#[derive(Deserialize)]
struct CompileParams {
    r#macro: Macro,
}

/// Graph → JS (without trace calls, as shown in the editor's Code tab), then a syntax check of the result.
fn compile_macro(runner: &dyn MacroRunner, params: Value) -> Result<Value, ErrorBody> {
    let CompileParams { r#macro } = serde_json::from_value(params).map_err(|e| fail(format!("compileMacro: invalid params: {e}")))?;
    let compiled = compile(&r#macro, &CompileOptions::default()).map_err(|e| ErrorBody { message: e.message, node_id: e.node_id })?;
    runner.check_syntax(&compiled.code).map_err(fail)?;
    Ok(json!({ "code": compiled.code }))
}
