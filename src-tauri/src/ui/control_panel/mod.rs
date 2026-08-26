use tauri::{AppHandle, Manager, WebviewUrl};

const WINDOW_LABEL: &str = "control-panel";

pub fn init(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window(WINDOW_LABEL) {
        if !window.is_visible().unwrap() {
            window.center().unwrap();
            window.show().unwrap();
        };
        window.set_focus().unwrap();
        return;
    };

    let builder = tauri::WebviewWindowBuilder::new(
        handle,
        WINDOW_LABEL,
        WebviewUrl::App("/control-panel".into()),
    )
    .title("Wyd Control Panel")
    .inner_size(400.0, 600.0)
    .min_inner_size(400.0, 600.0)
    .resizable(false)
    .maximizable(false)
    .visible(true);

    let window = builder.build().unwrap();
    let window_c = window.clone();
    window.on_window_event(move |event: &tauri::WindowEvent| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            window_c.hide().unwrap();
        }
    });
}
