use tauri::AppHandle;

#[cfg(debug_assertions)]
mod debug;
mod scene;

pub fn init(app_handle: &AppHandle) {
    #[cfg(debug_assertions)]
    debug::init(app_handle);
    scene::init(app_handle);
}
