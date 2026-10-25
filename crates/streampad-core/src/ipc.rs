//! Daemon socket protocol (docs/SPEC.md §4): newline-delimited JSON over a Unix socket.

use crate::config::{GlobalAction, OverlaySettings};
use crate::keys::{KeyId, Legend};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const PROTOCOL_VERSION: u32 = 1;

/// client → daemon
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
}

/// daemon → client, exactly one per request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Response { id, result: Some(result), error: None }
    }
    pub fn err(id: u64, message: impl Into<String>) -> Self {
        Response { id, result: None, error: Some(ErrorBody { message: message.into(), node_id: None }) }
    }
}

/// daemon → subscribed clients.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventMsg {
    pub event: String,
    pub data: Value,
}

/// What a client reads from the socket: either a response or an event.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Incoming {
    Response(Response),
    Event(EventMsg),
}

// ---------------------------------------------------------------------------------------------------------
// State snapshot

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaemonState {
    pub version: String,
    pub macro_mode: bool,
    pub profile_index: usize,
    pub profiles: Vec<ProfileSummary>,
    pub profile: RenderProfile,
    pub overlay: OverlayState,
    pub learn: bool,
    pub devices: Vec<DeviceInfo>,
    pub integrations: Integrations,
    pub config_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    /// Resolved absolute path (or null).
    pub icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderProfile {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    /// All 17 keys are present.
    pub keys: BTreeMap<KeyId, RenderKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenderKeyKind {
    Macro,
    Global,
    Empty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderKey {
    pub label: String,
    /// Absolute file path, or null.
    pub icon: Option<String>,
    /// True for monochrome (lucide) icons that should be tinted with the text colour.
    pub icon_tint: bool,
    /// Absolute file path, or null.
    pub cover: Option<String>,
    /// Visible part of the cover (normalized image coordinates); null = centred cover fit.
    pub cover_crop: Option<crate::config::CropRect>,
    pub color: Option<String>,
    pub kind: RenderKeyKind,
    pub global: Option<GlobalAction>,
    pub legend: Legend,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    pub visible: bool,
    pub pinned: bool,
    /// Shown via the editor's "Preview overlay" (ignores macro-mode visibility rules).
    pub preview: bool,
    /// Connector name to show on; null = compositor decides.
    pub output: Option<String>,
    pub settings: OverlaySettings,
    /// Effective accent: profile colour, else settings.overlay.accent.
    pub accent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// "<vendor hex4>:<product hex4>:<name>"
    pub id: String,
    pub name: String,
    pub path: String,
    pub phys: Option<String>,
    pub has_numpad: bool,
    pub eligible: bool,
    pub grabbed: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputInfo {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
    pub primary: bool,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: String,
    pub title: String,
    pub app_id: String,
    pub resource_class: String,
    pub desktop_file: String,
    pub pid: i64,
    pub output: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Integrations {
    pub kwin: bool,
    pub layer_shell: Option<bool>,
}

// ---------------------------------------------------------------------------------------------------------
// Event payloads

pub mod events {
    pub const STATE: &str = "state";
    pub const KEY: &str = "key";
    pub const PROFILE: &str = "profile";
    pub const TOAST: &str = "toast";
    pub const LEARN: &str = "learn";
    pub const RUN: &str = "run";
    pub const TRACE: &str = "trace";
    pub const LOG: &str = "log";
    pub const CONFIG_ERROR: &str = "configError";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyEventKind {
    Macro,
    Global,
    Empty,
    Learn,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyEvent {
    pub key: KeyId,
    pub pressed: bool,
    pub kind: KeyEventKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileEvent {
    pub index: usize,
    pub direction: i8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToastEvent {
    pub id: u64,
    pub text: String,
    pub kind: ToastKind,
    pub duration_ms: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Started,
    Finished,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunEvent {
    pub run_id: u64,
    pub key: Option<KeyId>,
    pub status: RunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEvent {
    pub run_id: u64,
    pub node_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub run_id: Option<u64>,
    pub level: LogLevel,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incoming_distinguishes() {
        let r: Incoming = serde_json::from_str(r#"{"id":1,"result":{"a":1}}"#).unwrap();
        assert!(matches!(r, Incoming::Response(_)));
        let e: Incoming = serde_json::from_str(r#"{"event":"state","data":{}}"#).unwrap();
        assert!(matches!(e, Incoming::Event(_)));
    }
}
