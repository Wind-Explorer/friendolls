use serde::{Deserialize, Serialize};
use specta::Type;

/// Public user identity exchanged with peers and remote servers. `id` is the
/// user's Ed25519 public key and is not persisted as local profile metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub display_name: String,
}
