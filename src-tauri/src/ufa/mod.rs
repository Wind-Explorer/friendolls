/*
UFA: User Focused App
*/

use tauri::AppHandle;
pub use types::*;
mod icon_cache;
#[cfg(target_os = "macos")]
mod macos;
mod types;
#[cfg(target_os = "windows")]
mod windows;

const ACTIVITY_SOURCE: &str = "foreground-app";

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
    update_local_app(handle, current_app());
    let handle = handle.clone();

    init_listener(move |meta: AppMeta| update_local_app(&handle, meta));
}

fn update_local_app(handle: &AppHandle, meta: AppMeta) {
    let details = meta
        .local
        .as_ref()
        .zip(meta.unlocal.as_ref())
        .filter(|(local, unlocal)| local != unlocal)
        .map(|(_, unlocal)| unlocal.clone());
    let name = meta.local.or(meta.unlocal);
    crate::activity::update_local(
        handle,
        ACTIVITY_SOURCE,
        Some(crate::activity::Activity {
            kind: crate::activity::ActivityKind::Application,
            name,
            details,
            icon: meta.ico,
        }),
    );
}
