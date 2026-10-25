//! StreamPad daemon (docs/SPEC.md §2, §4–§7). The binary in `main.rs` only parses the command line.

pub mod backend;
pub mod cli;
pub mod command;
pub mod config_store;
pub mod core;
pub mod ctl;
pub mod daemon;
pub mod host;
pub mod instance;
pub mod ipc;
pub mod kwin;
pub mod logging;
pub mod overlay_proc;
pub mod persist;
