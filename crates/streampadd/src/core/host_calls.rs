//! Macro host methods that act on daemon state (docs/SPEC.md §6): `profile.*`, `macroMode.*`, `overlay.set`,
//! `toast` and `vars.*`. Running inside the core makes `vars.toggle` atomic across parallel runs.

use super::Core;
use super::overlay::OverlayAction;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use streampad_core::ipc::ToastKind;

pub(super) fn str_arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, String> {
    args.get(name).and_then(Value::as_str).ok_or_else(|| format!("missing string argument \"{name}\""))
}

/// JavaScript truthiness of a JSON value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

impl Core {
    pub fn host_call(&mut self, method: &str, args: Value, now: Instant) -> Result<Value, String> {
        let result = self.dispatch_host(method, &args, now);
        self.settle(now);
        result
    }

    fn dispatch_host(&mut self, method: &str, args: &Value, now: Instant) -> Result<Value, String> {
        match method {
            "profile.current" => {
                let p = self.current_profile();
                Ok(json!({ "id": p.id, "name": p.name, "index": self.profile_index }))
            }
            "profile.set" => self.host_profile_set(args.get("target").unwrap_or(&Value::Null), now).map(|()| Value::Null),
            "macroMode.get" => Ok(json!(self.macro_mode)),
            "macroMode.set" => {
                let on = args.get("on").and_then(Value::as_bool).ok_or("macroMode.set needs a boolean \"on\"")?;
                self.set_macro_mode(on, now);
                Ok(Value::Null)
            }
            "macroMode.toggle" => {
                self.set_macro_mode(!self.macro_mode, now);
                Ok(json!(self.macro_mode))
            }
            "overlay.set" => {
                let value = str_arg(args, "value")?;
                let action = OverlayAction::parse(value).ok_or_else(|| format!("unknown overlay action \"{value}\""))?;
                let duration = args.get("durationMs").and_then(Value::as_u64).map_or_else(|| self.peek_duration(), Duration::from_millis);
                self.overlay.apply(action, now + duration);
                Ok(Value::Null)
            }
            "toast" => {
                let text = str_arg(args, "text")?.to_string();
                let duration = args.get("durationMs").and_then(Value::as_u64).unwrap_or(1600).min(60_000) as u32;
                let kind = match args.get("kind").and_then(Value::as_str) {
                    None | Some("info") => ToastKind::Info,
                    Some("success") => ToastKind::Success,
                    Some("error") => ToastKind::Error,
                    Some(other) => return Err(format!("unknown toast kind \"{other}\"")),
                };
                self.toast(text, kind, duration);
                Ok(Value::Null)
            }
            "vars.get" => Ok(self.vars.get(str_arg(args, "name")?).cloned().unwrap_or(Value::Null)),
            "vars.set" => {
                let name = str_arg(args, "name")?.to_string();
                self.vars.insert(name, args.get("value").cloned().unwrap_or(Value::Null));
                Ok(Value::Null)
            }
            "vars.toggle" => {
                let name = str_arg(args, "name")?;
                let on = !self.vars.get(name).is_some_and(truthy);
                self.vars.insert(name.to_string(), Value::Bool(on));
                Ok(json!(on))
            }
            _ => Err(format!("unknown host method \"{method}\"")),
        }
    }

    /// `profile.set(target)`: "next" | "prev" | profile id | profile name | 0-based index.
    fn host_profile_set(&mut self, target: &Value, now: Instant) -> Result<(), String> {
        let index = match target {
            Value::String(s) if s == "next" || s == "prev" => {
                self.step_profile(if s == "next" { 1 } else { -1 }, now);
                return Ok(());
            }
            Value::String(s) => self
                .cfg
                .profile_index(s)
                .or_else(|| self.cfg.profiles.iter().position(|p| p.name.eq_ignore_ascii_case(s.trim())))
                .ok_or_else(|| format!("no profile \"{s}\""))?,
            Value::Number(n) => {
                let i = n.as_u64().filter(|i| (*i as usize) < self.cfg.profiles.len());
                i.ok_or_else(|| format!("no profile at index {n}"))? as usize
            }
            _ => return Err("profile.set needs a target: \"next\", \"prev\", an id, a name or an index".into()),
        };
        self.jump_to_profile(index, now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::truthy;
    use serde_json::json;

    #[test]
    fn js_truthiness() {
        for v in [json!(null), json!(false), json!(0), json!(""), json!(0.0)] {
            assert!(!truthy(&v), "{v} should be falsy");
        }
        for v in [json!(true), json!(1), json!("x"), json!([]), json!({})] {
            assert!(truthy(&v), "{v} should be truthy");
        }
    }
}
