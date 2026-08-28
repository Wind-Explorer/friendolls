use axum::extract::ws::Message;
use uuid::Uuid;
use wyd_common::{MAX_SKIN_B64_SIZE, ServerMessage};

use super::{Clients, are_mutual_friends};

pub(super) enum RequestOutcome {
    StaleSession,
    Forwarded,
    Unavailable(Message),
}

pub(super) async fn request(
    clients: &Clients,
    requester_id: &str,
    connection_id: Uuid,
    request_id: String,
    user_id: String,
    skin_hash: String,
) -> RequestOutcome {
    let clients = clients.lock().await;
    let Some(requester) = clients
        .get(requester_id)
        .filter(|client| client.connection_id == connection_id)
    else {
        return RequestOutcome::StaleSession;
    };
    let target = clients.get(&user_id).filter(|target| {
        are_mutual_friends(requester_id, requester, &user_id, target)
            && target.profile.skin_hash.as_deref() == Some(&skin_hash)
    });

    let Some(target) = target else {
        return RequestOutcome::Unavailable(resolved(request_id, user_id, skin_hash, None));
    };
    let request = Message::Text(
        serde_json::to_string(&ServerMessage::SkinRequested {
            request_id: request_id.clone(),
            requester_id: requester_id.to_owned(),
            skin_hash: skin_hash.clone(),
        })
        .expect("skin request serializes")
        .into(),
    );
    match target.sender.try_send(request) {
        Ok(()) => RequestOutcome::Forwarded,
        Err(_) => RequestOutcome::Unavailable(resolved(request_id, user_id, skin_hash, None)),
    }
}

pub(super) async fn provide(
    clients: &Clients,
    provider_id: &str,
    connection_id: Uuid,
    request_id: String,
    requester_id: String,
    skin_hash: String,
    data: Option<String>,
) -> bool {
    if data
        .as_ref()
        .is_some_and(|data| data.len() > MAX_SKIN_B64_SIZE)
    {
        return false;
    }

    let clients = clients.lock().await;
    let Some(provider) = clients
        .get(provider_id)
        .filter(|client| client.connection_id == connection_id)
        .filter(|client| client.profile.skin_hash.as_deref() == Some(&skin_hash))
    else {
        return false;
    };
    let Some(requester) = clients
        .get(&requester_id)
        .filter(|requester| are_mutual_friends(provider_id, provider, &requester_id, requester))
    else {
        return true;
    };
    let _ = requester.sender.try_send(resolved(
        request_id,
        provider_id.to_owned(),
        skin_hash,
        data,
    ));
    true
}

fn resolved(
    request_id: String,
    user_id: String,
    skin_hash: String,
    data: Option<String>,
) -> Message {
    Message::Text(
        serde_json::to_string(&ServerMessage::SkinResolved {
            request_id,
            user_id,
            skin_hash,
            data,
        })
        .expect("skin response serializes")
        .into(),
    )
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::SigningKey;
    use tokio::sync::mpsc;
    use wyd_common::Profile;

    use super::*;
    use crate::network::Client;

    fn client(
        id: &str,
        connection_id: Uuid,
        friends: Vec<String>,
        skin_hash: Option<String>,
        sender: mpsc::Sender<Message>,
    ) -> Client {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        Client {
            connection_id,
            key: signing_key.verifying_key(),
            profile: Profile {
                id: id.to_owned(),
                display_name: id.to_owned(),
                skin_hash,
            },
            friends,
            sender,
        }
    }

    #[tokio::test]
    async fn mutual_friends_can_round_trip_the_advertised_skin() {
        let requester_id = URL_SAFE_NO_PAD.encode([1_u8; 32]);
        let provider_id = URL_SAFE_NO_PAD.encode([2_u8; 32]);
        let skin_hash = "a".repeat(64);
        let requester_connection = Uuid::new_v4();
        let provider_connection = Uuid::new_v4();
        let (requester_sender, mut requester_receiver) = mpsc::channel(2);
        let (provider_sender, mut provider_receiver) = mpsc::channel(2);
        let clients = Clients::default();
        {
            let mut clients = clients.lock().await;
            clients.insert(
                requester_id.clone(),
                client(
                    &requester_id,
                    requester_connection,
                    vec![provider_id.clone()],
                    None,
                    requester_sender,
                ),
            );
            clients.insert(
                provider_id.clone(),
                client(
                    &provider_id,
                    provider_connection,
                    vec![requester_id.clone()],
                    Some(skin_hash.clone()),
                    provider_sender,
                ),
            );
        }

        assert!(matches!(
            request(
                &clients,
                &requester_id,
                requester_connection,
                "request".to_owned(),
                provider_id.clone(),
                skin_hash.clone(),
            )
            .await,
            RequestOutcome::Forwarded
        ));
        let Message::Text(requested) = provider_receiver.recv().await.unwrap() else {
            panic!("expected skin request")
        };
        assert!(matches!(
            serde_json::from_str(&requested).unwrap(),
            ServerMessage::SkinRequested { request_id, .. } if request_id == "request"
        ));

        assert!(
            provide(
                &clients,
                &provider_id,
                provider_connection,
                "request".to_owned(),
                requester_id,
                skin_hash,
                Some("png".to_owned()),
            )
            .await
        );
        let Message::Text(resolved) = requester_receiver.recv().await.unwrap() else {
            panic!("expected skin response")
        };
        assert!(matches!(
            serde_json::from_str(&resolved).unwrap(),
            ServerMessage::SkinResolved { data: Some(data), .. } if data == "png"
        ));
    }

    #[tokio::test]
    async fn request_requires_the_exact_advertised_hash() {
        let (requester_sender, _requester_receiver) = mpsc::channel(1);
        let (provider_sender, mut provider_receiver) = mpsc::channel(1);
        let requester_connection = Uuid::new_v4();
        let requester_id = "requester".to_owned();
        let provider_id = "provider".to_owned();
        let clients = Clients::default();
        {
            let mut clients = clients.lock().await;
            clients.insert(
                requester_id.clone(),
                client(
                    &requester_id,
                    requester_connection,
                    vec![provider_id.clone()],
                    None,
                    requester_sender,
                ),
            );
            clients.insert(
                provider_id.clone(),
                client(
                    &provider_id,
                    Uuid::new_v4(),
                    vec![requester_id.clone()],
                    Some("a".repeat(64)),
                    provider_sender,
                ),
            );
        }

        let response = request(
            &clients,
            &requester_id,
            requester_connection,
            "request".to_owned(),
            provider_id,
            "b".repeat(64),
        )
        .await;
        assert!(matches!(response, RequestOutcome::Unavailable(_)));
        assert!(provider_receiver.try_recv().is_err());
    }

    async fn request_with_provider_sender(
        provider_sender: mpsc::Sender<Message>,
    ) -> RequestOutcome {
        let (requester_sender, _requester_receiver) = mpsc::channel(1);
        let requester_connection = Uuid::new_v4();
        let requester_id = "requester".to_owned();
        let provider_id = "provider".to_owned();
        let skin_hash = "a".repeat(64);
        let clients = Clients::default();
        {
            let mut clients = clients.lock().await;
            clients.insert(
                requester_id.clone(),
                client(
                    &requester_id,
                    requester_connection,
                    vec![provider_id.clone()],
                    None,
                    requester_sender,
                ),
            );
            clients.insert(
                provider_id.clone(),
                client(
                    &provider_id,
                    Uuid::new_v4(),
                    vec![requester_id.clone()],
                    Some(skin_hash.clone()),
                    provider_sender,
                ),
            );
        }

        request(
            &clients,
            &requester_id,
            requester_connection,
            "request".to_owned(),
            provider_id,
            skin_hash,
        )
        .await
    }

    #[tokio::test]
    async fn closed_target_queue_returns_an_immediate_response() {
        let (provider_sender, provider_receiver) = mpsc::channel(1);
        drop(provider_receiver);

        assert!(matches!(
            request_with_provider_sender(provider_sender).await,
            RequestOutcome::Unavailable(_)
        ));
    }

    #[tokio::test]
    async fn full_target_queue_returns_an_immediate_response() {
        let (provider_sender, _provider_receiver) = mpsc::channel(1);
        provider_sender
            .try_send(Message::Text("occupied".into()))
            .unwrap();

        assert!(matches!(
            request_with_provider_sender(provider_sender).await,
            RequestOutcome::Unavailable(_)
        ));
    }
}
