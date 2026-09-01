/*
UFA: User Focused App
*/

use std::collections::HashMap;
use std::sync::RwLock;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
pub use types::*;
mod icon_cache;
#[cfg(target_os = "macos")]
mod macos;
mod types;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundAppChanged {
    pub meta: AppMeta,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct FriendForegroundAppChanged {
    pub friend_id: String,
    pub meta: AppMeta,
}

#[derive(Default)]
pub(crate) struct ForegroundAppState(RwLock<HashMap<String, AppMeta>>);

impl ForegroundAppState {
    fn update(&self, user_id: String, meta: AppMeta) -> Result<(), String> {
        self.0
            .write()
            .map_err(|error| error.to_string())?
            .insert(user_id, meta);
        Ok(())
    }

    pub(crate) fn remove(&self, user_ids: &[String]) -> Result<(), String> {
        let mut apps = self.0.write().map_err(|error| error.to_string())?;
        apps.retain(|user_id, _| !user_ids.contains(user_id));
        Ok(())
    }

    pub(crate) fn snapshot(&self) -> Result<HashMap<String, AppMeta>, String> {
        self.0
            .read()
            .map(|apps| apps.clone())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn get(&self, user_id: &str) -> Result<Option<AppMeta>, String> {
        self.0
            .read()
            .map(|apps| apps.get(user_id).cloned())
            .map_err(|error| error.to_string())
    }
}

fn current_app() -> AppMeta {
    #[cfg(target_os = "macos")]
    let meta = macos::get_active_app_metadata_macos();
    #[cfg(target_os = "windows")]
    let meta = windows::get_active_app_metadata_windows(None);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let meta = AppMeta::default();
    meta
}

/// Listens for changes in the active (foreground) application and calls the provided callback with metadata.
/// The implementation varies by platform: macOS uses NSWorkspace notifications, Windows uses WinEventHook.
pub fn init_listener<F>(callback: F)
where
    F: Fn(AppMeta) + Send + 'static,
{
    listen_impl(callback)
}

#[cfg(target_os = "macos")]
fn listen_impl<F>(callback: F)
where
    F: Fn(AppMeta) + Send + 'static,
{
    macos::listen_for_active_app_changes(callback);
}

#[cfg(target_os = "windows")]
fn listen_impl<F>(callback: F)
where
    F: Fn(AppMeta) + Send + 'static,
{
    windows::listen_for_active_app_changes(callback);
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn listen_impl<F>(_callback: F)
where
    F: Fn(AppMeta) + Send + 'static,
{
    // no-op on unsupported platforms
}

pub fn init(handle: &AppHandle) {
    handle.manage(ForegroundAppState::default());
}

/// Starts the foreground app change listener after network state is available.
pub fn start(handle: &AppHandle) {
    update_local_app(handle, current_app());
    let handle = handle.clone();

    init_listener(move |meta: AppMeta| update_local_app(&handle, meta));
}

pub(crate) fn emit_friend_app(handle: &AppHandle, friend_id: String, meta: AppMeta) {
    if let Err(error) = handle
        .state::<ForegroundAppState>()
        .update(friend_id.clone(), meta.clone())
    {
        eprintln!("Failed to cache friend foreground app: {error}");
        return;
    }
    if let Err(error) = (FriendForegroundAppChanged { friend_id, meta }).emit(handle) {
        eprintln!("Failed to emit friend foreground app change: {error}");
    }
}

fn update_local_app(handle: &AppHandle, meta: AppMeta) {
    let user_id = handle
        .state::<crate::keypair::AppKeypair>()
        .public_key()
        .to_owned();
    if let Err(error) = handle
        .state::<ForegroundAppState>()
        .update(user_id, meta.clone())
    {
        eprintln!("Failed to cache foreground app: {error}");
        return;
    }
    if let Err(error) = (ForegroundAppChanged { meta: meta.clone() }).emit(handle) {
        eprintln!("Failed to emit foreground app change: {error}");
    }
    handle
        .state::<crate::network::Network>()
        .send_live_data(crate::live_data::LiveData::ForegroundApp { meta });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str) -> AppMeta {
        AppMeta {
            local: Some(name.to_owned()),
            unlocal: None,
            ico: None,
        }
    }

    #[test]
    fn foreground_app_state_snapshots_and_removes_users() {
        let state = ForegroundAppState::default();
        state.update("local".to_owned(), app("Local")).unwrap();
        state.update("friend".to_owned(), app("Friend")).unwrap();

        assert_eq!(
            state.get("friend").unwrap().unwrap().local.as_deref(),
            Some("Friend")
        );
        state.remove(&["friend".to_owned()]).unwrap();

        let snapshot = state.snapshot().unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot["local"].local.as_deref(), Some("Local"));
    }
}
