//! StreamPad shared model: config schema, IPC protocol, key table, paths and the macro compiler.
//! See docs/SPEC.md — this crate is the source of truth for every JSON shape.

pub mod combo;
pub mod compile;
pub mod config;
pub mod defaults;
pub mod ipc;
mod key_table;
pub mod keys;
pub mod paths;

pub use config::*;
pub use keys::{KeyId, Legend};

/// Version string reported over IPC.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
