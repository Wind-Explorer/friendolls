use tauri::{AppHandle, Manager, WebviewUrl};

const WINDOW_LABEL: &str = "control-panel";
const ACTION_WINDOW_PREFIX: &str = "control-panel-action-";

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

#[tauri::command]
#[specta::specta]
pub fn open_action_window(
    handle: AppHandle,
    name: String,
    title: String,
    page_url: String,
) -> Result<(), String> {
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err("Action window name must contain only letters, numbers, or hyphens.".into());
    }
    if title.trim().is_empty() || title.chars().count() > 80 {
        return Err("Action window title must contain between 1 and 80 characters.".into());
    }
    if !(page_url.starts_with("/control-panel/add/")
        || page_url.starts_with("/control-panel/edit/"))
        || page_url.contains("..")
        || page_url.contains(['?', '#'])
    {
        return Err("Action windows can only open control-panel add or edit pages.".into());
    }

    let label = format!("{ACTION_WINDOW_PREFIX}{name}");
    if let Some(window) = handle.get_webview_window(&label) {
        window.show().map_err(|error| error.to_string())?;
        window.center().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }

    tauri::WebviewWindowBuilder::new(&handle, label, WebviewUrl::App(page_url.into()))
        .title(title)
        .inner_size(300.0, 450.0)
        .min_inner_size(300.0, 450.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .skip_taskbar(true)
        .center()
        .build()
        .map(|_| ())
        .map_err(|error| error.to_string())
}
