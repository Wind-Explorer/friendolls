use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};

#[derive(Default)]
struct ApplicationState {
    started: AtomicBool,
}

pub async fn init(handle: &AppHandle) -> Result<(), String> {
    handle.manage(ApplicationState::default());
    let accessibility_permission_granted = crate::macos::init(handle).await?;
    let settings = crate::settings::get(&handle.state())
        .await
        .map_err(crate::db::command_error)?;

    crate::ui::splashscreen::close(handle).await?;

    if settings.onboarding_done {
        start(handle).await;
        if cfg!(target_os = "macos") && !accessibility_permission_granted {
            crate::ui::onboarding::show_accessibility_page(handle).await?;
        }
    } else {
        crate::ui::onboarding::show_initial(handle).await?;
    }
    Ok(())
}

pub async fn start(handle: &AppHandle) {
    let state = handle.state::<ApplicationState>();
    if state
        .started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    crate::ui::init(handle);
    reconcile_cursor(handle).await;
}

pub fn is_started(handle: &AppHandle) -> bool {
    handle
        .state::<ApplicationState>()
        .started
        .load(Ordering::Acquire)
}

pub async fn reconcile_cursor(handle: &AppHandle) {
    if !is_started(handle) || !crate::macos::accessibility_permission_granted(handle) {
        return;
    }

    if let Err(error) = crate::cursor::start_tracking(handle).await {
        eprintln!("failed to initialize cursor tracking: {error}");
    }
}
