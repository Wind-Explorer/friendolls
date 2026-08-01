use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signer, SigningKey};
use sqlx::FromRow;
use tauri::{AppHandle, Manager, State};

use crate::db::{self, AppDatabase};

#[cfg(test)]
mod tests;

const KEYPAIR_ID: i64 = 1;

#[derive(Debug, FromRow)]
struct StoredKeypair {
    public_key: String,
    secret_key: String,
}

#[derive(Debug)]
pub enum KeypairError {
    Database(sqlx::Error),
    Randomness(getrandom::Error),
    InvalidEncoding(base64::DecodeError),
    InvalidKeyLength {
        key: &'static str,
        expected: usize,
        actual: usize,
    },
    PublicKeyMismatch,
    MissingAfterPrepare,
}

impl fmt::Display for KeypairError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "keypair database error: {error}"),
            Self::Randomness(error) => write!(formatter, "could not generate a keypair: {error}"),
            Self::InvalidEncoding(error) => {
                write!(formatter, "stored keypair is not valid base64: {error}")
            }
            Self::InvalidKeyLength {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "stored {key} has an invalid length: expected {expected} bytes, got {actual}"
            ),
            Self::PublicKeyMismatch => {
                write!(formatter, "stored public key does not match the secret key")
            }
            Self::MissingAfterPrepare => {
                write!(formatter, "keypair was not present after preparation")
            }
        }
    }
}

impl std::error::Error for KeypairError {}

impl From<sqlx::Error> for KeypairError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error)
    }
}

impl From<getrandom::Error> for KeypairError {
    fn from(error: getrandom::Error) -> Self {
        Self::Randomness(error)
    }
}

impl From<base64::DecodeError> for KeypairError {
    fn from(error: base64::DecodeError) -> Self {
        Self::InvalidEncoding(error)
    }
}

pub async fn init(handle: &AppHandle) -> Result<(), KeypairError> {
    let keypair = prepare(&handle.state::<db::AppDatabase>()).await?;
    handle.manage(keypair);
    Ok(())
}

fn decode_key<const LENGTH: usize>(
    encoded: &str,
    key: &'static str,
) -> Result<[u8; LENGTH], KeypairError> {
    let decoded = URL_SAFE_NO_PAD.decode(encoded)?;
    let actual = decoded.len();
    decoded
        .try_into()
        .map_err(|_| KeypairError::InvalidKeyLength {
            key,
            expected: LENGTH,
            actual,
        })
}

impl StoredKeypair {
    fn signing_key(&self) -> Result<SigningKey, KeypairError> {
        let secret_key = decode_key::<32>(&self.secret_key, "secret key")?;
        let public_key = decode_key::<32>(&self.public_key, "public key")?;
        let signing_key = SigningKey::from_bytes(&secret_key);

        if signing_key.verifying_key().to_bytes() != public_key {
            return Err(KeypairError::PublicKeyMismatch);
        }

        Ok(signing_key)
    }
}

#[allow(dead_code)] // The signing key will be read by the pending WebSocket sender.
pub struct AppKeypair {
    signing_key: SigningKey,
    public_key: String,
}

impl AppKeypair {
    fn from_stored(keypair: StoredKeypair) -> Result<Self, KeypairError> {
        let signing_key = keypair.signing_key()?;
        Ok(Self {
            signing_key,
            public_key: keypair.public_key,
        })
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    #[allow(dead_code)] // Exercised by tests until the WebSocket sender is connected.
    pub fn sign(&self, payload: &[u8]) -> String {
        let signature = self.signing_key.sign(payload);
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    }
}

async fn stored(database: &AppDatabase) -> Result<Option<StoredKeypair>, KeypairError> {
    sqlx::query_as::<_, StoredKeypair>("SELECT public_key, secret_key FROM keypair WHERE id = ?1")
        .bind(KEYPAIR_ID)
        .fetch_optional(database.pool())
        .await
        .map_err(Into::into)
}

/// Creates and persists the app identity when it does not exist yet.
///
/// The returned keypair contains the stable entity ID shared with friends and
/// the decoded signing key. The secret key is intentionally stored unencrypted
/// in SQLite and loaded into memory once during app startup.
pub async fn prepare(database: &AppDatabase) -> Result<AppKeypair, KeypairError> {
    if let Some(keypair) = stored(database).await? {
        return AppKeypair::from_stored(keypair);
    }

    let mut secret_key = [0_u8; 32];
    getrandom::fill(&mut secret_key)?;
    let signing_key = SigningKey::from_bytes(&secret_key);
    let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().as_bytes());
    let secret_key = URL_SAFE_NO_PAD.encode(secret_key);

    // Another caller may prepare the singleton concurrently. In that case the
    // already-persisted identity remains authoritative.
    sqlx::query(
        "INSERT INTO keypair (id, public_key, secret_key) VALUES (?1, ?2, ?3) \
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(KEYPAIR_ID)
    .bind(public_key)
    .bind(secret_key)
    .execute(database.pool())
    .await?;

    let keypair = stored(database)
        .await?
        .ok_or(KeypairError::MissingAfterPrepare)?;
    AppKeypair::from_stored(keypair)
}

#[tauri::command]
#[specta::specta]
pub fn get_public_key(keypair: State<'_, AppKeypair>) -> String {
    keypair.public_key().to_owned()
}
