//! Temporary suppression, independent of whether puppet policy wants a scene.
//! The scene reconciliation loop owns this monitor, so no detached worker or
//! native observer survives its lifecycle. A future Windows backend implements
//! the same `is_active() -> io::Result<bool>` contract.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(not(target_os = "macos"))]
mod platform {
    pub(super) const ENABLED: bool = false;

    pub(super) fn is_active() -> std::io::Result<bool> {
        Ok(false)
    }
}

use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct Monitor {
    next_sample: Option<Instant>,
    visibility: Suppression,
    failed: bool,
}

impl Monitor {
    pub(super) async fn suppressed(&mut self) -> bool {
        if !platform::ENABLED {
            return false;
        }
        let now = Instant::now();
        if self.next_sample.is_none_or(|deadline| now >= deadline) {
            // Native process inspection stays off the async executor. Only
            // one bounded scan is in flight under the scene's mutex.
            let result = tauri::async_runtime::spawn_blocking(platform::is_active).await;
            let active = match result {
                Ok(Ok(active)) => {
                    self.failed = false;
                    active
                }
                error => {
                    if !self.failed {
                        eprintln!("Screenshot picker detection failed: {error:?}");
                    }
                    self.failed = true;
                    // Fail open rather than leaving the scene hidden forever.
                    false
                }
            };
            let now = Instant::now();
            self.next_sample = Some(now + Duration::from_millis(500));
            self.visibility.observe(active, now);
        }
        self.visibility.active
    }
}

#[derive(Default)]
struct Suppression {
    active: bool,
    clear_since: Option<Instant>,
}

impl Suppression {
    fn observe(&mut self, active: bool, now: Instant) {
        if active {
            self.active = true;
            self.clear_since = None;
        } else if self.active {
            let since = *self.clear_since.get_or_insert(now);
            // Require another clear scan before restoring; a replacement session
            // during dismissal cancels restoration without flashing the scene.
            if now.duration_since(since) >= Duration::from_millis(100) {
                self.active = false;
                self.clear_since = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn restoration_requires_confirmed_clear_and_reopening_cancels_it() {
        let mut state = Suppression::default();
        let now = Instant::now();
        state.observe(true, now);
        state.observe(false, now);
        assert!(state.active);
        state.observe(true, now + Duration::from_millis(200));
        state.observe(false, now + Duration::from_millis(400));
        assert!(state.active);
        state.observe(false, now + Duration::from_millis(600));
        assert!(!state.active);
    }
}
