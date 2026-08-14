use tauri::{AppHandle, Manager, WebviewUrl};

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
    .title("Scene")
    .inner_size(800.0, 600.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .visible(true)
    .focused(false);

    let window = builder.build().map_err(|e: tauri::Error| e.to_string())?;

    #[cfg(debug_assertions)]
    window.open_devtools();

    Ok(window)
}

pub fn init(app_handle: &AppHandle) {
    let window = open_window(app_handle).expect("Scene window should be opened successfully");
    overlay_fullscreen(app_handle, &window).expect("Scene window should be fullscreened");
    apply_macos_decoration_window_policy(app_handle, WINDOW_LABEL.to_string());
    window
        .set_ignore_cursor_events(true)
        .expect("Scene window needs to ignore cursor events");
}
