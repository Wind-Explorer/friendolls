use device_query::{DeviceEvents, DeviceEventsHandler, DeviceQuery, DeviceState};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::{mpsc, oneshot, watch};

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

fn current_position(handle: &AppHandle) -> Result<CursorPositions, String> {
    let monitor = handle
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or("Primary monitor is unavailable")?;
    let position = DeviceState::checked_new()
        .ok_or_else(|| "System cursor access is unavailable".to_owned())?
        .get_mouse()
        .coords;

    #[cfg(target_os = "windows")]
    let raw = CursorPosition {
        x: position.0 as f64,
        y: position.1 as f64,
    };

    #[cfg(not(target_os = "windows"))]
    let raw = CursorPosition {
        x: position.0 as f64 * monitor.scale_factor(),
        y: position.1 as f64 * monitor.scale_factor(),
    };

    Ok(CursorPositions {
        mapped: transform_cursor_pos(&raw, true, &monitor),
        raw,
    })
}

// Was private, but for some reason LSP
// complains even when there's no external references.
// Possibly because of `lazy_static!`.
// Just leave it public I guess.
struct CursorTask {
    stop_tx: watch::Sender<bool>,
    task: tauri::async_runtime::JoinHandle<()>,
}

enum CursorTracker {
    Starting,
    Running(CursorTask),
}

lazy_static! {
    static ref CURSOR_TRACKER: Mutex<Option<CursorTracker>> = Mutex::new(None);
}

pub fn init(app: &AppHandle) {
    app.manage(CursorState::default());
}

/// Starts cursor tracking after Accessibility permission is available.
pub async fn start_tracking(app: &AppHandle) -> Result<(), String> {
    if !crate::macos::accessibility_permission_granted(app) {
        return Err("macOS Accessibility permission has not been granted".to_owned());
    }

    let primary_monitor = app
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "Failed to resolve primary monitor".to_owned())?;

    {
        let mut tracker = CURSOR_TRACKER
            .lock()
            .map_err(|error| format!("Failed to lock cursor tracker state: {error}"))?;
        if tracker.is_some() {
            return Ok(());
        }
        *tracker = Some(CursorTracker::Starting);
    }

    println!("Initializing cursor tracking...");
    match current_position(app) {
        Ok(positions) => update_cursor_position(app, positions),
        Err(error) => eprintln!("Failed to resolve current cursor position: {error}"),
    }

    let (stop_tx, stop_rx) = watch::channel(false);
    let (ready_tx, ready_rx) = oneshot::channel();

    let handle = app.clone();
    let task = tauri::async_runtime::spawn(async move {
        if let Err(e) = init_cursor_tracking_i(stop_rx, primary_monitor, handle, ready_tx).await {
            println!("Failed to initialize cursor tracking: {}", e);
        }
    });

    let startup = ready_rx
        .await
        .unwrap_or_else(|_| Err("Cursor tracking task stopped during initialization".to_owned()));
    if let Err(error) = startup {
        if let Ok(mut tracker) = CURSOR_TRACKER.lock()
            && matches!(*tracker, Some(CursorTracker::Starting))
        {
            *tracker = None;
        }
        let _ = task.await;
        return Err(error);
    }

    let installed = {
        let mut tracker = CURSOR_TRACKER
            .lock()
            .map_err(|error| format!("Failed to lock cursor tracker state: {error}"))?;
        if matches!(*tracker, Some(CursorTracker::Starting)) {
            *tracker = Some(CursorTracker::Running(CursorTask { stop_tx, task }));
            true
        } else {
            false
        }
    };

    if !installed {
        return Err("Cursor tracking startup was cancelled".to_owned());
    }

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
}

/// Stop cursor tracking and unregister all listeners.
#[allow(dead_code)] // TODO: get rid of this when app teardown sequence is introduced.
pub async fn stop_cursor_tracking() {
    println!("stop_cursor_tracking called");

    let tracker = match CURSOR_TRACKER.lock() {
        Ok(mut tracker) => tracker.take(),
        Err(e) => {
            println!("Failed to lock cursor tracker state: {}", e);
            return;
        }
    };

    let tracker = match tracker {
        Some(CursorTracker::Running(tracker)) => tracker,
        Some(CursorTracker::Starting) => {
            println!("Cursor tracking startup cancelled");
            return;
        }
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

async fn init_cursor_tracking_i(
    mut stop_rx: watch::Receiver<bool>,
    monitor: tauri::Monitor,
    handle: AppHandle,
    ready_tx: oneshot::Sender<Result<(), String>>,
) -> Result<(), String> {
    // Create a channel to decouple event generation (producer) from processing (consumer).
    // Capacity 100 is plenty for 500ms polling (2Hz).
    let (tx, mut rx) = mpsc::channel::<CursorPositions>(100);
    let permission_handle = handle.clone();

    // Spawn the consumer task
    // This task handles WebSocket reporting and local position projection updates.
    // It runs independently of the device event loop.
    tauri::async_runtime::spawn(async move {
        println!("Cursor event consumer started");

        while let Some(positions) = rx.recv().await {
            update_cursor_position(&handle, positions);
        }
        println!("Cursor event consumer stopped (channel closed)");
    });

    let device_state = match DeviceEventsHandler::new(Duration::from_millis(500)) {
        Some(device_state) => device_state,
        None => {
            let error = "Failed to create device event handler (already running?)".to_owned();
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }
    };

    println!("Device event handler created successfully");
    println!("Setting up mouse move handler for event broadcasting...");

    #[cfg(not(target_os = "windows"))]
    let scale_factor = monitor.scale_factor();

    // The producer closure moves `tx` into it.
    // device_query runs this closure on its own thread.
    let _guard = device_state.on_mouse_move(move |position: &(i32, i32)| {
        if !crate::macos::accessibility_permission_granted(&permission_handle) {
            return;
        }

        #[cfg(target_os = "windows")]
        let raw = CursorPosition {
            x: position.0 as f64,
            y: position.1 as f64,
        };

        #[cfg(not(target_os = "windows"))]
        let raw = CursorPosition {
            x: position.0 as f64 * scale_factor,
            y: position.1 as f64 * scale_factor,
        };

        let mapped = transform_cursor_pos(&raw, true, &monitor);

        let positions = CursorPositions { raw, mapped };

        // Send to consumer channel (non-blocking)
        if let Err(e) = tx.try_send(positions) {
            println!("Failed to send cursor position to channel: {:?}", e);
        }
    });

    ready_tx
        .send(Ok(()))
        .map_err(|_| "Cursor tracking startup was cancelled".to_owned())?;
    println!("Mouse move handler registered - now broadcasting cursor events to all windows");

    // Keep the handler alive while tracking is enabled.
    // This loop is necessary to keep `_guard` and `device_state` in scope.
    while stop_rx.changed().await.is_ok() {
        if *stop_rx.borrow() {
            println!("Stopping cursor tracking event handler");
            break;
        }
    }

    Ok(())
}
