use std::time::{Duration, Instant};

const HIDE_DELAY: Duration = Duration::from_secs(5);
const SHOW_DELAY: Duration = Duration::from_secs(3);

/// Backend-owned visibility survives destruction of the scene webview.
pub(super) struct Visibility {
    pub show_local: bool,
    pending: Option<(bool, Instant)>,
}

impl Default for Visibility {
    fn default() -> Self {
        Self {
            show_local: true,
            pending: None,
        }
    }
}

impl Visibility {
    pub fn window_open(&self, has_friends: bool) -> bool {
        self.show_local || has_friends
    }

    pub fn update(&mut self, hide_when_alone: bool, has_friends: bool, now: Instant) {
        if !hide_when_alone {
            self.show_local = true;
            self.pending = None;
            return;
        }
        if self.show_local == has_friends {
            self.pending = None;
            return;
        }
        let (target, deadline) = *self.pending.get_or_insert_with(|| {
            (
                has_friends,
                now + if has_friends { SHOW_DELAY } else { HIDE_DELAY },
            )
        });
        if now >= deadline {
            self.show_local = target;
            self.pending = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_puppet_hides_after_five_seconds_and_return_delays_only_local_puppet() {
        let now = Instant::now();
        let mut visibility = Visibility::default();
        visibility.update(true, false, now);
        visibility.update(true, false, now + HIDE_DELAY - Duration::from_millis(1));
        assert!(visibility.show_local);
        visibility.update(true, false, now + HIDE_DELAY);
        assert!(!visibility.show_local);
        assert!(!visibility.window_open(false));
        let returned = now + HIDE_DELAY;
        visibility.update(true, true, returned);
        assert!(!visibility.show_local);
        assert!(visibility.window_open(true));
        visibility.update(true, true, returned + SHOW_DELAY);
        assert!(visibility.show_local);
    }

    #[test]
    fn reconnect_cancels_hide_and_disconnect_cancels_show() {
        let now = Instant::now();
        let mut visibility = Visibility::default();
        visibility.update(true, false, now);
        visibility.update(true, true, now + Duration::from_secs(4));
        visibility.update(true, false, now + Duration::from_secs(5));
        assert!(visibility.show_local);
        visibility.update(true, false, now + Duration::from_secs(10));
        assert!(!visibility.show_local);
        visibility.update(true, true, now + Duration::from_secs(11));
        visibility.update(true, false, now + Duration::from_secs(12));
        visibility.update(true, true, now + Duration::from_secs(14));
        assert!(!visibility.show_local);
        visibility.update(true, true, now + Duration::from_secs(17));
        assert!(visibility.show_local);
    }

    #[test]
    fn disabling_setting_restores_local_and_cancels_pending_timer() {
        let now = Instant::now();
        let mut visibility = Visibility::default();
        visibility.update(true, false, now);
        visibility.update(true, false, now + HIDE_DELAY);
        visibility.update(false, false, now + HIDE_DELAY);
        assert!(visibility.show_local);
        visibility.update(true, false, now + HIDE_DELAY);
        visibility.update(false, false, now + HIDE_DELAY + Duration::from_secs(1));
        let restarted = now + HIDE_DELAY + Duration::from_secs(2);
        visibility.update(true, false, restarted);
        visibility.update(
            true,
            false,
            restarted + HIDE_DELAY - Duration::from_millis(1),
        );
        assert!(visibility.show_local);
        visibility.update(true, false, restarted + HIDE_DELAY);
        assert!(!visibility.show_local);
    }
}
