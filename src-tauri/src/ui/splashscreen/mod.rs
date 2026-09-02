use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, WebviewUrl};

const WINDOW_LABEL: &str = "splashscreen";
const MINIMUM_DISPLAY_DURATION: Duration = Duration::from_secs(2);

struct SplashscreenState {
    opened_at: Instant,
}

pub fn open(handle: &AppHandle) -> Result<(), String> {
    tauri::WebviewWindowBuilder::new(handle, WINDOW_LABEL, WebviewUrl::App("/splash.html".into()))
        .title("Friendolls")
        .inner_size(500.0, 290.0)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .center()
        .build()
        .map_err(|error| error.to_string())?;

    handle.manage(SplashscreenState {
        opened_at: Instant::now(),
    });
    Ok(())
}

pub async fn close(handle: &AppHandle) -> Result<(), String> {
    let opened_at = handle.state::<SplashscreenState>().opened_at;
    tokio::time::sleep(MINIMUM_DISPLAY_DURATION.saturating_sub(opened_at.elapsed())).await;

    if let Some(window) = handle.get_webview_window(WINDOW_LABEL) {
        window.close().map_err(|error| error.to_string())?;
    }

    Ok(())
}
