use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use tauri::{AppHandle, Manager};

pub struct AccessibilityPermissionState(AtomicBool);

impl AccessibilityPermissionState {
    fn new(granted: bool) -> Self {
        Self(AtomicBool::new(granted))
    }

    fn replace(&self, granted: bool) -> bool {
        self.0.swap(granted, Ordering::AcqRel) != granted
    }

    fn granted(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[cfg(target_os = "macos")]
fn system_accessibility_permission_granted() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted()
}

#[cfg(not(target_os = "macos"))]
fn system_accessibility_permission_granted() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn request_system_accessibility_permission() -> bool {
    macos_accessibility_client::accessibility::application_is_trusted_with_prompt()
}

#[cfg(not(target_os = "macos"))]
fn request_system_accessibility_permission() -> bool {
    true
}

pub fn accessibility_permission_granted(handle: &AppHandle) -> bool {
    if !cfg!(target_os = "macos") {
        return true;
    }
    handle.state::<AccessibilityPermissionState>().granted()
}

async fn apply_permission_state(handle: &AppHandle, granted: bool) -> Result<bool, String> {
    if !handle
        .state::<AccessibilityPermissionState>()
        .replace(granted)
    {
        return Ok(false);
    }
    let database = handle.state();
    let status = crate::onboarding::emit_status(handle, &database).await?;
    if status.onboarding_done {
        if granted {
            crate::application::reconcile_cursor(handle).await;
        } else {
            crate::ui::onboarding::show_accessibility_page(handle).await?;
        }
    }
    Ok(true)
}

pub async fn init(handle: &AppHandle) -> Result<bool, String> {
    let granted = system_accessibility_permission_granted();
    handle.manage(AccessibilityPermissionState::new(granted));

    #[cfg(target_os = "macos")]
    {
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            interval.tick().await;

            loop {
                interval.tick().await;
                let granted = system_accessibility_permission_granted();
                if let Err(error) = apply_permission_state(&handle, granted).await {
                    eprintln!("failed to update macOS Accessibility permission: {error}");
                }
            }
        });
    }

    Ok(granted)
}

#[tauri::command]
#[specta::specta]
pub async fn request_accessibility_permission(handle: AppHandle) -> Result<bool, String> {
    let granted = tauri::async_runtime::spawn_blocking(request_system_accessibility_permission)
        .await
        .map_err(|error| error.to_string())?;
    apply_permission_state(&handle, granted).await?;
    Ok(granted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_state_reports_only_actual_transitions() {
        let state = AccessibilityPermissionState::new(false);

        assert!(!state.replace(false));
        assert!(state.replace(true));
        assert!(!state.replace(true));
        assert!(state.replace(false));
    }
}
