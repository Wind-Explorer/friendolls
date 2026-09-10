use std::collections::HashMap;
use std::hash::Hash;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

use crate::activity::Activities;
use crate::cursor::CursorPositions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LiveDataKind {
    Cursor,
    Activity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveDataEnvelope {
    pub session_id: String,
    pub sequence: u64,
    #[serde(flatten)]
    pub data: LiveData,
}

impl LiveDataEnvelope {
    pub(crate) fn kind(&self) -> LiveDataKind {
        self.data.kind()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LiveData {
    Cursor { positions: CursorPositions },
    Activities { activities: Activities },
}

impl LiveData {
    pub(crate) fn kind(&self) -> LiveDataKind {
        match self {
            Self::Cursor { .. } => LiveDataKind::Cursor,
            Self::Activities { .. } => LiveDataKind::Activity,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LiveDataSnapshot {
    pub cursor_positions: HashMap<String, CursorPositions>,
    pub activities: HashMap<String, Activities>,
}

#[tauri::command]
#[specta::specta]
pub fn list_live_data(handle: AppHandle) -> Result<LiveDataSnapshot, String> {
    Ok(LiveDataSnapshot {
        cursor_positions: handle.state::<crate::cursor::CursorState>().snapshot()?,
        activities: handle
            .state::<crate::activity::ActivityState>()
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
    if let Some(activities) = handle
        .state::<crate::activity::ActivityState>()
        .get(&user_id)?
    {
        network.send_live_data(LiveData::Activities { activities });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cursor::{CursorPosition, CursorPositions};

    #[test]
    fn live_data_is_internally_tagged_for_client_side_dispatch() {
        let payload = serde_json::to_string(&LiveDataEnvelope {
            session_id: "session".to_owned(),
            sequence: 7,
            data: LiveData::Cursor {
                positions: CursorPositions {
                    raw: CursorPosition { x: 120.0, y: 80.0 },
                    mapped: CursorPosition { x: 0.25, y: 0.5 },
                },
            },
        })
        .unwrap();

        assert_eq!(
            payload,
            r#"{"sessionId":"session","sequence":7,"type":"cursor","positions":{"raw":{"x":120.0,"y":80.0},"mapped":{"x":0.25,"y":0.5}}}"#
        );
        assert!(matches!(
            serde_json::from_str(&payload).unwrap(),
            LiveDataEnvelope {
                data: LiveData::Cursor { .. },
                ..
            }
        ));
    }
}
