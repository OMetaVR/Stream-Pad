//! Graph → JavaScript compiler (docs/SPEC.md §3.3, §6).
//!
//! The output is meant to be read (and adopted via "Convert to script"), so it is structured like
//! hand-written code: linear chains are plain statement lists, `flow.if` becomes `if/else` with the
//! branches re-joined at the immediate post-dominator, `flow.repeat` becomes a `for` loop and `flow.stop`
//! a `return`. Graphs that do not fit that shape (a node shared by two otherwise unrelated paths) are
//! emitted in an equivalent function-per-node form instead.
//!
//! Pipeline: `graph::Graph::build` validates the graph and lowers every reachable node to a `lower::Op`
//! (parameter checks happen there), then `emit` prints JavaScript.

mod emit;
mod graph;
mod js;
mod lower;
#[cfg(test)]
mod tests;

use crate::config::Macro;

#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// Insert `__node("<id>")` calls before each node so the runtime can emit `trace` events.
    pub trace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledMacro {
    /// Plain JS: top-level = press handler; optional `function onRelease(event) { … }`.
    pub code: String,
    /// Whether the code defines `onRelease` (graph: has a connected `trigger.release`; script: mentions
    /// `onRelease`). Conservative for scripts: it may be true for code that never defines the handler.
    pub has_release: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct CompileError {
    pub message: String,
    /// Graph node the error is about, if any (editor highlights it).
    pub node_id: Option<String>,
}

impl CompileError {
    fn graph(message: impl Into<String>) -> Self {
        CompileError { message: message.into(), node_id: None }
    }

    fn node(node_id: &str, message: impl Into<String>) -> Self {
        CompileError { message: message.into(), node_id: Some(node_id.to_string()) }
    }
}

pub fn compile(m: &Macro, opts: &CompileOptions) -> Result<CompiledMacro, CompileError> {
    match m {
        Macro::Script { source } => {
            Ok(CompiledMacro { code: source.clone(), has_release: source.contains("onRelease") })
        }
        Macro::Graph { nodes, edges } => {
            let graph = graph::Graph::build(nodes, edges)?;
            Ok(emit::emit(&graph, opts))
        }
    }
}
