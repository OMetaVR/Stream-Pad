//! `list_kde_shortcuts`: every global shortcut registered with KDE's kglobalaccel, for the
//! `kde.shortcut` macro node picker. Empty (not an error) outside Plasma.

mod qt_keys;

use crate::error::CmdResult;
use serde::Serialize;
use std::time::Duration;
use zbus::Connection;
use zbus::zvariant::OwnedObjectPath;

const SERVICE: &str = "org.kde.kglobalaccel";
const TIMEOUT: Duration = Duration::from_secs(4);

/// Mirrors `KdeShortcut` in `src/lib/backend.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KdeShortcut {
    component: String,
    component_name: String,
    name: String,
    friendly_name: String,
    keys: Vec<String>,
}

/// KGlobalShortcutInfo as marshalled by KF6:
/// (uniqueName, friendlyName, componentUniqueName, componentFriendlyName, contextUniqueName,
///  contextFriendlyName, keys, defaultKeys).
type ShortcutInfo = (String, String, String, String, String, String, Vec<i32>, Vec<i32>);

#[tauri::command]
pub async fn list_kde_shortcuts() -> CmdResult<Vec<KdeShortcut>> {
    match tokio::time::timeout(TIMEOUT, query()).await {
        Ok(Ok(list)) => Ok(list),
        Ok(Err(e)) => {
            log::info!("kglobalaccel unavailable: {e}");
            Ok(Vec::new())
        }
        Err(_) => {
            log::warn!("kglobalaccel did not answer within {TIMEOUT:?}");
            Ok(Vec::new())
        }
    }
}

async fn query() -> zbus::Result<Vec<KdeShortcut>> {
    let conn = Connection::session().await?;
    let components: Vec<OwnedObjectPath> = conn
        .call_method(Some(SERVICE), "/kglobalaccel", Some("org.kde.KGlobalAccel"), "allComponents", &())
        .await?
        .body()
        .deserialize()?;
    let mut out = Vec::new();
    for path in components {
        let reply = conn
            .call_method(Some(SERVICE), path.as_str(), Some("org.kde.kglobalaccel.Component"), "allShortcutInfos", &())
            .await;
        // One misbehaving component must not hide the others.
        let infos: Vec<ShortcutInfo> = match reply.and_then(|m| m.body().deserialize()) {
            Ok(infos) => infos,
            Err(e) => {
                log::debug!("skipping kglobalaccel component {}: {e}", path.as_str());
                continue;
            }
        };
        for (name, friendly, component, component_name, _ctx, _ctx_name, keys, _defaults) in infos {
            out.push(KdeShortcut {
                friendly_name: if friendly.trim().is_empty() { name.clone() } else { friendly },
                component_name: if component_name.trim().is_empty() { component.clone() } else { component_name },
                keys: keys.into_iter().filter(|k| *k != 0).map(qt_keys::format_key_combination).collect(),
                component,
                name,
            });
        }
    }
    out.sort_by_cached_key(|s| (s.component_name.to_lowercase(), s.friendly_name.to_lowercase()));
    Ok(out)
}
