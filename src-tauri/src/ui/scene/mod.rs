use std::sync::RwLock;

use serde::Deserialize;
use specta::Type;
use tauri::{AppHandle, Manager, State, WebviewUrl};

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SceneHitbox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl SceneHitbox {
    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[derive(Default)]
pub struct SceneHitboxes(RwLock<Vec<SceneHitbox>>);

impl SceneHitboxes {
    fn contains(&self, x: f64, y: f64) -> Result<bool, String> {
        let hitboxes = self.0.read().map_err(|error| error.to_string())?;
        Ok(hitboxes.iter().any(|hitbox| hitbox.contains(x, y)))
    }
}

#[tauri::command]
#[specta::specta]
pub fn update_scene_hitboxes(
    hitboxes: Vec<SceneHitbox>,
    state: State<'_, SceneHitboxes>,
) -> Result<(), String> {
    *state.0.write().map_err(|error| error.to_string())? = hitboxes;
    Ok(())
}

pub fn apply_macos_decoration_window_policy(app_handle: &AppHandle, window_label: String) {
    #[cfg(target_os = "macos")]
    {
        let app_handle_for_closure = app_handle.clone();

        if let Err(e) = app_handle.run_on_main_thread(move || {
            let Some(window) = app_handle_for_closure.get_window(&window_label) else {
                println!(
                    "Failed to apply macOS scene hardening: window '{}' not found",
                    window_label
                );
                return;
            };

            fn native(window: &tauri::Window) -> Result<(), tauri::Error> {
                let ns_window = unsafe { &*window.ns_window()?.cast::<objc2_app_kit::NSWindow>() };
                let behavior = objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
                    | objc2_app_kit::NSWindowCollectionBehavior::Stationary
                    | objc2_app_kit::NSWindowCollectionBehavior::IgnoresCycle;

                ns_window.setLevel(objc2_app_kit::NSStatusWindowLevel);
                ns_window.setCollectionBehavior(behavior);

                Ok(())
            }

            if let Err(e) = native(&window) {
                println!("Failed to apply macOS scene hardening policy: {}", e);
            }
        }) {
            println!("Failed to schedule macOS scene hardening policy: {}", e);
        }
    }

    #[cfg(not(target_os = "macos"))]
    {}
}

pub const WINDOW_LABEL: &str = "scene";

fn track_scene_hitboxes(app_handle: AppHandle, window: tauri::WebviewWindow) {
    tauri::async_runtime::spawn(async move {
        let mut ignores_cursor = true;
        let mut interval = tokio::time::interval(crate::cursor::SYSTEM_CURSOR_POLL_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            let cursor = match app_handle
                .state::<crate::cursor::CursorPositionProvider>()
                .latest()
            {
                Ok(Some(cursor)) => cursor,
                Ok(None) => continue,
                Err(error) => {
                    eprintln!(
                        "Failed to read shared cursor position for scene hit-testing: {error}"
                    );
                    continue;
                }
            };
            let window_position = match window.outer_position() {
                Ok(position) => position,
                Err(error) => {
                    eprintln!("Failed to read scene position for hit-testing: {error}");
                    continue;
                }
            };
            let scale_factor = match window.scale_factor() {
                Ok(scale_factor) => scale_factor,
                Err(error) => {
                    eprintln!("Failed to read scene scale factor for hit-testing: {error}");
                    continue;
                }
            };
            let x = (cursor.x - window_position.x as f64) / scale_factor;
            let y = (cursor.y - window_position.y as f64) / scale_factor;
            let should_ignore = match app_handle.state::<SceneHitboxes>().contains(x, y) {
                Ok(contains_cursor) => !contains_cursor,
                Err(error) => {
                    eprintln!("Failed to read scene hitboxes: {error}");
                    continue;
                }
            };

            if should_ignore != ignores_cursor {
                match window.set_ignore_cursor_events(should_ignore) {
                    Ok(()) => ignores_cursor = should_ignore,
                    Err(error) => {
                        eprintln!("Failed to update scene cursor event policy: {error}")
                    }
                }
            }
        }
    });
}

pub fn overlay_fullscreen(
    app_handle: &AppHandle,
    window: &tauri::WebviewWindow,
) -> Result<(), tauri::Error> {
    let monitor = app_handle.primary_monitor()?.unwrap();
    let monitor_size = monitor.size();

    // Fullscreen the window by expanding the window to match monitor size then move it to the top-left corner
    // This forces the window to fit under the notch that exists on MacBooks with a notch
    window.set_size(tauri::PhysicalSize {
        width: monitor_size.width,
        height: monitor_size.height,
    })?;

    window.set_position(tauri::PhysicalPosition { x: 0, y: 0 })?;

    Ok(())
}

pub fn open_window(app_handle: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app_handle.get_webview_window(WINDOW_LABEL) {
        window.show().map_err(|e| e.to_string())?;
        return Ok(window);
    };

    let builder = tauri::WebviewWindowBuilder::new(
        app_handle,
        WINDOW_LABEL,
        WebviewUrl::App("/scene".into()),
    )
    .title(crate::settings::text(
        app_handle,
        crate::settings::NativeText::SceneTitle,
    ))
    .inner_size(800.0, 600.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .accept_first_mouse(true)
    .visible(true)
    .focused(false);

    let window = builder.build().map_err(|e: tauri::Error| e.to_string())?;

    #[cfg(debug_assertions)]
    window.open_devtools();

    Ok(window)
}

pub fn init(app_handle: &AppHandle) {
    app_handle.manage(SceneHitboxes::default());
    let window = open_window(app_handle).expect("Scene window should be opened successfully");
    overlay_fullscreen(app_handle, &window).expect("Scene window should be fullscreened");
    apply_macos_decoration_window_policy(app_handle, WINDOW_LABEL.to_string());
    window
        .set_ignore_cursor_events(true)
        .expect("Scene window needs to ignore cursor events");
    track_scene_hitboxes(app_handle.clone(), window);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_hitbox_includes_edges_and_excludes_outside_points() {
        let hitbox = SceneHitbox {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        };

        assert!(hitbox.contains(10.0, 20.0));
        assert!(hitbox.contains(40.0, 60.0));
        assert!(!hitbox.contains(9.9, 20.0));
        assert!(!hitbox.contains(40.1, 60.0));
    }
}
