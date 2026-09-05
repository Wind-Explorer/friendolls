use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};

#[derive(Default)]
struct ApplicationState {
    started: AtomicBool,
}

pub async fn init(handle: &AppHandle) -> Result<(), String> {
    handle.manage(ApplicationState::default());
    let settings = crate::settings::get(&handle.state())
        .await
        .map_err(crate::db::command_error)?;

    crate::ui::splashscreen::close(handle).await?;

    if settings.onboarding_done {
        start(handle);
    } else {
        crate::ui::onboarding::show_initial(handle).await?;
    }
    Ok(())
}

pub fn start(handle: &AppHandle) {
    let state = handle.state::<ApplicationState>();
    if state
        .started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return;
    }

    if let Err(error) = crate::cursor::start_tracking(handle) {
        eprintln!("failed to initialize cursor tracking: {error}");
    }
    crate::ui::init(handle);
}

pub fn is_started(handle: &AppHandle) -> bool {
    handle
        .state::<ApplicationState>()
        .started
        .load(Ordering::Acquire)
}
