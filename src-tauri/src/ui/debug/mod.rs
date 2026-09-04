use tauri::{AppHandle, Manager, WebviewUrl};

const WINDOW_LABEL: &str = "debug";

#[allow(dead_code)]
pub fn init(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window(WINDOW_LABEL) {
        if !window.is_visible().unwrap() {
            window.center().unwrap();
            window.show().unwrap();
        };
        window.set_focus().unwrap();
        return;
    };

    let builder =
        tauri::WebviewWindowBuilder::new(handle, WINDOW_LABEL, WebviewUrl::App("/".into()))
            .title("Friendolls Debug")
            .inner_size(800.0, 600.0)
            .min_inner_size(500.0, 400.0)
            .transparent(true)
            .visible(true);

    builder.build().unwrap();
}
