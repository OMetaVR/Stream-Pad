//! The macro runtime's view of the system (docs/SPEC.md §6 "Host boundary"). Called on worker threads, so
//! everything here may block; core-state methods are forwarded to the event loop.

pub mod desktop_entry;
pub mod launch;
pub mod session;
pub mod spawn;

use crate::backend::Cancellations;
use crate::command::{Command, CommandTx};
use crate::ipc::hub::Hub;
use crate::kwin::{Kwin, UNSUPPORTED};
use launch::{Launcher, expand_user_path};
use serde_json::{Value, json};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use streampad_core::combo::{self, Combo};
use streampad_core::ipc::{LogEvent, LogLevel, TraceEvent, events};
use streampad_core::paths::Paths;
use streampad_input::{KeyOutput, UntypableText};
use streampad_script::{Host, RunContext};
use tokio::runtime::Handle;
use tokio::sync::oneshot;

/// Desktop-file id of the StreamPad editor (packaging/applications), also its icon name.
pub const APP_ID: &str = "io.github.ometavr.streampad";

const SHUTTING_DOWN: &str = "StreamPad is shutting down";
const NO_KEYBOARD: &str = "keyboard output is unavailable (StreamPad runs without input access)";
const WINDOW_ACTIONS: &[&str] = &["minimize", "maximize", "fullscreen", "close", "keepAbove", "center"];
const DIRECTIONS: &[&str] = &["next", "prev", "left", "right", "up", "down"];

pub struct DaemonHost {
    pub core: CommandTx,
    pub rt: Handle,
    pub hub: Arc<Hub>,
    pub kwin: Option<Arc<Kwin>>,
    pub bus: Option<zbus::Connection>,
    pub keys: Option<KeyOutput>,
    pub paths: Arc<Paths>,
    pub launcher: Arc<Launcher>,
    pub cancellations: Arc<Cancellations>,
}

fn str_arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, String> {
    args.get(name).and_then(Value::as_str).ok_or_else(|| format!("missing string argument \"{name}\""))
}

fn bool_arg(args: &Value, name: &str, default: bool) -> bool {
    args.get(name).and_then(Value::as_bool).unwrap_or(default)
}

fn combo_arg(args: &Value) -> Result<Combo, String> {
    let s = str_arg(args, "combo")?;
    combo::parse_combo(s).map_err(|e| format!("invalid shortcut \"{s}\": {e}"))
}

fn null(r: Result<(), String>) -> Result<Value, String> {
    r.map(|()| Value::Null)
}

/// A desktop target: one of `DIRECTIONS` or a 1-based desktop number (also accepted as a numeric string).
fn desktop_target(args: &Value) -> Result<Value, String> {
    let target = args.get("target").unwrap_or(&Value::Null);
    let number: Option<u64> = match target {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    };
    match (target.as_str(), number) {
        (_, Some(n)) if n >= 1 => Ok(json!(n)),
        (Some(s), None) if DIRECTIONS.contains(&s) => Ok(json!(s)),
        _ => Err(format!("invalid desktop target {target}")),
    }
}

impl DaemonHost {
    fn keys(&self) -> Result<&KeyOutput, String> {
        self.keys.as_ref().ok_or_else(|| NO_KEYBOARD.to_string())
    }

    fn kwin(&self) -> Result<&Kwin, String> {
        self.kwin.as_deref().filter(|k| k.available()).ok_or_else(|| UNSUPPORTED.to_string())
    }

    fn bus(&self) -> Result<&zbus::Connection, String> {
        self.bus.as_ref().ok_or_else(|| "no D-Bus session bus".to_string())
    }

    fn block<T>(&self, f: impl Future<Output = T>) -> T {
        self.rt.block_on(f)
    }

    fn ask_core(&self, method: &str, args: Value) -> Result<Value, String> {
        let (reply, answer) = oneshot::channel();
        self.core.send(Command::Host { method: method.to_string(), args, reply }).map_err(|_| SHUTTING_DOWN)?;
        answer.blocking_recv().map_err(|_| SHUTTING_DOWN.to_string())?
    }

    fn tap(&self, combo: &Combo) -> Result<(), String> {
        self.keys()?.tap(combo).map_err(|e| e.to_string())
    }

    /// Type via the virtual keyboard; text the US layout can't produce is pasted through the clipboard.
    fn type_text(&self, args: &Value) -> Result<(), String> {
        let text = str_arg(args, "text")?;
        let delay = Duration::from_millis(args.get("delayMs").and_then(Value::as_u64).unwrap_or(8).min(1000));
        match self.keys()?.type_text(text, delay) {
            Ok(()) => Ok(()),
            Err(e) if e.downcast_ref::<UntypableText>().is_some() => {
                session::clipboard_set(text)?;
                // Let the clipboard owner settle before the target app asks for the content.
                std::thread::sleep(Duration::from_millis(60));
                self.tap(&Combo { keys: vec![combo::KEY_LEFTCTRL, KEY_V] })
            }
            Err(e) => Err(e.to_string()),
        }
    }

    fn media(&self, args: &Value) -> Result<(), String> {
        let action = str_arg(args, "action")?;
        let key = match action {
            "playPause" => "playpause",
            "next" => "nextsong",
            "prev" => "previoussong",
            "stop" => "stopcd",
            "volumeUp" => "volumeup",
            "volumeDown" => "volumedown",
            "mute" => "mute",
            other => return Err(format!("unknown media action \"{other}\"")),
        };
        self.tap(&combo::parse_combo(key).map_err(|e| e.to_string())?)
    }

    fn run_command(&self, run: &RunContext, args: &Value) -> Result<Value, String> {
        let command = str_arg(args, "command")?;
        if command.trim().is_empty() {
            return Err("run: empty command".into());
        }
        if !bool_arg(args, "wait", false) {
            let argv = ["/bin/sh".to_string(), "-c".into(), command.into()];
            return null(self.launcher.spawner().detached(&argv, None, None));
        }
        let run_id = run.run_id;
        let out = spawn::run_captured(command, || self.cancellations.is_cancelled(run_id))?;
        Ok(json!({ "code": out.code, "stdout": out.stdout, "stderr": out.stderr }))
    }

    fn open_path(&self, args: &Value) -> Result<(), String> {
        let path = expand_user_path(str_arg(args, "path")?);
        if !path.exists() {
            return Err(format!("no such file or folder: {}", path.display()));
        }
        self.launcher.open(&path.to_string_lossy())
    }

    fn window_call(&self, method: &str, args: &Value) -> Result<Value, String> {
        let kwin = self.kwin()?;
        match method {
            "window.focus" => {
                let by_title = args.get("by").and_then(Value::as_str) == Some("title");
                self.block(kwin.focus(str_arg(args, "match")?, by_title)).map(Value::Bool)
            }
            "window.moveToOutput" => {
                let target = str_arg(args, "target")?.trim();
                if target.is_empty() {
                    return Err("window.moveToOutput needs a target".into());
                }
                null(self.block(kwin.move_to_output(target)))
            }
            "window.action" => {
                let action = str_arg(args, "action")?;
                if !WINDOW_ACTIONS.contains(&action) {
                    return Err(format!("unknown window action \"{action}\""));
                }
                null(self.block(kwin.window_action(action)))
            }
            "desktop.switch" => null(self.block(kwin.desktop_switch(&desktop_target(args)?))),
            "desktop.moveWindow" => null(self.block(kwin.desktop_move_window(&desktop_target(args)?, bool_arg(args, "follow", false)))),
            "desktop.create" => null(self.block(kwin.desktop_create())),
            "desktop.remove" => null(self.block(kwin.desktop_remove())),
            "desktop.current" => self.block(kwin.desktop_current()),
            "desktop.overview" => null(self.block(session::kde_shortcut(self.bus()?, "kwin", "Overview"))),
            _ => Err(format!("unknown host method \"{method}\"")),
        }
    }

    fn log(&self, run: &RunContext, args: &Value) {
        let level = match args.get("level").and_then(Value::as_str) {
            Some("warn") => LogLevel::Warn,
            Some("error") => LogLevel::Error,
            _ => LogLevel::Info,
        };
        let message = match args.get("message") {
            Some(Value::String(s)) => s.clone(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        log::debug!(target: "macro", "run {}: {message}", run.run_id);
        let event = LogEvent { run_id: Some(run.run_id), level, message };
        self.hub.broadcast(events::LOG, json!(event), false);
    }
}

const KEY_V: u16 = 47;

impl Host for DaemonHost {
    fn call(&self, run: RunContext, method: &str, args: Value) -> Result<Value, String> {
        match method {
            "key.press" => null(self.tap(&combo_arg(&args)?)),
            "key.down" => null(self.keys()?.key_down(&combo_arg(&args)?).map_err(|e| e.to_string())),
            "key.up" => null(self.keys()?.key_up(&combo_arg(&args)?).map_err(|e| e.to_string())),
            "key.type" => null(self.type_text(&args)),
            "key.media" => null(self.media(&args)),
            "app.launch" => {
                let app = str_arg(&args, "app")?;
                let extra = args.get("args").and_then(Value::as_str).unwrap_or("");
                null(self.launcher.launch_app(app, bool_arg(&args, "focusIfRunning", true), extra))
            }
            "app.close" => {
                let app = str_arg(&args, "app")?;
                self.launcher.close_app(app, bool_arg(&args, "force", false)).map(Value::Bool)
            }
            "open.url" => {
                let url = str_arg(&args, "url")?.trim();
                if url.is_empty() {
                    return Err("openUrl: empty URL".into());
                }
                null(self.launcher.open(url))
            }
            "open.path" => null(self.open_path(&args)),
            "run.command" => self.run_command(&run, &args),
            "window.active" => Ok(json!(self.kwin.as_ref().and_then(|k| k.active_window()))),
            // Queries degrade to "nothing there" off KDE so conditions simply evaluate false.
            "window.list" => match self.kwin() {
                Ok(kwin) => self.block(kwin.windows()).map(|w| json!(w)),
                Err(_) => Ok(json!([])),
            },
            m if m.starts_with("window.") || m.starts_with("desktop.") => self.window_call(m, &args),
            "kde.shortcut" => {
                let (component, name) = (str_arg(&args, "component")?, str_arg(&args, "name")?);
                null(self.block(session::kde_shortcut(self.bus()?, component, name)))
            }
            "notify" => {
                let title = str_arg(&args, "title")?;
                let body = args.get("body").and_then(Value::as_str).unwrap_or("");
                null(self.block(session::notify(self.bus()?, title, body)))
            }
            "clipboard.get" => session::clipboard_get().map(Value::String),
            "clipboard.set" => null(session::clipboard_set(str_arg(&args, "text")?)),
            "sound.play" => null(session::play_sound(&self.paths.resolve_asset(str_arg(&args, "path")?))),
            "log" => {
                self.log(&run, &args);
                Ok(Value::Null)
            }
            "trace" => {
                let node_id = str_arg(&args, "nodeId")?.to_string();
                self.hub.broadcast(events::TRACE, json!(TraceEvent { run_id: run.run_id, node_id }), true);
                Ok(Value::Null)
            }
            _ => self.ask_core(method, args),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_targets() {
        assert_eq!(desktop_target(&json!({ "target": "left" })).unwrap(), json!("left"));
        assert_eq!(desktop_target(&json!({ "target": 3 })).unwrap(), json!(3));
        assert_eq!(desktop_target(&json!({ "target": " 2 " })).unwrap(), json!(2));
        assert!(desktop_target(&json!({ "target": 0 })).is_err());
        assert!(desktop_target(&json!({ "target": "sideways" })).is_err());
        assert!(desktop_target(&json!({})).is_err());
    }
}
