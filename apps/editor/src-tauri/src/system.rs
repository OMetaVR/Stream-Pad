//! Small system commands: open a path with the desktop's default handler, compile a macro locally.

use crate::error::{CmdError, CmdResult};
use serde_json::{Map, Value};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use streampad_core::Macro;
use streampad_core::compile::{CompileOptions, compile};

#[tauri::command]
pub fn open_path(path: String) -> CmdResult<()> {
    if path.trim().is_empty() {
        return Err(CmdError::new("no path to open"));
    }
    let mut child = Command::new("xdg-open")
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| CmdError::new(format!("cannot run xdg-open: {e}")))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// `CompileResult` in `src/lib/backend.ts`: `{ ok: true, code }` or `{ ok: false, error, nodeId? }`.
#[tauri::command]
pub async fn compile_macro(macro_def: Macro) -> Value {
    let mut out = Map::new();
    match compile(&macro_def, &CompileOptions::default()) {
        Ok(compiled) => {
            out.insert("ok".into(), true.into());
            out.insert("code".into(), compiled.code.into());
        }
        Err(e) => {
            out.insert("ok".into(), false.into());
            out.insert("error".into(), e.message.into());
            if let Some(node) = e.node_id {
                out.insert("nodeId".into(), node.into());
            }
        }
    }
    Value::Object(out)
}
