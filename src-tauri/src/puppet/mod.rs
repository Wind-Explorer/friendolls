use std::{
    collections::HashMap,
    sync::RwLock,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;
use tokio::time::MissedTickBehavior;

use crate::cursor::{CursorPosition, CursorPositions, CursorState};
use crate::scene_configuration::{PuppetMovementMode, SceneConfigurationState};

const TICK_INTERVAL: Duration = Duration::from_millis(125);
const SPEED_LOGICAL_PIXELS_PER_SECOND: f64 = 80.0;
const FOLLOW_RADIUS_LOGICAL_PIXELS: f64 = 48.0;
const BOTTOM_MOVEMENT_Y: f64 = 1.0;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Type)]
#[serde(rename_all = "camelCase")]
pub struct PuppetState {
    pub id: String,
    pub position: CursorPosition,
    pub is_moving: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct PuppetStatesChanged {
    pub puppets: Vec<PuppetState>,
}

#[derive(Default)]
pub struct PuppetStateStore(RwLock<HashMap<String, PuppetState>>);

impl PuppetStateStore {
    fn update(
        &self,
        cursor_positions: &HashMap<String, CursorPositions>,
        viewport: Viewport,
        elapsed: Duration,
        movement_mode: PuppetMovementMode,
    ) -> Result<Option<Vec<PuppetState>>, String> {
        let mut puppets = self.0.write().map_err(|error| error.to_string())?;
        let changed = advance_puppets(
            &mut puppets,
            cursor_positions,
            viewport,
            elapsed,
            movement_mode,
        );
        Ok(changed.then(|| sorted_snapshot(&puppets)))
    }

    fn snapshot(&self) -> Result<Vec<PuppetState>, String> {
        self.0
            .read()
            .map(|puppets| sorted_snapshot(&puppets))
            .map_err(|error| error.to_string())
    }
}

#[derive(Clone, Copy)]
struct Viewport {
    width: f64,
    height: f64,
}

#[tauri::command]
#[specta::specta]
pub fn list_puppet_states(state: State<'_, PuppetStateStore>) -> Result<Vec<PuppetState>, String> {
    state.snapshot()
}

pub fn init(handle: &AppHandle) -> Result<(), String> {
    let monitor = handle
        .primary_monitor()
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "primary monitor is unavailable".to_owned())?;
    let scale_factor = monitor.scale_factor();
    let viewport = Viewport {
        width: monitor.size().width as f64 / scale_factor,
        height: monitor.size().height as f64 / scale_factor,
    };
    let handle = handle.clone();

    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(TICK_INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut previous_tick = Instant::now();

        loop {
            ticker.tick().await;
            let now = Instant::now();
            let elapsed = now.duration_since(previous_tick);
            previous_tick = now;

            let cursor_positions = match handle.state::<CursorState>().snapshot() {
                Ok(positions) => positions,
                Err(error) => {
                    eprintln!("failed to read cursor positions for puppet motion: {error}");
                    continue;
                }
            };
            let movement_mode = match handle
                .state::<SceneConfigurationState>()
                .puppet_movement_mode()
            {
                Ok(mode) => mode,
                Err(error) => {
                    eprintln!("failed to read Puppet movement mode: {error}");
                    continue;
                }
            };
            let puppets = match handle.state::<PuppetStateStore>().update(
                &cursor_positions,
                viewport,
                elapsed,
                movement_mode,
            ) {
                Ok(Some(puppets)) => puppets,
                Ok(None) => continue,
                Err(error) => {
                    eprintln!("failed to update puppet motion: {error}");
                    continue;
                }
            };

            if let Err(error) = emit_changed(&handle, puppets) {
                eprintln!("failed to emit puppet states: {error}");
            }
        }
    });

    Ok(())
}

fn emit_changed(handle: &AppHandle, puppets: Vec<PuppetState>) -> Result<(), String> {
    (PuppetStatesChanged { puppets })
        .emit(handle)
        .map_err(|error| error.to_string())
}

fn advance_puppets(
    puppets: &mut HashMap<String, PuppetState>,
    cursor_positions: &HashMap<String, CursorPositions>,
    viewport: Viewport,
    elapsed: Duration,
    movement_mode: PuppetMovementMode,
) -> bool {
    let previous_len = puppets.len();
    puppets.retain(|id, _| cursor_positions.contains_key(id));
    let mut changed = puppets.len() != previous_len;

    for (id, cursor) in cursor_positions {
        let target = movement_target(&cursor.mapped, movement_mode);
        let Some(puppet) = puppets.get_mut(id) else {
            puppets.insert(
                id.clone(),
                PuppetState {
                    id: id.clone(),
                    position: target,
                    is_moving: false,
                },
            );
            changed = true;
            continue;
        };

        if movement_mode == PuppetMovementMode::Bottom && puppet.position.y != BOTTOM_MOVEMENT_Y {
            puppet.position.y = BOTTOM_MOVEMENT_Y;
            changed = true;
        }
        changed |= advance_puppet(puppet, &target, viewport, elapsed.as_secs_f64());
    }

    changed
}

fn movement_target(cursor: &CursorPosition, mode: PuppetMovementMode) -> CursorPosition {
    CursorPosition {
        x: cursor.x,
        y: match mode {
            PuppetMovementMode::Free => cursor.y,
            PuppetMovementMode::Bottom => BOTTOM_MOVEMENT_Y,
        },
    }
}

fn advance_puppet(
    puppet: &mut PuppetState,
    target: &CursorPosition,
    viewport: Viewport,
    elapsed_seconds: f64,
) -> bool {
    let delta_x = (target.x - puppet.position.x) * viewport.width;
    let delta_y = (target.y - puppet.position.y) * viewport.height;
    let distance = delta_x.hypot(delta_y);
    let available_distance = (distance - FOLLOW_RADIUS_LOGICAL_PIXELS).max(0.0);
    let step = (SPEED_LOGICAL_PIXELS_PER_SECOND * elapsed_seconds).min(available_distance);
    let is_moving = step > 0.0;

    if !is_moving {
        if puppet.is_moving {
            puppet.is_moving = false;
            return true;
        }
        return false;
    }

    puppet.position.x += delta_x / distance * step / viewport.width;
    puppet.position.y += delta_y / distance * step / viewport.height;
    puppet.is_moving = true;
    true
}

fn sorted_snapshot(puppets: &HashMap<String, PuppetState>) -> Vec<PuppetState> {
    let mut snapshot = puppets.values().cloned().collect::<Vec<_>>();
    snapshot.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> Viewport {
        Viewport {
            width: 1_000.0,
            height: 500.0,
        }
    }

    fn cursors(entries: &[(&str, f64, f64)]) -> HashMap<String, CursorPositions> {
        entries
            .iter()
            .map(|(id, x, y)| {
                (
                    (*id).to_owned(),
                    CursorPositions {
                        raw: CursorPosition::default(),
                        mapped: CursorPosition { x: *x, y: *y },
                    },
                )
            })
            .collect()
    }

    #[test]
    fn new_puppet_starts_at_first_cursor_target() {
        let mut puppets = HashMap::new();
        let changed = advance_puppets(
            &mut puppets,
            &cursors(&[("friend", 0.75, 0.25)]),
            viewport(),
            TICK_INTERVAL,
            PuppetMovementMode::Free,
        );

        assert!(changed);
        assert_eq!(
            puppets["friend"].position,
            CursorPosition { x: 0.75, y: 0.25 }
        );
        assert!(!puppets["friend"].is_moving);
    }

    #[test]
    fn movement_is_bounded_by_speed_and_elapsed_time() {
        let mut puppet = PuppetState {
            id: "friend".to_owned(),
            position: CursorPosition { x: 0.0, y: 0.5 },
            is_moving: false,
        };

        assert!(advance_puppet(
            &mut puppet,
            &CursorPosition { x: 1.0, y: 0.5 },
            viewport(),
            0.125,
        ));
        assert_eq!(puppet.position.x, 0.01);
        assert_eq!(puppet.position.y, 0.5);
        assert!(puppet.is_moving);
    }

    #[test]
    fn puppet_stops_at_follow_radius_without_overshooting() {
        let mut puppet = PuppetState {
            id: "friend".to_owned(),
            position: CursorPosition { x: 0.0, y: 0.5 },
            is_moving: true,
        };

        advance_puppet(
            &mut puppet,
            &CursorPosition { x: 0.05, y: 0.5 },
            viewport(),
            0.125,
        );

        assert_eq!(puppet.position.x, 0.002);
        assert!(advance_puppet(
            &mut puppet,
            &CursorPosition { x: 0.05, y: 0.5 },
            viewport(),
            0.125,
        ));
        assert!(!puppet.is_moving);
        assert!(!advance_puppet(
            &mut puppet,
            &CursorPosition { x: 0.05, y: 0.5 },
            viewport(),
            0.125,
        ));
    }

    #[test]
    fn removed_cursor_removes_puppet() {
        let mut puppets = HashMap::from([(
            "friend".to_owned(),
            PuppetState {
                id: "friend".to_owned(),
                position: CursorPosition::default(),
                is_moving: false,
            },
        )]);

        assert!(advance_puppets(
            &mut puppets,
            &HashMap::new(),
            viewport(),
            TICK_INTERVAL,
            PuppetMovementMode::Free,
        ));
        assert!(puppets.is_empty());
    }

    #[test]
    fn bottom_mode_preserves_horizontal_target_and_insets_vertical_target() {
        let cursor = CursorPosition { x: 0.25, y: 0.1 };

        assert_eq!(
            movement_target(&cursor, PuppetMovementMode::Bottom),
            CursorPosition {
                x: 0.25,
                y: BOTTOM_MOVEMENT_Y,
            }
        );
        assert_eq!(movement_target(&cursor, PuppetMovementMode::Free), cursor);
    }

    #[test]
    fn bottom_mode_emits_a_fixed_inset_y_for_existing_puppets() {
        let mut puppets = HashMap::from([(
            "friend".to_owned(),
            PuppetState {
                id: "friend".to_owned(),
                position: CursorPosition { x: 0.25, y: 0.1 },
                is_moving: false,
            },
        )]);

        assert!(advance_puppets(
            &mut puppets,
            &cursors(&[("friend", 0.25, 0.1)]),
            viewport(),
            TICK_INTERVAL,
            PuppetMovementMode::Bottom,
        ));
        assert_eq!(puppets["friend"].position.y, BOTTOM_MOVEMENT_Y);
    }

    #[test]
    fn normalized_step_accounts_for_viewport_aspect_ratio() {
        let mut horizontal = PuppetState {
            id: "horizontal".to_owned(),
            position: CursorPosition::default(),
            is_moving: false,
        };
        let mut vertical = PuppetState {
            id: "vertical".to_owned(),
            position: CursorPosition::default(),
            is_moving: false,
        };

        advance_puppet(
            &mut horizontal,
            &CursorPosition { x: 1.0, y: 0.0 },
            viewport(),
            0.125,
        );
        advance_puppet(
            &mut vertical,
            &CursorPosition { x: 0.0, y: 1.0 },
            viewport(),
            0.125,
        );

        assert_eq!(horizontal.position.x, 0.01);
        assert_eq!(vertical.position.y, 0.02);
    }
}
