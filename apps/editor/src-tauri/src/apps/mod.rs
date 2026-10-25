//! `list_apps`: installed applications with resolved icons, cached for the lifetime of the editor.

mod desktop_entry;
mod icon_theme;

use crate::error::{CmdResult, blocking};
use desktop_entry::Session;
use icon_theme::IconLookup;
use serde::Serialize;
use std::sync::{Arc, Mutex};

/// Mirrors `AppEntry` in `src/lib/backend.ts`; `iconUrl` is filled in by the webview (asset protocol).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    id: String,
    name: String,
    description: String,
    icon_path: Option<String>,
    categories: Vec<String>,
    wm_class: Option<String>,
}

#[derive(Default)]
pub struct AppsCache(Mutex<Option<Arc<Vec<AppEntry>>>>);

#[tauri::command]
pub async fn list_apps(cache: tauri::State<'_, AppsCache>, refresh: Option<bool>) -> CmdResult<Arc<Vec<AppEntry>>> {
    if !refresh.unwrap_or(false)
        && let Some(apps) = lock(&cache).clone()
    {
        return Ok(apps);
    }
    let apps = Arc::new(blocking(|| Ok(discover())).await?);
    *lock(&cache) = Some(apps.clone());
    Ok(apps)
}

fn lock(cache: &AppsCache) -> std::sync::MutexGuard<'_, Option<Arc<Vec<AppEntry>>>> {
    cache.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn discover() -> Vec<AppEntry> {
    let icons = IconLookup::new();
    let mut apps: Vec<AppEntry> = desktop_entry::scan(&Session::from_env())
        .into_iter()
        .map(|a| AppEntry {
            icon_path: a.icon.as_deref().and_then(|i| icons.find(i)).map(|p| p.display().to_string()),
            description: a.generic_name.or(a.comment).unwrap_or_default(),
            id: a.id,
            name: a.name,
            categories: a.categories,
            wm_class: a.wm_class,
        })
        .collect();
    apps.sort_by_cached_key(|a| (a.name.to_lowercase(), a.id.clone()));
    apps
}
