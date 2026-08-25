use std::io::Cursor;

use axum::extract::ws::Message;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use tokio::sync::mpsc;
use uuid::Uuid;
use wyd_common::{
    InteractionContent, InteractionDeliveryStatus, MAX_IMAGE_DIMENSION,
    MAX_INTERACTION_PAYLOAD_BYTES, ServerMessage, interaction_bytes,
};

use super::{Clients, verify};

pub(super) async fn relay(
    clients: &Clients,
    public_key: &str,
    connection_id: Uuid,
    interaction_id: &str,
    recipient_id: &str,
    payload: String,
    signature: &str,
) -> Option<InteractionDeliveryStatus> {
    let (source_key, recipient) = {
        let clients = clients.lock().await;
        let source = clients
            .get(public_key)
            .filter(|client| client.connection_id == connection_id)?;
        let recipient = clients
            .get(recipient_id)
            .filter(|recipient| {
                source.friends.iter().any(|friend| friend == recipient_id)
                    && recipient.friends.iter().any(|friend| friend == public_key)
            })
            .map(|recipient| recipient.sender.clone());
        (source.key, recipient)
    };

    if !verify(
        &source_key,
        &interaction_bytes(interaction_id, recipient_id, &payload),
        signature,
    ) {
        return None;
    }
    if payload.len() > MAX_INTERACTION_PAYLOAD_BYTES
        || recipient_id == public_key
        || !payload_is_valid(&payload)
    {
        return Some(InteractionDeliveryStatus::Rejected);
    }
    let Some(recipient) = recipient else {
        return Some(InteractionDeliveryStatus::Unavailable);
    };

    let message = Message::Text(
        serde_json::to_string(&ServerMessage::FriendInteraction {
            interaction_id: interaction_id.to_owned(),
            friend_id: public_key.to_owned(),
            payload,
        })
        .unwrap()
        .into(),
    );
    Some(match recipient.try_send(message) {
        Ok(()) => InteractionDeliveryStatus::Delivered,
        Err(mpsc::error::TrySendError::Full(_)) => InteractionDeliveryStatus::Busy,
        Err(mpsc::error::TrySendError::Closed(_)) => InteractionDeliveryStatus::Unavailable,
    })
}

fn payload_is_valid(payload: &str) -> bool {
    let Ok(content) = serde_json::from_str::<InteractionContent>(payload) else {
        return false;
    };
    if content.validate().is_err() {
        return false;
    }
    let InteractionContent::Image { media_type, data } = content else {
        return true;
    };

    let Ok(bytes) = STANDARD.decode(data) else {
        return false;
    };
    let expected_format = match media_type.as_str() {
        "image/webp" => image::ImageFormat::WebP,
        "image/jpeg" => image::ImageFormat::Jpeg,
        _ => return false,
    };
    if image::guess_format(&bytes).ok() != Some(expected_format) {
        return false;
    }

    let Ok((width, height)) =
        image::ImageReader::with_format(Cursor::new(bytes), expected_format).into_dimensions()
    else {
        return false;
    };
    width > 0 && height > 0 && width <= MAX_IMAGE_DIMENSION && height <= MAX_IMAGE_DIMENSION
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::{Signer, SigningKey};
    use image::{Rgb, RgbImage};
    use wyd_common::{MAX_TEXT_CHARS, Profile};

    use super::*;
    use crate::network::Client;

    fn jpeg_payload(width: u32, height: u32) -> String {
        let image = RgbImage::from_pixel(width, height, Rgb([30, 90, 150]));
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 50)
            .encode_image(&image)
            .unwrap();
        serde_json::json!({
            "type": "image",
            "mediaType": "image/jpeg",
            "data": STANDARD.encode(bytes),
        })
        .to_string()
    }

    #[test]
    fn payload_validation_enforces_the_shared_schema_and_image_details() {
        assert!(payload_is_valid(r#"{"type":"wave"}"#));
        assert!(!payload_is_valid(r#"{"type":"unknown"}"#));
        assert!(!payload_is_valid(r#"{"type":"text","text":" "}"#));
        assert!(!payload_is_valid(
            &serde_json::json!({
                "type": "text",
                "text": "x".repeat(MAX_TEXT_CHARS + 1),
            })
            .to_string()
        ));
        assert!(payload_is_valid(&jpeg_payload(
            MAX_IMAGE_DIMENSION,
            MAX_IMAGE_DIMENSION
        )));
        assert!(!payload_is_valid(&jpeg_payload(MAX_IMAGE_DIMENSION + 1, 1)));
        assert!(!payload_is_valid(
            r#"{"type":"image","mediaType":"image/jpeg","data":"not-base64"}"#
        ));
    }

    #[tokio::test]
    async fn relay_is_targeted_signed_and_reports_delivery_state() {
        let signing_key = SigningKey::from_bytes(&[8; 32]);
        let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
        let connection_id = Uuid::new_v4();
        let (source_sender, _source_receiver) = mpsc::channel(1);
        let (target_sender, mut target_receiver) = mpsc::channel(1);
        let (one_sided_sender, mut one_sided_receiver) = mpsc::channel(1);
        let clients = Clients::default();
        let mut registry = clients.lock().await;
        registry.insert(
            public_key.clone(),
            Client {
                connection_id,
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: public_key.clone(),
                    display_name: "Source".to_owned(),
                },
                friends: vec!["target".to_owned(), "one-sided".to_owned()],
                sender: source_sender,
            },
        );
        registry.insert(
            "target".to_owned(),
            Client {
                connection_id: Uuid::new_v4(),
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: "target".to_owned(),
                    display_name: "Target".to_owned(),
                },
                friends: vec![public_key.clone()],
                sender: target_sender.clone(),
            },
        );
        registry.insert(
            "one-sided".to_owned(),
            Client {
                connection_id: Uuid::new_v4(),
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: "one-sided".to_owned(),
                    display_name: "One-sided".to_owned(),
                },
                friends: Vec::new(),
                sender: one_sided_sender,
            },
        );
        drop(registry);

        let interaction_id = "interaction-1";
        let payload = r#"{"type":"wave"}"#.to_owned();
        let signature = URL_SAFE_NO_PAD.encode(
            signing_key
                .sign(&interaction_bytes(interaction_id, "target", &payload))
                .to_bytes(),
        );
        assert_eq!(
            relay(
                &clients,
                &public_key,
                connection_id,
                interaction_id,
                "target",
                payload.clone(),
                &signature,
            )
            .await,
            Some(InteractionDeliveryStatus::Delivered)
        );
        assert!(target_receiver.recv().await.is_some());
        assert!(one_sided_receiver.try_recv().is_err());

        assert!(
            target_sender
                .try_send(Message::Ping(Vec::new().into()))
                .is_ok()
        );
        let busy_id = "interaction-2";
        let busy_signature = URL_SAFE_NO_PAD.encode(
            signing_key
                .sign(&interaction_bytes(busy_id, "target", &payload))
                .to_bytes(),
        );
        assert_eq!(
            relay(
                &clients,
                &public_key,
                connection_id,
                busy_id,
                "target",
                payload.clone(),
                &busy_signature,
            )
            .await,
            Some(InteractionDeliveryStatus::Busy)
        );

        let unavailable_signature = URL_SAFE_NO_PAD.encode(
            signing_key
                .sign(&interaction_bytes(interaction_id, "one-sided", &payload))
                .to_bytes(),
        );
        assert_eq!(
            relay(
                &clients,
                &public_key,
                connection_id,
                interaction_id,
                "one-sided",
                payload.clone(),
                &unavailable_signature,
            )
            .await,
            Some(InteractionDeliveryStatus::Unavailable)
        );
        assert_eq!(
            relay(
                &clients,
                &public_key,
                Uuid::new_v4(),
                interaction_id,
                "target",
                payload.clone(),
                &signature,
            )
            .await,
            None
        );
        assert_eq!(
            relay(
                &clients,
                &public_key,
                connection_id,
                interaction_id,
                "target",
                "tampered".to_owned(),
                &signature,
            )
            .await,
            None
        );
    }

    #[tokio::test]
    async fn relay_rejects_invalid_payloads_before_delivery() {
        let signing_key = SigningKey::from_bytes(&[9; 32]);
        let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
        let connection_id = Uuid::new_v4();
        let (source_sender, _source_receiver) = mpsc::channel(1);
        let (target_sender, mut target_receiver) = mpsc::channel(1);
        let clients = Clients::default();
        let mut registry = clients.lock().await;
        registry.insert(
            public_key.clone(),
            Client {
                connection_id,
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: public_key.clone(),
                    display_name: "Source".to_owned(),
                },
                friends: vec!["target".to_owned()],
                sender: source_sender,
            },
        );
        registry.insert(
            "target".to_owned(),
            Client {
                connection_id: Uuid::new_v4(),
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: "target".to_owned(),
                    display_name: "Target".to_owned(),
                },
                friends: vec![public_key.clone()],
                sender: target_sender,
            },
        );
        drop(registry);

        for (interaction_id, payload) in [
            ("unknown", r#"{"type":"unknown"}"#.to_owned()),
            (
                "oversized",
                serde_json::json!({
                    "type": "wave",
                    "padding": "x".repeat(MAX_INTERACTION_PAYLOAD_BYTES),
                })
                .to_string(),
            ),
            (
                "long-text",
                serde_json::json!({
                    "type": "text",
                    "text": "x".repeat(MAX_TEXT_CHARS + 1),
                })
                .to_string(),
            ),
            ("oversized-image", jpeg_payload(MAX_IMAGE_DIMENSION + 1, 1)),
        ] {
            let signature = URL_SAFE_NO_PAD.encode(
                signing_key
                    .sign(&interaction_bytes(interaction_id, "target", &payload))
                    .to_bytes(),
            );
            assert_eq!(
                relay(
                    &clients,
                    &public_key,
                    connection_id,
                    interaction_id,
                    "target",
                    payload,
                    &signature,
                )
                .await,
                Some(InteractionDeliveryStatus::Rejected)
            );
        }
        assert!(target_receiver.try_recv().is_err());
    }
}
