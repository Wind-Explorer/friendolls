use std::collections::HashSet;

use tauri::{AppHandle, Manager, RunEvent, Window, WindowEvent, Wry, plugin::Plugin};

/// Normal windows live from opening until dismissal. Minimized windows still count.
#[derive(Default)]
pub struct ActivationPolicy {
    windows: HashSet<String>,
    applied_regular: Option<bool>,
}

impl ActivationPolicy {
    fn record_window(&mut self, label: &str, created: bool) {
        if matches!(label, "scene" | "splashscreen") {
            return;
        }
        if created {
            self.windows.insert(label.to_owned());
        } else {
            self.windows.remove(label);
        }
    }

    fn reconcile(&mut self, app: &AppHandle) -> tauri::Result<()> {
        let regular = !self.windows.is_empty();
        if self.applied_regular != Some(regular) {
            app.set_activation_policy(if regular {
                tauri::ActivationPolicy::Regular
            } else {
                tauri::ActivationPolicy::Accessory
            })?;
            self.applied_regular = Some(regular);
        }
        Ok(())
    }
}

impl Plugin<Wry> for ActivationPolicy {
    fn name(&self) -> &'static str {
        "activation-policy"
    }

    fn initialize(
        &mut self,
        app: &AppHandle,
        _config: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.reconcile(app)?;
        Ok(())
    }

    fn window_created(&mut self, window: Window) {
        self.record_window(window.label(), true);
        if let Err(error) = self.reconcile(window.app_handle()) {
            eprintln!("failed to update macOS activation policy: {error}");
        }
    }

    fn on_event(&mut self, app: &AppHandle, event: &RunEvent) {
        if let RunEvent::WindowEvent {
            label,
            event: WindowEvent::Destroyed,
            ..
        } = event
        {
            self.record_window(label, false);
            if let Err(error) = self.reconcile(app) {
                eprintln!("failed to update macOS activation policy: {error}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ActivationPolicy;

    #[test]
    fn decoration_windows_do_not_keep_the_app_in_the_dock() {
        let mut policy = ActivationPolicy::default();
        policy.record_window("scene", true);
        policy.record_window("splashscreen", true);
        policy.record_window("onboarding", true);
        policy.record_window("splashscreen", false);
        assert_eq!(policy.windows.len(), 1);
        policy.record_window("onboarding", false);
        assert!(policy.windows.is_empty());
    }

    #[test]
    fn action_windows_keep_the_app_in_the_dock_after_the_panel_closes() {
        let mut policy = ActivationPolicy::default();
        policy.record_window("control-panel", true);
        policy.record_window("control-panel", true);
        policy.record_window("control-panel-action-add-friend", true);
        policy.record_window("control-panel", false);
        policy.record_window("control-panel", false);
        assert_eq!(policy.windows.len(), 1);
        policy.record_window("control-panel-action-add-friend", false);
        assert!(policy.windows.is_empty());
        policy.record_window("control-panel", true);
        assert_eq!(policy.windows.len(), 1);
    }
}
