//! Command error type. Serialized as a plain string so the webview receives a readable message
//! (`backend.tauri.ts` turns it into `Error(message)`).

use serde::{Serialize, Serializer};
use std::fmt;

#[derive(Debug)]
pub struct CmdError(String);

pub type CmdResult<T> = Result<T, CmdError>;

impl CmdError {
    pub fn new(message: impl Into<String>) -> Self {
        CmdError(message.into())
    }
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for CmdError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl From<std::io::Error> for CmdError {
    fn from(e: std::io::Error) -> Self {
        CmdError(e.to_string())
    }
}

impl From<streampad_core::ConfigError> for CmdError {
    fn from(e: streampad_core::ConfigError) -> Self {
        CmdError(e.to_string())
    }
}

/// Run blocking filesystem / process work off the async runtime's worker threads.
pub async fn blocking<T, F>(f: F) -> CmdResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CmdResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| CmdError::new(format!("background task failed: {e}")))?
}
