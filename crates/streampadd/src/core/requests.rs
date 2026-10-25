//! IPC methods that read or change core state (docs/SPEC.md §4.1). Methods that need KWin or other async work
//! are handled by the IPC server instead.

use super::overlay::OverlayAction;
use super::{ClientId, Core, Effect};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};
use streampad_core::compile::{CompileOptions, compile};
use streampad_core::ipc::{ErrorBody, OutputInfo};
use streampad_core::{KeyId, Macro, VERSION};
use streampad_input::KeyAction;
use streampad_script::Invocation;

type Reply = Result<Value, ErrorBody>;

fn fail(message: impl Into<String>) -> ErrorBody {
    ErrorBody { message: message.into(), node_id: None }
}

fn params<T: DeserializeOwned>(method: &str, params: Value) -> Result<T, ErrorBody> {
    let params = if params.is_null() { json!({}) } else { params };
    serde_json::from_value(params).map_err(|e| fail(format!("{method}: invalid params: {e}")))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HelloParams {
    #[serde(default)]
    layer_shell: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MacroModeParams {
    on: Option<bool>,
    #[serde(default)]
    toggle: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProfileParams {
    index: Option<usize>,
    id: Option<String>,
    delta: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OverlayParams {
    action: String,
    duration_ms: Option<u64>,
    #[serde(default)]
    preview: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunBindingParams {
    profile_id: String,
    key: KeyId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunMacroParams {
    r#macro: Macro,
    key: Option<KeyId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunIdParams {
    run_id: u64,
}

#[derive(Deserialize)]
struct OnParams {
    on: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OutputsParams {
    List(Vec<OutputInfo>),
    Wrapped { outputs: Vec<OutputInfo> },
}

#[derive(Deserialize)]
struct SimulateKeyParams {
    key: String,
    #[serde(default = "pressed_default")]
    pressed: bool,
}

fn pressed_default() -> bool {
    true
}

#[derive(Deserialize)]
struct SetVarParams {
    name: String,
    #[serde(default)]
    value: Value,
}

impl Core {
    pub fn request(&mut self, client: ClientId, method: &str, p: Value, now: Instant) -> Reply {
        let reply = match method {
            "hello" => self.req_hello(params(method, p)?),
            "getState" => Ok(json!(self.snapshot())),
            "setMacroMode" => self.req_macro_mode(params(method, p)?, now),
            "setProfile" => self.req_profile(params(method, p)?, now),
            "overlay" => self.req_overlay(params(method, p)?, now),
            "runBinding" => self.req_run_binding(params(method, p)?),
            "runMacro" => self.req_run_macro(params(method, p)?),
            "cancelRun" => {
                let RunIdParams { run_id } = params(method, p)?;
                self.runs.cancel(run_id);
                Ok(json!({}))
            }
            "setLearn" => {
                let OnParams { on } = params(method, p)?;
                self.set_learn(on, Some(client), now);
                Ok(json!({}))
            }
            "listDevices" => {
                self.devices = self.deps.input.devices();
                Ok(json!(self.devices))
            }
            "listOutputs" => Ok(json!(self.known_outputs())),
            "reportOutputs" => {
                let outputs = match params(method, p)? {
                    OutputsParams::List(o) | OutputsParams::Wrapped { outputs: o } => o,
                };
                self.reported_outputs = Some(outputs);
                Ok(json!({}))
            }
            "reload" => self.reload_from_disk(now).map(|()| json!({})).map_err(fail),
            "simulateKey" => self.req_simulate_key(params(method, p)?, now),
            "getVars" => Ok(json!(self.vars)),
            "setVar" => {
                let SetVarParams { name, value } = params(method, p)?;
                self.vars.insert(name, value);
                Ok(json!({}))
            }
            "quit" => {
                self.effects.push(Effect::Quit);
                Ok(json!({}))
            }
            _ => Err(fail(format!("unknown method \"{method}\""))),
        };
        self.settle(now);
        reply
    }

    fn req_hello(&mut self, p: HelloParams) -> Reply {
        if p.layer_shell.is_some() {
            self.integrations.layer_shell = p.layer_shell;
        }
        Ok(json!({ "version": VERSION, "state": self.snapshot() }))
    }

    fn req_macro_mode(&mut self, p: MacroModeParams, now: Instant) -> Reply {
        let on = match (p.on, p.toggle) {
            (_, true) => !self.macro_mode,
            (Some(on), false) => on,
            (None, false) => return Err(fail("setMacroMode: pass {\"on\": bool} or {\"toggle\": true}")),
        };
        self.set_macro_mode(on, now);
        Ok(json!({ "macroMode": self.macro_mode }))
    }

    fn req_profile(&mut self, p: ProfileParams, now: Instant) -> Reply {
        let count = self.cfg.profiles.len();
        match (p.index, p.id, p.delta) {
            (Some(index), _, _) if index < count => self.jump_to_profile(index, now),
            (Some(index), _, _) => return Err(fail(format!("no profile at index {index} (there are {count})"))),
            (None, Some(id), _) => {
                let index = self.cfg.profile_index(&id).ok_or_else(|| fail(format!("no profile with id \"{id}\"")))?;
                self.jump_to_profile(index, now);
            }
            (None, None, Some(delta)) => self.step_profile(delta, now),
            (None, None, None) => return Err(fail("setProfile: pass index, id or delta")),
        }
        Ok(json!({ "profileIndex": self.profile_index }))
    }

    /// Direct selection: the slide direction follows the position change.
    pub(super) fn jump_to_profile(&mut self, index: usize, now: Instant) {
        let direction = if index > self.profile_index { 1 } else { -1 };
        self.select_profile(index, direction, now);
    }

    fn req_overlay(&mut self, p: OverlayParams, now: Instant) -> Reply {
        let action = OverlayAction::parse(&p.action).ok_or_else(|| fail(format!("unknown overlay action \"{}\"", p.action)))?;
        let duration = p.duration_ms.map_or_else(|| self.peek_duration(), Duration::from_millis);
        if p.preview {
            self.overlay.apply_preview(action, now + duration, now);
        } else {
            self.overlay.apply(action, now + duration);
        }
        Ok(json!({ "visible": self.overlay.wanted(self.macro_mode, now) }))
    }

    /// Test run of a saved binding: press, then release right away so an `onRelease` handler runs too.
    fn req_run_binding(&mut self, p: RunBindingParams) -> Reply {
        let index = self.cfg.profile_index(&p.profile_id).ok_or_else(|| fail(format!("no profile with id \"{}\"", p.profile_id)))?;
        let profile = &self.cfg.profiles[index];
        let bound = profile.keys.get(&p.key).and_then(|b| Some((super::run_title(&b.label, p.key), b.r#macro.clone()?)));
        let Some((title, m)) = bound else {
            return Err(fail(format!("nothing is bound to {} in \"{}\"", p.key.display_name(), profile.name)));
        };
        let inv = Invocation {
            run_id: 0,
            key: Some(p.key),
            repeat: false,
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
        };
        self.test_run(&m, inv, title)
    }

    fn req_run_macro(&mut self, p: RunMacroParams) -> Reply {
        let profile = self.current_profile();
        let inv = Invocation {
            run_id: 0,
            key: p.key,
            repeat: false,
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
        };
        let title = p.key.map_or("Test run", KeyId::display_name).to_string();
        self.test_run(&p.r#macro, inv, title)
    }

    fn test_run(&mut self, m: &Macro, inv: Invocation, title: String) -> Reply {
        let code = compile(m, &CompileOptions { trace: true })
            .map_err(|e| ErrorBody { message: e.message.clone(), node_id: e.node_id.clone() })?;
        if !self.runs.has_capacity() {
            return Err(fail("too many macros are running"));
        }
        let run_id = self.start_run(Arc::new(code), inv, None, title);
        self.runs.release(run_id);
        Ok(json!({ "runId": run_id }))
    }

    fn req_simulate_key(&mut self, p: SimulateKeyParams, now: Instant) -> Reply {
        if p.key.eq_ignore_ascii_case("toggle") {
            if p.pressed {
                self.set_macro_mode(!self.macro_mode, now);
            }
            return Ok(json!({}));
        }
        let key = KeyId::parse(&p.key).ok_or_else(|| fail(format!("unknown key \"{}\"", p.key)))?;
        if !p.pressed {
            self.pad_key(key, KeyAction::Release, now);
        } else if self.macro_mode && self.capture.contains(&key) {
            self.pad_key(key, KeyAction::Press, now);
        }
        Ok(json!({}))
    }
}
