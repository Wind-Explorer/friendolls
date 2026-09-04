use tauri::AppHandle;

#[cfg(target_os = "macos")]
pub mod activation_policy;
pub mod control_panel;
#[cfg(debug_assertions)]
mod debug;
pub mod onboarding;
pub mod scene;
pub mod splashscreen;
mod tray;

pub fn init(app_handle: &AppHandle) {
    // #[cfg(debug_assertions)]
    // debug::init(app_handle);
    scene::init(app_handle);
    tray::init(app_handle);
}

pub fn refresh_locale(app_handle: &AppHandle) {
    tray::refresh_locale(app_handle);
}
