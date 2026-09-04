use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::db;

#[tauri::command]
#[specta::specta]
pub fn get_autostart_enabled(handle: AppHandle) -> Result<bool, String> {
    handle.autolaunch().is_enabled().map_err(db::command_error)
}

#[tauri::command]
#[specta::specta]
pub fn set_autostart_enabled(handle: AppHandle, enabled: bool) -> Result<bool, String> {
    let manager = handle.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(db::command_error)?;
    manager.is_enabled().map_err(db::command_error)
}
