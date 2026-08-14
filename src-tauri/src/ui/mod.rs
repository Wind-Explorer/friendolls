use tauri::AppHandle;

mod scene;

pub fn init(app_handle: &AppHandle) {
    scene::init(app_handle);
}
