use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use friendolls_common::InteractionContent;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

use crate::network::Network;

const SEEN_INTERACTION_LIMIT: usize = 256;

fn validate_content(content: &InteractionContent) -> Result<(), String> {
    content.validate()?;
    if let InteractionContent::Image { data, .. } = content {
        STANDARD
            .decode(data)
            .map_err(|_| "Image contains invalid Base64 data".to_owned())?;
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct FriendInteractionReceived {
    pub interaction_id: String,
    pub friend_id: String,
    pub content: InteractionContent,
}

#[derive(Default)]
struct SeenInteractions {
    ids: HashSet<String>,
    order: VecDeque<String>,
}

impl SeenInteractions {
    fn insert(&mut self, id: String) -> bool {
        if !self.ids.insert(id.clone()) {
            return false;
        }
        self.order.push_back(id);
        if self.order.len() > SEEN_INTERACTION_LIMIT
            && let Some(oldest) = self.order.pop_front()
        {
            self.ids.remove(&oldest);
        }
        true
    }
}

#[derive(Default)]
pub(crate) struct InteractionState(Mutex<SeenInteractions>);

pub fn init(handle: &AppHandle) {
    handle.manage(InteractionState::default());
}

pub(crate) fn receive(
    handle: &AppHandle,
    interaction_id: String,
    friend_id: String,
    payload: &str,
) {
    let Ok(content) = serde_json::from_str::<InteractionContent>(payload) else {
        eprintln!("failed to decode friend interaction");
        return;
    };
    if let Err(error) = validate_content(&content) {
        eprintln!("rejected friend interaction: {error}");
        return;
    }

    let dedupe_id = format!("{friend_id}\0{interaction_id}");
    let state = handle.state::<InteractionState>();
    let Ok(mut seen) = state.0.lock() else {
        eprintln!("failed to lock received interaction state");
        return;
    };
    if !seen.insert(dedupe_id) {
        return;
    }
    drop(seen);

    if let Err(error) = (FriendInteractionReceived {
        interaction_id,
        friend_id,
        content,
    })
    .emit(handle)
    {
        eprintln!("failed to emit friend interaction: {error}");
    }
}

#[tauri::command]
#[specta::specta]
pub async fn send_interaction(
    recipient_id: String,
    content: InteractionContent,
    network: State<'_, Network>,
) -> Result<(), String> {
    validate_content(&content)?;
    network.send_interaction(recipient_id, content).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use friendolls_common::MAX_TEXT_CHARS;

    #[test]
    fn validates_interaction_content_limits() {
        assert!(validate_content(&InteractionContent::Wave).is_ok());
        assert!(
            InteractionContent::Text {
                text: "hello".to_owned()
            }
            .validate()
            .is_ok()
        );
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
        assert!(
            InteractionContent::Image {
                media_type: "image/png".to_owned(),
                data: STANDARD.encode([1, 2, 3]),
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn image_media_type_uses_the_typescript_wire_name() {
        let content = InteractionContent::Image {
            media_type: "image/webp".to_owned(),
            data: STANDARD.encode([1, 2, 3]),
        };
        let value = serde_json::to_value(&content).unwrap();

        assert_eq!(value["mediaType"], "image/webp");
        assert!(value.get("media_type").is_none());
        assert!(matches!(
            serde_json::from_value(value).unwrap(),
            InteractionContent::Image { media_type, .. } if media_type == "image/webp"
        ));
    }

    #[test]
    fn received_interaction_ids_are_bounded_and_deduplicated() {
        let mut seen = SeenInteractions::default();
        assert!(seen.insert("first".to_owned()));
        assert!(!seen.insert("first".to_owned()));
        for index in 0..SEEN_INTERACTION_LIMIT {
            assert!(seen.insert(format!("id-{index}")));
        }
        assert_eq!(seen.ids.len(), SEEN_INTERACTION_LIMIT);
        assert!(seen.insert("first".to_owned()));
    }
}
