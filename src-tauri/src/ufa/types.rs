use serde::{Deserialize, Serialize};

pub const ICON_SIZE: u32 = 64;
pub const ICON_CACHE_LIMIT: usize = 50;

/// Maximum base64-encoded icon size in bytes (~50KB).
/// A 64x64 RGBA PNG should be well under this. Anything larger
/// indicates multi-representation or uncompressed data that
/// could crash WebSocket payloads.
pub const MAX_ICON_B64_SIZE: usize = 50_000;

/// Metadata for the currently active application, including localized and unlocalized names, and an optional base64-encoded icon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMeta {
    pub local: Option<String>,
    pub unlocal: Option<String>,
    pub ico: Option<String>,
}
