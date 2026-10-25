//! `state.json` (docs/SPEC.md §3.5): runtime state that survives restarts.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use streampad_core::config::write_atomic;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PersistedState {
    pub macro_mode: bool,
    pub profile_id: String,
    pub pinned: bool,
    pub vars: BTreeMap<String, Value>,
}

impl PersistedState {
    /// Missing file → `None`. A corrupt file is logged and treated as missing (it is daemon-owned and gets
    /// rewritten on the next change).
    pub fn load(path: &Path) -> Option<PersistedState> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                log::warn!("cannot read {}: {e}", path.display());
                return None;
            }
        };
        serde_json::from_str(&text)
            .inspect_err(|e| log::warn!("ignoring corrupt {}: {e}", path.display()))
            .ok()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        write_atomic(path, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        assert_eq!(PersistedState::load(&path), None);
        let state = PersistedState {
            macro_mode: true,
            profile_id: "p_1".into(),
            pinned: true,
            vars: BTreeMap::from([("gameMode".into(), Value::Bool(true))]),
        };
        state.save(&path).unwrap();
        assert_eq!(PersistedState::load(&path), Some(state));
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(PersistedState::load(&path), None);
    }
}
