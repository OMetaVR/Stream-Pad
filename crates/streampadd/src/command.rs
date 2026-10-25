//! Messages to the event loop that owns the core.

use crate::core::ClientId;
use serde_json::Value;
use streampad_core::ipc::{ErrorBody, OutputInfo};
use streampad_input::EngineEvent;
use streampad_script::RunOutcome;
use tokio::sync::{mpsc, oneshot};

pub enum Command {
    Engine(EngineEvent),
    /// An IPC request for a method the core implements.
    Request { client: ClientId, method: String, params: Value, reply: oneshot::Sender<Result<Value, ErrorBody>> },
    /// A macro host call that needs core state (see `core::host_calls`).
    Host { method: String, args: Value, reply: oneshot::Sender<Result<Value, String>> },
    RunDone { run_id: u64, outcome: RunOutcome },
    ClientGone(ClientId),
    ConfigChanged,
    Kwin { available: bool, outputs: Vec<OutputInfo> },
    OutputResolved { seq: u64, output: Option<String> },
    /// SIGTERM / SIGINT.
    Shutdown,
}

/// Unbounded so worker threads, the engine and D-Bus handlers never block on the event loop. IPC clients
/// are limited to one in-flight core request each, which bounds what they can queue.
pub type CommandTx = mpsc::UnboundedSender<Command>;
pub type CommandRx = mpsc::UnboundedReceiver<Command>;
