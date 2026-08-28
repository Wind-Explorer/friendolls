use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{Mutex, mpsc};
use uuid::Uuid;
use wyd_common::{ClientMessage, Profile, ServerMessage, message_bytes, register_bytes};

mod interactions;
mod presence;
mod profiles;
mod skins;

type Clients = Arc<Mutex<HashMap<String, Client>>>;

struct Client {
    connection_id: Uuid,
    key: VerifyingKey,
    profile: Profile,
    friends: Vec<String>,
    sender: mpsc::Sender<Message>,
}

fn are_mutual_friends(source_id: &str, source: &Client, friend_id: &str, friend: &Client) -> bool {
    source.friends.iter().any(|id| id == friend_id)
        && friend.friends.iter().any(|id| id == source_id)
}

pub fn routes() -> Router {
    Router::new()
        .route("/v1/ws", get(upgrade))
        .with_state(Clients::default())
}

async fn upgrade(ws: WebSocketUpgrade, State(clients): State<Clients>) -> Response {
    ws.on_upgrade(move |socket| connected(socket, clients))
}

async fn connected(mut socket: WebSocket, clients: Clients) {
    let challenge = Uuid::new_v4().to_string();
    if send(
        &mut socket,
        &ServerMessage::Challenge {
            version: wyd_common::VERSION,
            challenge: challenge.clone(),
        },
    )
    .await
    .is_err()
    {
        return;
    }

    let Some(Ok(Message::Text(text))) = socket.recv().await else {
        return;
    };
    let Ok(ClientMessage::Register {
        profile,
        friends,
        signature,
    }) = serde_json::from_str(&text)
    else {
        return;
    };
    if profile
        .skin_hash
        .as_deref()
        .is_some_and(|hash| !wyd_common::is_skin_hash(hash))
    {
        return;
    }
    let Ok(key) = key(&profile.id) else { return };
    if !verify(
        &key,
        &register_bytes(&challenge, &profile, &friends),
        &signature,
    ) {
        return;
    }

    let connection_id = Uuid::new_v4();
    let public_key = profile.id.clone();
    let (sender, mut outgoing) = mpsc::channel(32);
    let previous_friends = clients
        .lock()
        .await
        .insert(
            public_key.clone(),
            Client {
                connection_id,
                key,
                profile,
                friends,
                sender,
            },
        )
        .map(|client| client.friends);
    if send(&mut socket, &ServerMessage::Registered).await.is_err() {
        presence::disconnected(&clients, &public_key, connection_id).await;
        return;
    }
    profiles::broadcast(&clients, &public_key, connection_id).await;
    presence::connected(&clients, &public_key, previous_friends).await;

    let (mut writer, mut reader) = socket.split();
    let mut ping = tokio::time::interval(Duration::from_secs(20));
    loop {
        tokio::select! {
            message = outgoing.recv() => {
                let Some(message) = message else { break };
                if writer.send(message).await.is_err() { break; }
            }
            _ = ping.tick() => {
                if writer.send(Message::Ping(Vec::new().into())).await.is_err() { break; }
            }
            message = reader.next() => match message {
                Some(Ok(Message::Text(text))) => match serde_json::from_str(&text) {
                    Ok(ClientMessage::Signed { payload, signature }) => {
                        if !relay_live_data(&clients, &public_key, connection_id, payload, &signature).await {
                            break;
                        }
                    }
                    Ok(ClientMessage::Interaction {
                        interaction_id,
                        recipient_id,
                        payload,
                        signature,
                    }) => {
                        let Some(status) = interactions::relay(
                            &clients,
                            &public_key,
                            connection_id,
                            &interaction_id,
                            &recipient_id,
                            payload,
                            &signature,
                        ).await else {
                            break;
                        };
                        let response = ServerMessage::InteractionDelivery {
                            interaction_id,
                            status,
                        };
                        if writer.send(Message::Text(serde_json::to_string(&response).unwrap().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(ClientMessage::ProfileUpdated { profile, signature }) => {
                        if !profiles::update(&clients, &public_key, connection_id, profile, &signature).await {
                            break;
                        }
                    }
                    Ok(ClientMessage::FriendsUpdated { friends, signature }) => {
                        let Some(friend_ids) = presence::update_friends(
                            &clients,
                            &public_key,
                            connection_id,
                            friends,
                            &signature,
                        ).await else {
                            break;
                        };
                        let message = ServerMessage::FriendStatuses { friend_ids };
                        if writer.send(Message::Text(serde_json::to_string(&message).unwrap().into())).await.is_err() {
                            break;
                        }
                        let Some(profiles) = profiles::snapshot(&clients, &public_key, connection_id).await else {
                            break;
                        };
                        let message = ServerMessage::FriendProfiles { profiles };
                        if writer.send(Message::Text(serde_json::to_string(&message).unwrap().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(ClientMessage::SyncFriendProfiles) => {
                        let Some(profiles) = profiles::snapshot(&clients, &public_key, connection_id).await else {
                            break;
                        };
                        let message = ServerMessage::FriendProfiles { profiles };
                        if writer.send(Message::Text(serde_json::to_string(&message).unwrap().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(ClientMessage::SyncFriendStatuses) => {
                        let Some(friend_ids) = presence::snapshot(&clients, &public_key, connection_id).await else {
                            break;
                        };
                        let message = ServerMessage::FriendStatuses { friend_ids };
                        if writer.send(Message::Text(serde_json::to_string(&message).unwrap().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(ClientMessage::ResolveProfile { request_id, user_id }) => {
                        let Some(profile) = profiles::resolve(
                            &clients,
                            &public_key,
                            connection_id,
                            &user_id,
                        ).await else {
                            break;
                        };
                        let message = ServerMessage::ProfileResolved {
                            request_id,
                            profile,
                        };
                        if writer.send(Message::Text(serde_json::to_string(&message).unwrap().into())).await.is_err() {
                            break;
                        }
                    }
                    Ok(ClientMessage::RequestSkin { request_id, user_id, skin_hash }) => {
                        match skins::request(
                            &clients,
                            &public_key,
                            connection_id,
                            request_id,
                            user_id,
                            skin_hash,
                        ).await {
                            skins::RequestOutcome::StaleSession => break,
                            skins::RequestOutcome::Forwarded => {}
                            skins::RequestOutcome::Unavailable(response) => {
                                if writer.send(response).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(ClientMessage::ProvideSkin {
                        request_id,
                        requester_id,
                        skin_hash,
                        data,
                    }) => {
                        if !skins::provide(
                            &clients,
                            &public_key,
                            connection_id,
                            request_id,
                            requester_id,
                            skin_hash,
                            data,
                        ).await {
                            break;
                        }
                    }
                    _ => break,
                }
                Some(Ok(Message::Ping(data))) => {
                    if writer.send(Message::Pong(data)).await.is_err() { break; }
                }
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            }
        }
    }

    presence::disconnected(&clients, &public_key, connection_id).await;
}

async fn relay_live_data(
    clients: &Clients,
    public_key: &str,
    connection_id: Uuid,
    payload: String,
    signature: &str,
) -> bool {
    let clients = clients.lock().await;
    let Some(source) = clients
        .get(public_key)
        .filter(|client| client.connection_id == connection_id)
    else {
        return false;
    };
    if !verify(&source.key, &message_bytes(&payload), signature) {
        return false;
    }

    let recipients: Vec<_> = clients
        .iter()
        .filter(|(recipient_id, recipient)| {
            recipient_id.as_str() != public_key
                && are_mutual_friends(public_key, source, recipient_id, recipient)
        })
        .map(|(_, recipient)| recipient.sender.clone())
        .collect();
    drop(clients);

    let message = Message::Text(
        serde_json::to_string(&ServerMessage::FriendLiveData {
            friend_id: public_key.to_owned(),
            payload,
        })
        .unwrap()
        .into(),
    );
    for recipient in recipients {
        let _ = recipient.try_send(message.clone());
    }
    true
}

async fn send(socket: &mut WebSocket, message: &ServerMessage) -> Result<(), axum::Error> {
    socket
        .send(Message::Text(
            serde_json::to_string(message).unwrap().into(),
        ))
        .await
}

fn key(public_key: &str) -> Result<VerifyingKey, ()> {
    let bytes = URL_SAFE_NO_PAD.decode(public_key).map_err(|_| ())?;
    let bytes: [u8; 32] = bytes.try_into().map_err(|_| ())?;
    VerifyingKey::from_bytes(&bytes).map_err(|_| ())
}

fn verify(key: &VerifyingKey, bytes: &[u8], signature: &str) -> bool {
    let Ok(signature) = URL_SAFE_NO_PAD.decode(signature) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&signature) else {
        return false;
    };
    key.verify(bytes, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use wyd_common::friends_bytes;

    use super::*;

    #[test]
    fn verifies_registration_and_message_signatures() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let profile = Profile {
            id: URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes()),
            display_name: "Wind".to_owned(),
            skin_hash: None,
        };
        let friends = vec!["friend".to_owned()];
        let registration = register_bytes("challenge", &profile, &friends);
        let signature = URL_SAFE_NO_PAD.encode(signing_key.sign(&registration).to_bytes());

        assert!(verify(
            &signing_key.verifying_key(),
            &registration,
            &signature
        ));
        assert!(!verify(
            &signing_key.verifying_key(),
            &register_bytes("another challenge", &profile, &friends),
            &signature,
        ));
        assert!(!verify(
            &signing_key.verifying_key(),
            &register_bytes("challenge", &profile, &["another-friend".to_owned()]),
            &signature,
        ));

        let message = message_bytes("hello");
        let signature = URL_SAFE_NO_PAD.encode(signing_key.sign(&message).to_bytes());
        assert!(verify(&signing_key.verifying_key(), &message, &signature));
        assert!(!verify(
            &signing_key.verifying_key(),
            &message_bytes("tampered"),
            &signature,
        ));
    }

    #[tokio::test]
    async fn authenticated_friend_update_changes_only_ids() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
        let connection_id = Uuid::new_v4();
        let (sender, _receiver) = mpsc::channel(1);
        let clients = Clients::default();
        clients.lock().await.insert(
            public_key.clone(),
            Client {
                connection_id,
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: public_key.clone(),
                    display_name: "Wind".to_owned(),
                    skin_hash: None,
                },
                friends: Vec::new(),
                sender,
            },
        );
        let friends = vec!["friend-a".to_owned(), "friend-b".to_owned()];
        let signature =
            URL_SAFE_NO_PAD.encode(signing_key.sign(&friends_bytes(&friends)).to_bytes());

        assert!(
            presence::update_friends(
                &clients,
                &public_key,
                connection_id,
                friends.clone(),
                &signature,
            )
            .await
            .is_some()
        );
        assert_eq!(
            clients.lock().await.get(&public_key).unwrap().friends,
            friends
        );
    }

    #[tokio::test]
    async fn presence_sync_and_disconnect_broadcast_require_mutual_friendship() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let connection_id = Uuid::new_v4();
        let (source_sender, _source_receiver) = mpsc::channel(4);
        let (mutual_sender, mut mutual_receiver) = mpsc::channel(4);
        let (one_way_sender, mut one_way_receiver) = mpsc::channel(4);
        let clients = Clients::default();
        let mut registry = clients.lock().await;
        registry.insert(
            "source".to_owned(),
            Client {
                connection_id,
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: "source".to_owned(),
                    display_name: "Source".to_owned(),
                    skin_hash: None,
                },
                friends: vec!["mutual".to_owned()],
                sender: source_sender,
            },
        );
        registry.insert(
            "mutual".to_owned(),
            Client {
                connection_id: Uuid::new_v4(),
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: "mutual".to_owned(),
                    display_name: "Mutual".to_owned(),
                    skin_hash: None,
                },
                friends: vec!["source".to_owned()],
                sender: mutual_sender,
            },
        );
        registry.insert(
            "one-way".to_owned(),
            Client {
                connection_id: Uuid::new_v4(),
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: "one-way".to_owned(),
                    display_name: "One way".to_owned(),
                    skin_hash: None,
                },
                friends: vec!["source".to_owned()],
                sender: one_way_sender,
            },
        );
        drop(registry);

        assert_eq!(
            presence::snapshot(&clients, "source", connection_id)
                .await
                .expect("current session"),
            ["mutual"]
        );
        assert!(
            presence::snapshot(&clients, "source", Uuid::new_v4())
                .await
                .is_none()
        );

        presence::broadcast_status(&clients, "source", true).await;
        assert_friend_status(mutual_receiver.recv().await, "source", true);
        assert!(one_way_receiver.try_recv().is_err());

        let no_friends = Vec::new();
        let signature =
            URL_SAFE_NO_PAD.encode(signing_key.sign(&friends_bytes(&no_friends)).to_bytes());
        assert_eq!(
            presence::update_friends(&clients, "source", connection_id, no_friends, &signature,)
                .await,
            Some(Vec::new())
        );
        assert_friend_status(mutual_receiver.recv().await, "source", false);

        let mutual_friends = vec!["mutual".to_owned()];
        let signature =
            URL_SAFE_NO_PAD.encode(signing_key.sign(&friends_bytes(&mutual_friends)).to_bytes());
        assert_eq!(
            presence::update_friends(
                &clients,
                "source",
                connection_id,
                mutual_friends,
                &signature,
            )
            .await,
            Some(vec!["mutual".to_owned()])
        );
        assert_friend_status(mutual_receiver.recv().await, "source", true);

        presence::disconnected(&clients, "source", Uuid::new_v4()).await;
        assert!(clients.lock().await.contains_key("source"));
        assert!(mutual_receiver.try_recv().is_err());

        presence::disconnected(&clients, "source", connection_id).await;
        assert_friend_status(mutual_receiver.recv().await, "source", false);
        assert!(one_way_receiver.try_recv().is_err());
        assert!(!clients.lock().await.contains_key("source"));
    }

    #[tokio::test]
    async fn live_data_relay_requires_valid_session_signature_and_mutual_friendship() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
        let connection_id = Uuid::new_v4();
        let (source_sender, _source_receiver) = mpsc::channel(1);
        let (mutual_sender, mut mutual_receiver) = mpsc::channel(1);
        let (sender_only_sender, mut sender_only_receiver) = mpsc::channel(1);
        let (recipient_only_sender, mut recipient_only_receiver) = mpsc::channel(1);
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
                    skin_hash: None,
                },
                friends: vec!["mutual".to_owned(), "sender-only".to_owned()],
                sender: source_sender,
            },
        );
        for (id, friends, sender) in [
            ("mutual", vec![public_key.clone()], mutual_sender),
            ("sender-only", Vec::new(), sender_only_sender),
            (
                "recipient-only",
                vec![public_key.clone()],
                recipient_only_sender,
            ),
        ] {
            registry.insert(
                id.to_owned(),
                Client {
                    connection_id: Uuid::new_v4(),
                    key: signing_key.verifying_key(),
                    profile: Profile {
                        id: id.to_owned(),
                        display_name: id.to_owned(),
                        skin_hash: None,
                    },
                    friends,
                    sender,
                },
            );
        }
        drop(registry);

        let payload = r#"{"type":"cursor","positions":{}}"#.to_owned();
        let signature =
            URL_SAFE_NO_PAD.encode(signing_key.sign(&message_bytes(&payload)).to_bytes());
        assert!(
            relay_live_data(
                &clients,
                &public_key,
                connection_id,
                payload.clone(),
                &signature,
            )
            .await
        );

        let Message::Text(message) = mutual_receiver
            .recv()
            .await
            .expect("mutual friend receives")
        else {
            panic!("expected text live data");
        };
        let ServerMessage::FriendLiveData {
            friend_id,
            payload: received_payload,
        } = serde_json::from_str(&message).expect("decode live data")
        else {
            panic!("expected friend live data");
        };
        assert_eq!(friend_id, public_key);
        assert_eq!(received_payload, payload);
        assert!(sender_only_receiver.try_recv().is_err());
        assert!(recipient_only_receiver.try_recv().is_err());

        assert!(
            !relay_live_data(
                &clients,
                &public_key,
                Uuid::new_v4(),
                payload.clone(),
                &signature,
            )
            .await
        );
        assert!(
            !relay_live_data(
                &clients,
                &public_key,
                connection_id,
                "tampered".to_owned(),
                &signature,
            )
            .await
        );
        assert!(mutual_receiver.try_recv().is_err());
    }

    fn assert_friend_status(message: Option<Message>, expected_id: &str, expected_online: bool) {
        let Message::Text(message) = message.expect("friend receives status") else {
            panic!("expected text status");
        };
        let ServerMessage::FriendStatusChanged { friend_id, online } =
            serde_json::from_str(&message).expect("decode friend status")
        else {
            panic!("expected friend status");
        };
        assert_eq!(friend_id, expected_id);
        assert_eq!(online, expected_online);
    }
}
