use tauri::{AppHandle, Manager, WebviewUrl};

const WINDOW_LABEL: &str = "onboarding";

pub async fn show_initial(handle: &AppHandle) -> Result<(), String> {
    if let Some(window) = handle.get_webview_window(WINDOW_LABEL) {
        window.unminimize().map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        return window.set_focus().map_err(|error| error.to_string());
    }

    let window = tauri::WebviewWindowBuilder::new(
        handle,
        WINDOW_LABEL,
        WebviewUrl::App("/onboarding".into()),
    )
    .title(crate::settings::text(
        handle,
        crate::settings::NativeText::OnboardingTitle,
    ))
    .inner_size(680.0, 520.0)
    .min_inner_size(680.0, 520.0)
    .transparent(true)
    .resizable(false)
    .maximizable(false)
    .center()
    .build()
    .map_err(|error| error.to_string())?;

    let handle = handle.clone();
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::CloseRequested { .. })
            && !crate::application::is_started(&handle)
        {
            handle.exit(0);
        }
    });
    Ok(())
}
