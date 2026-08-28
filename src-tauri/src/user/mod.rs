use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Public user identity exchanged with peers and remote servers. `id` is the
/// user's Ed25519 public key and is not persisted as local profile metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub display_name: String,
    pub skin_hash: Option<String>,
}

pub(crate) fn validate_id(id: &str) -> Result<(), String> {
    let bytes = URL_SAFE_NO_PAD
        .decode(id)
        .map_err(|_| "Identification key is not valid base64url.".to_owned())?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "Identification key must encode a 32-byte public key.".to_owned())?;
    VerifyingKey::from_bytes(&bytes)
        .map(|_| ())
        .map_err(|_| "Identification key is not a valid Ed25519 public key.".to_owned())
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::*;

    #[test]
    fn id_must_be_an_ed25519_public_key() {
        let valid = URL_SAFE_NO_PAD.encode(SigningKey::from_bytes(&[7; 32]).verifying_key());
        assert!(validate_id(&valid).is_ok());
        assert!(validate_id("not a public key").is_err());
    }
}
