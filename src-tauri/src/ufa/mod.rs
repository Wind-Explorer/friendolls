/*
UFA: User Focused App
*/

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

#[derive(Default)]
pub struct ForegroundAppState(RwLock<AppMeta>);

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

/// Initializes the foreground app change listener
/// and emits events to the Tauri app on changes.
/// Used for app to emit user foreground app to peers.
pub fn init(handle: &AppHandle) {
    let handle = handle.clone();

    init_listener(move |meta: AppMeta| {
        let state = handle.state::<ForegroundAppState>();
        let mut current = state.0.write().expect("Foreground App lock failed");
        *current = meta.clone();
        drop(current);

        if let Err(error) = (ForegroundAppChanged { meta }).emit(&handle) {
            eprintln!("Failed to emit foreground app change: {error}");
        }
    });
}
