use serde::{Deserialize, Serialize};
use specta::Type;

pub const VERSION: u8 = 3;
pub const MAX_INTERACTION_PAYLOAD_BYTES: usize = 160 * 1024;
pub const MAX_IMAGE_B64_SIZE: usize = 150 * 1024;
pub const MAX_SKIN_B64_SIZE: usize = 96 * 1024;
pub const MAX_IMAGE_DIMENSION: u32 = 480;
pub const MAX_TEXT_CHARS: usize = 500;

pub fn is_skin_hash(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub display_name: String,
    pub skin_hash: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    Register {
        profile: Profile,
        friends: Vec<String>,
        signature: String,
    },
    ProfileUpdated {
        profile: Profile,
        signature: String,
    },
    FriendsUpdated {
        friends: Vec<String>,
        signature: String,
    },
    SyncFriendProfiles,
    SyncFriendStatuses,
    ResolveProfile {
        request_id: String,
        user_id: String,
    },
    RequestSkin {
        request_id: String,
        user_id: String,
        skin_hash: String,
    },
    ProvideSkin {
        request_id: String,
        requester_id: String,
        skin_hash: String,
        data: Option<String>,
    },
    Signed {
        payload: String,
        signature: String,
    },
    Interaction {
        interaction_id: String,
        recipient_id: String,
        payload: String,
        signature: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InteractionDeliveryStatus {
    Delivered,
    Unavailable,
    Busy,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum InteractionContent {
    Text {
        text: String,
    },
    Wave,
    Image {
        #[serde(rename = "mediaType")]
        #[specta(rename = "mediaType")]
        media_type: String,
        data: String,
    },
}

impl InteractionContent {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Text { text } if text.trim().is_empty() => {
                Err("Message cannot be empty".to_owned())
            }
            Self::Text { text } if text.chars().count() > MAX_TEXT_CHARS => {
                Err(format!("Message exceeds {MAX_TEXT_CHARS} characters"))
            }
            Self::Image { media_type, .. }
                if !matches!(media_type.as_str(), "image/webp" | "image/jpeg") =>
            {
                Err("Image must be WebP or JPEG".to_owned())
            }
            Self::Image { data, .. } if data.is_empty() || data.len() > MAX_IMAGE_B64_SIZE => {
                Err(format!(
                    "Compressed image must be at most {} KiB",
                    MAX_IMAGE_B64_SIZE / 1024
                ))
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Challenge {
        version: u8,
        challenge: String,
    },
    Registered,
    FriendProfileUpdated {
        profile: Profile,
    },
    FriendProfiles {
        profiles: Vec<Profile>,
    },
    ProfileResolved {
        request_id: String,
        profile: Option<Profile>,
    },
    SkinRequested {
        request_id: String,
        requester_id: String,
        skin_hash: String,
    },
    SkinResolved {
        request_id: String,
        user_id: String,
        skin_hash: String,
        data: Option<String>,
    },
    FriendStatusChanged {
        friend_id: String,
        online: bool,
    },
    FriendStatuses {
        friend_ids: Vec<String>,
    },
    FriendLiveData {
        friend_id: String,
        payload: String,
    },
    FriendInteraction {
        interaction_id: String,
        friend_id: String,
        payload: String,
    },
    InteractionDelivery {
        interaction_id: String,
        status: InteractionDeliveryStatus,
    },
}

pub fn register_bytes(challenge: &str, profile: &Profile, friends: &[String]) -> Vec<u8> {
    format!(
        "wyd-register-v{VERSION}\n{challenge}\n{}\n{}\n{}\n{}",
        profile.id,
        profile.display_name,
        profile.skin_hash.as_deref().unwrap_or(""),
        friends.join("\n")
    )
    .into_bytes()
}

pub fn message_bytes(payload: &str) -> Vec<u8> {
    format!("wyd-message-v{VERSION}\n{payload}").into_bytes()
}

pub fn interaction_bytes(interaction_id: &str, recipient_id: &str, payload: &str) -> Vec<u8> {
    let mut bytes = format!("wyd-interaction-v{VERSION}\0").into_bytes();
    for value in [
        interaction_id.as_bytes(),
        recipient_id.as_bytes(),
        payload.as_bytes(),
    ] {
        bytes.extend_from_slice(&(value.len() as u64).to_be_bytes());
        bytes.extend_from_slice(value);
    }
    bytes
}

pub fn profile_bytes(profile: &Profile) -> Vec<u8> {
    format!(
        "wyd-profile-v{VERSION}\n{}\n{}\n{}",
        profile.id,
        profile.display_name,
        profile.skin_hash.as_deref().unwrap_or("")
    )
    .into_bytes()
}

pub fn friends_bytes(friends: &[String]) -> Vec<u8> {
    format!("wyd-friends-v{VERSION}\n{}", friends.join("\n")).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::{InteractionContent, MAX_TEXT_CHARS, Profile, interaction_bytes, profile_bytes};

    #[test]
    fn interaction_signature_bytes_are_length_delimited() {
        assert_ne!(
            interaction_bytes("one", "two", "three\nfour"),
            interaction_bytes("one\ntwo", "three", "four"),
        );
        assert_ne!(
            interaction_bytes("id", "recipient-a", "payload"),
            interaction_bytes("id", "recipient-b", "payload"),
        );
    }

    #[test]
    fn interaction_content_enforces_shared_wire_invariants() {
        assert!(InteractionContent::Wave.validate().is_ok());
        assert!(
            InteractionContent::Text {
                text: " ".to_owned()
            }
            .validate()
            .is_err()
        );
        assert!(
            InteractionContent::Text {
                text: "x".repeat(MAX_TEXT_CHARS + 1)
            }
            .validate()
            .is_err()
        );
        assert!(serde_json::from_str::<InteractionContent>(r#"{"type":"unknown"}"#).is_err());
    }

    #[test]
    fn profile_signature_bytes_include_the_skin_hash() {
        let mut profile = Profile {
            id: "id".to_owned(),
            display_name: "Name".to_owned(),
            skin_hash: None,
        };
        let without_skin = profile_bytes(&profile);
        profile.skin_hash = Some("a".repeat(64));
        assert_ne!(without_skin, profile_bytes(&profile));
    }
}
