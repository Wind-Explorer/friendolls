use serde::{Deserialize, Serialize};

use crate::cursor::CursorPositions;
use crate::ufa::AppMeta;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum LiveData {
    Cursor { positions: CursorPositions },
    ForegroundApp { meta: AppMeta },
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
