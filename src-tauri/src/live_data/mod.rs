use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::cursor::CursorPositions;
use crate::ufa::AppMeta;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LiveData {
    Cursor { positions: CursorPositions },
    ForegroundApp { meta: AppMeta },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LiveDataSnapshot {
    pub cursor_positions: HashMap<String, CursorPositions>,
    pub foreground_apps: HashMap<String, AppMeta>,
}

#[tauri::command]
#[specta::specta]
pub fn list_live_data(handle: AppHandle) -> Result<LiveDataSnapshot, String> {
    Ok(LiveDataSnapshot {
        cursor_positions: handle.state::<crate::cursor::CursorState>().snapshot()?,
        foreground_apps: handle
            .state::<crate::ufa::ForegroundAppState>()
            .snapshot()?,
    })
}

pub(crate) fn publish_current(handle: &AppHandle) -> Result<(), String> {
    let user_id = handle
        .state::<crate::keypair::AppKeypair>()
        .public_key()
        .to_owned();
    let network = handle.state::<crate::network::Network>();

    if let Some(positions) = handle.state::<crate::cursor::CursorState>().get(&user_id)? {
        network.send_live_data(LiveData::Cursor { positions });
    }
    if let Some(meta) = handle
        .state::<crate::ufa::ForegroundAppState>()
        .get(&user_id)?
    {
        network.send_live_data(LiveData::ForegroundApp { meta });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::{CursorPosition, CursorPositions};

    #[test]
    fn live_data_is_internally_tagged_for_client_side_dispatch() {
        let payload = serde_json::to_string(&LiveData::Cursor {
            positions: CursorPositions {
                raw: CursorPosition { x: 120.0, y: 80.0 },
                mapped: CursorPosition { x: 0.25, y: 0.5 },
            },
        })
        .unwrap();

        assert_eq!(
            payload,
            r#"{"type":"cursor","positions":{"raw":{"x":120.0,"y":80.0},"mapped":{"x":0.25,"y":0.5}}}"#
        );
        assert!(matches!(
            serde_json::from_str(&payload).unwrap(),
            LiveData::Cursor { .. }
        ));
    }
}
