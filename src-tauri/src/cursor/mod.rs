use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::watch;

pub(crate) const SYSTEM_CURSOR_POLL_INTERVAL: Duration = Duration::from_millis(250);
const CURSOR_BROADCAST_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct CursorPositions {
    /// Absolute cursor coordinates in physical pixels.
    pub raw: CursorPosition,
    /// Cursor coordinates normalized to the source monitor for scale-independent projection.
    pub mapped: CursorPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct CursorPositionChanged {
    pub positions: HashMap<String, CursorPositions>,
}

#[derive(Default)]
pub(crate) struct CursorState(RwLock<HashMap<String, CursorPositions>>);

impl CursorState {
    fn update(
        &self,
        user_id: String,
        positions: CursorPositions,
    ) -> Result<HashMap<String, CursorPositions>, String> {
        let mut positions_by_user = self.0.write().map_err(|error| error.to_string())?;
        positions_by_user.insert(user_id, positions);
        Ok(positions_by_user.clone())
    }

    fn remove(
        &self,
        user_ids: &[String],
    ) -> Result<Option<HashMap<String, CursorPositions>>, String> {
        let mut positions_by_user = self.0.write().map_err(|error| error.to_string())?;
        let previous_len = positions_by_user.len();
        positions_by_user.retain(|user_id, _| !user_ids.contains(user_id));
        Ok((positions_by_user.len() != previous_len).then(|| positions_by_user.clone()))
    }

    pub(crate) fn snapshot(&self) -> Result<HashMap<String, CursorPositions>, String> {
        self.0
            .read()
            .map(|positions| positions.clone())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn get(&self, user_id: &str) -> Result<Option<CursorPositions>, String> {
        self.0
            .read()
            .map(|positions| positions.get(user_id).cloned())
            .map_err(|error| error.to_string())
    }
}

fn read_system_cursor_position(handle: &AppHandle) -> Result<CursorPosition, String> {
    handle
        .cursor_position()
        .map(|position| CursorPosition {
            x: position.x,
            y: position.y,
        })
        .map_err(|error| error.to_string())
}

fn positions_from_raw(raw: CursorPosition, monitor: &tauri::Monitor) -> CursorPositions {
    CursorPositions {
        mapped: transform_cursor_pos(&raw, true, monitor),
        raw,
    }
}

struct CursorTask {
    stop_tx: watch::Sender<bool>,
    task: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Default)]
pub(crate) struct CursorPositionProvider {
    latest: RwLock<Option<CursorPosition>>,
    tracker: Mutex<Option<CursorTask>>,
}

impl CursorPositionProvider {
    pub(crate) fn latest(&self) -> Result<Option<CursorPosition>, String> {
        self.latest
            .read()
            .map(|position| position.clone())
            .map_err(|error| error.to_string())
    }

    fn update(&self, position: CursorPosition) -> Result<(), String> {
        let mut latest = self.latest.write().map_err(|error| error.to_string())?;
        if latest.as_ref() != Some(&position) {
            *latest = Some(position);
        }
        Ok(())
    }
}

pub fn init(app: &AppHandle) {
    app.manage(CursorState::default());
    app.manage(CursorPositionProvider::default());
}

/// Starts the shared cursor provider and change-based cursor broadcasting.
pub fn start_tracking(app: &AppHandle) -> Result<(), String> {
    let primary_monitor = app
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Failed to resolve primary monitor".to_owned())?;

    let provider = app.state::<CursorPositionProvider>();
    let mut tracker = provider
        .tracker
        .lock()
        .map_err(|error| format!("Failed to lock cursor tracker state: {error}"))?;
    if tracker.is_some() {
        return Ok(());
    }

    println!("Initializing cursor tracking...");
    let initial_position = read_system_cursor_position(app)
        .map_err(|error| format!("Failed to resolve current cursor position: {error}"))?;
    provider.update(initial_position.clone())?;
    update_cursor_position(app, positions_from_raw(initial_position, &primary_monitor));

    let (stop_tx, stop_rx) = watch::channel(false);
    let handle = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        if let Err(error) = track_cursor(stop_rx, primary_monitor, handle).await {
            eprintln!("Cursor tracking stopped with an error: {error}");
        }
    });

    *tracker = Some(CursorTask { stop_tx, task });

    println!("EVENT: Cursor Tracker Enabled");
    Ok(())
}

#[inline]
fn update_cursor_position(handle: &AppHandle, positions: CursorPositions) {
    let user_id = handle
        .state::<crate::keypair::AppKeypair>()
        .public_key()
        .to_owned();
    emit_position(handle, user_id, positions.clone());

    handle
        .state::<crate::network::Network>()
        .send_live_data(crate::live_data::LiveData::Cursor { positions });
}

pub(crate) fn emit_position(handle: &AppHandle, user_id: String, positions: CursorPositions) {
    let positions = match handle.state::<CursorState>().update(user_id, positions) {
        Ok(positions) => positions,
        Err(error) => {
            eprintln!("Failed to update cursor positions: {error}");
            return;
        }
    };

    if let Err(error) = (CursorPositionChanged { positions }).emit(handle) {
        eprintln!("Failed to emit cursor position change: {error}");
    }
}

pub(crate) fn remove_positions(handle: &AppHandle, user_ids: &[String]) {
    if user_ids.is_empty() {
        return;
    }
    let positions = match handle.state::<CursorState>().remove(user_ids) {
        Ok(Some(positions)) => positions,
        Ok(None) => return,
        Err(error) => {
            eprintln!("Failed to remove offline cursor positions: {error}");
            return;
        }
    };
    if let Err(error) = (CursorPositionChanged { positions }).emit(handle) {
        eprintln!("Failed to emit cursor position removal: {error}");
    }
}

/// Convert absolute to normalized coordinates (0.12, 0.78), or normalized to absolute (1234, 567)
pub fn transform_cursor_pos(
    pos: &CursorPosition,
    to_normalized: bool,
    monitor: &tauri::Monitor,
) -> CursorPosition {
    transform_coords(
        pos,
        to_normalized,
        monitor.size().width as f64,
        monitor.size().height as f64,
    )
}

/// Core coordinate transformation, extracted for testability.
/// `w` and `h` are the monitor dimensions in pixels.
fn transform_coords(pos: &CursorPosition, to_normalized: bool, w: f64, h: f64) -> CursorPosition {
    if to_normalized {
        CursorPosition {
            x: (pos.x / w).clamp(0.0, 1.0),
            y: (pos.y / h).clamp(0.0, 1.0),
        }
    } else {
        CursorPosition {
            x: (pos.x * w).round(),
            y: (pos.y * h).round(),
        }
    }
}

/// Stop cursor tracking and unregister all listeners.
#[allow(dead_code)] // TODO: get rid of this when app teardown sequence is introduced.
pub async fn stop_cursor_tracking(app: &AppHandle) {
    println!("stop_cursor_tracking called");

    let tracker = match app.state::<CursorPositionProvider>().tracker.lock() {
        Ok(mut tracker) => tracker.take(),
        Err(e) => {
            println!("Failed to lock cursor tracker state: {}", e);
            return;
        }
    };

    let tracker = match tracker {
        Some(tracker) => tracker,
        None => {
            println!("Cursor tracking is not running");
            return;
        }
    };

    if let Err(e) = tracker.stop_tx.send(true) {
        println!("Failed to signal cursor tracking stop: {}", e);
    }

    if let Err(e) = tracker.task.await {
        println!("Cursor tracking task join failed: {}", e);
    }

    println!("EVENT: Cursor Tracker Disabled");
}

async fn track_cursor(
    mut stop_rx: watch::Receiver<bool>,
    monitor: tauri::Monitor,
    handle: AppHandle,
) -> Result<(), String> {
    let provider = handle.state::<CursorPositionProvider>();
    let mut last_broadcast_position = provider.latest()?;
    let mut poll_interval = tokio::time::interval(SYSTEM_CURSOR_POLL_INTERVAL);
    let mut broadcast_interval = tokio::time::interval(CURSOR_BROADCAST_INTERVAL);
    poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    broadcast_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // The initial position was already sampled and broadcast by start_tracking.
    poll_interval.tick().await;
    broadcast_interval.tick().await;

    loop {
        tokio::select! {
            _ = poll_interval.tick() => {
                match read_system_cursor_position(&handle) {
                    Ok(position) => provider.update(position)?,
                    Err(error) => eprintln!("Failed to read system cursor position: {error}"),
                }
            }
            _ = broadcast_interval.tick() => {
                let latest = provider.latest()?;
                if latest != last_broadcast_position {
                    if let Some(position) = latest.as_ref() {
                        update_cursor_position(
                            &handle,
                            positions_from_raw(position.clone(), &monitor),
                        );
                    }
                    last_broadcast_position = latest;
                }
            }
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    println!("Stopping cursor tracking");
                    break;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn positions(x: f64) -> CursorPositions {
        CursorPositions {
            raw: CursorPosition { x, y: x },
            mapped: CursorPosition { x, y: x },
        }
    }

    #[test]
    fn cursor_state_tracks_and_replaces_positions_by_user_id() {
        let state = CursorState::default();

        let snapshot = state.update("local".to_owned(), positions(1.0)).unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot["local"].raw.x, 1.0);

        let snapshot = state.update("friend".to_owned(), positions(2.0)).unwrap();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot["friend"].raw.x, 2.0);

        let snapshot = state.update("local".to_owned(), positions(3.0)).unwrap();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot["local"].raw.x, 3.0);

        let snapshot = state
            .remove(&["friend".to_owned()])
            .unwrap()
            .expect("friend cursor removed");
        assert_eq!(snapshot.keys().collect::<Vec<_>>(), ["local"]);
        assert!(state.remove(&["missing".to_owned()]).unwrap().is_none());
    }

    #[test]
    fn physical_cursor_positions_normalize_across_display_scale_factors() {
        let one_x = transform_coords(&CursorPosition { x: 720.0, y: 450.0 }, true, 1440.0, 900.0);
        let two_x = transform_coords(
            &CursorPosition {
                x: 1440.0,
                y: 900.0,
            },
            true,
            2880.0,
            1800.0,
        );

        assert_eq!(one_x.x, 0.5);
        assert_eq!(one_x.y, 0.5);
        assert_eq!(two_x.x, one_x.x);
        assert_eq!(two_x.y, one_x.y);
    }

    #[test]
    fn cursor_provider_keeps_the_latest_system_position() {
        let provider = CursorPositionProvider::default();
        assert_eq!(provider.latest().unwrap(), None);

        provider
            .update(CursorPosition { x: 120.0, y: 80.0 })
            .unwrap();
        assert_eq!(
            provider.latest().unwrap(),
            Some(CursorPosition { x: 120.0, y: 80.0 })
        );

        provider
            .update(CursorPosition { x: 240.0, y: 160.0 })
            .unwrap();
        assert_eq!(
            provider.latest().unwrap(),
            Some(CursorPosition { x: 240.0, y: 160.0 })
        );
    }
}
