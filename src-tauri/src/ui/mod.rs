use tauri::AppHandle;

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
    control_panel::init(app_handle);
    tray::init(app_handle);
}
