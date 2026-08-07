use serde::{Deserialize, Serialize};

pub const VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub display_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    Register { profile: Profile, signature: String },
    ProfileUpdated { profile: Profile, signature: String },
    Signed { payload: String, signature: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Challenge { version: u8, challenge: String },
    Registered,
}

pub fn register_bytes(challenge: &str, profile: &Profile) -> Vec<u8> {
    format!(
        "wyd-register-v{VERSION}\n{challenge}\n{}\n{}",
        profile.id, profile.display_name
    )
    .into_bytes()
}

pub fn message_bytes(payload: &str) -> Vec<u8> {
    format!("wyd-message-v{VERSION}\n{payload}").into_bytes()
}

pub fn profile_bytes(profile: &Profile) -> Vec<u8> {
    format!(
        "wyd-profile-v{VERSION}\n{}\n{}",
        profile.id, profile.display_name
    )
    .into_bytes()
}
