use tauri::AppHandle;

mod control_panel;
#[cfg(debug_assertions)]
mod debug;
pub mod scene;

pub fn init(app_handle: &AppHandle) {
    #[cfg(debug_assertions)]
    debug::init(app_handle);
    scene::init(app_handle);
    control_panel::init(app_handle);
}
