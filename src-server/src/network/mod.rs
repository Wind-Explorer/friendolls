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
use wyd_common::{
    ClientMessage, Profile, ServerMessage, friends_bytes, message_bytes, profile_bytes,
    register_bytes,
};

type Clients = Arc<Mutex<HashMap<String, Client>>>;

struct Client {
    connection_id: Uuid,
    key: VerifyingKey,
    #[allow(dead_code)] // Used when presence and profile lookup are exposed.
    profile: Profile,
    #[allow(dead_code)] // Used when friend-authorized message routing is added.
    friends: Vec<String>,
    #[allow(dead_code)] // Used when server-side message routing is added.
    sender: mpsc::Sender<Message>,
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
    clients.lock().await.insert(
        public_key.clone(),
        Client {
            connection_id,
            key,
            profile,
            friends,
            sender,
        },
    );
    if send(&mut socket, &ServerMessage::Registered).await.is_err() {
        remove(&clients, &public_key, connection_id).await;
        return;
    }

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
                    Ok(ClientMessage::ProfileUpdated { profile, signature }) => {
                        if !update_profile(&clients, &public_key, connection_id, profile, &signature).await {
                            break;
                        }
                    }
                    Ok(ClientMessage::FriendsUpdated { friends, signature }) => {
                        if !update_friends(&clients, &public_key, connection_id, friends, &signature).await {
                            break;
                        }
                    }
                    Ok(ClientMessage::SyncFriendProfiles) => {
                        let Some(profiles) = friend_profiles(&clients, &public_key, connection_id).await else {
                            break;
                        };
                        let message = ServerMessage::FriendProfiles { profiles };
                        if writer.send(Message::Text(serde_json::to_string(&message).unwrap().into())).await.is_err() {
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

    remove(&clients, &public_key, connection_id).await;
}

async fn update_profile(
    clients: &Clients,
    public_key: &str,
    connection_id: Uuid,
    profile: Profile,
    signature: &str,
) -> bool {
    let mut clients = clients.lock().await;
    {
        let Some(client) = clients
            .get_mut(public_key)
            .filter(|client| client.connection_id == connection_id)
        else {
            return false;
        };
        if profile.id != public_key || !verify(&client.key, &profile_bytes(&profile), signature) {
            return false;
        }
        client.profile = profile.clone();
    }

    let recipients: Vec<_> = clients
        .values()
        .filter(|client| client.friends.iter().any(|friend| friend == public_key))
        .map(|client| client.sender.clone())
        .collect();
    drop(clients);

    let message = Message::Text(
        serde_json::to_string(&ServerMessage::FriendProfileUpdated { profile })
            .unwrap()
            .into(),
    );
    for recipient in recipients {
        let _ = recipient.send(message.clone()).await;
    }
    true
}

async fn update_friends(
    clients: &Clients,
    public_key: &str,
    connection_id: Uuid,
    friends: Vec<String>,
    signature: &str,
) -> bool {
    let mut clients = clients.lock().await;
    let Some(client) = clients
        .get_mut(public_key)
        .filter(|client| client.connection_id == connection_id)
    else {
        return false;
    };
    if !verify(&client.key, &friends_bytes(&friends), signature) {
        return false;
    }
    client.friends = friends;
    true
}

async fn friend_profiles(
    clients: &Clients,
    public_key: &str,
    connection_id: Uuid,
) -> Option<Vec<Profile>> {
    let clients = clients.lock().await;
    let client = clients
        .get(public_key)
        .filter(|client| client.connection_id == connection_id)?;
    let mut profiles: Vec<_> = client
        .friends
        .iter()
        .filter_map(|friend| clients.get(friend).map(|client| client.profile.clone()))
        .collect();
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    Some(profiles)
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
                && source.friends.iter().any(|friend| friend == *recipient_id)
                && recipient.friends.iter().any(|friend| friend == public_key)
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

async fn remove(clients: &Clients, public_key: &str, connection_id: Uuid) {
    let mut clients = clients.lock().await;
    if clients
        .get(public_key)
        .is_some_and(|client| client.connection_id == connection_id)
    {
        clients.remove(public_key);
    }
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

    use super::*;

    #[test]
    fn verifies_registration_and_message_signatures() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let profile = Profile {
            id: URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes()),
            display_name: "Wind".to_owned(),
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
    async fn authenticated_profile_update_changes_the_registered_client() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
        let connection_id = Uuid::new_v4();
        let (sender, _receiver) = mpsc::channel(1);
        let (friend_sender, mut friend_receiver) = mpsc::channel(1);
        let clients = Clients::default();
        {
            let mut clients = clients.lock().await;
            clients.insert(
                public_key.clone(),
                Client {
                    connection_id,
                    key: signing_key.verifying_key(),
                    profile: Profile {
                        id: public_key.clone(),
                        display_name: "Old".to_owned(),
                    },
                    friends: Vec::new(),
                    sender,
                },
            );
            clients.insert(
                "friend".to_owned(),
                Client {
                    connection_id: Uuid::new_v4(),
                    key: signing_key.verifying_key(),
                    profile: Profile {
                        id: "friend".to_owned(),
                        display_name: "Friend".to_owned(),
                    },
                    friends: vec![public_key.clone()],
                    sender: friend_sender,
                },
            );
        }
        let profile = Profile {
            id: public_key.clone(),
            display_name: "New".to_owned(),
        };
        let signature =
            URL_SAFE_NO_PAD.encode(signing_key.sign(&profile_bytes(&profile)).to_bytes());

        assert!(update_profile(&clients, &public_key, connection_id, profile, &signature,).await);
        assert_eq!(
            clients
                .lock()
                .await
                .get(&public_key)
                .unwrap()
                .profile
                .display_name,
            "New"
        );
        let announcement = friend_receiver
            .recv()
            .await
            .expect("friend receives update");
        let Message::Text(announcement) = announcement else {
            panic!("expected text announcement");
        };
        let ServerMessage::FriendProfileUpdated { profile } =
            serde_json::from_str(&announcement).expect("decode announcement")
        else {
            panic!("expected friend profile update");
        };
        assert_eq!(profile.id, public_key);
        assert_eq!(profile.display_name, "New");

        let stale_profile = Profile {
            id: public_key.clone(),
            display_name: "Stale".to_owned(),
        };
        let stale_signature =
            URL_SAFE_NO_PAD.encode(signing_key.sign(&profile_bytes(&stale_profile)).to_bytes());
        assert!(
            !update_profile(
                &clients,
                &public_key,
                Uuid::new_v4(),
                stale_profile,
                &stale_signature,
            )
            .await
        );
        assert!(friend_receiver.try_recv().is_err());
        assert_eq!(
            clients
                .lock()
                .await
                .get(&public_key)
                .unwrap()
                .profile
                .display_name,
            "New"
        );
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
                },
                friends: Vec::new(),
                sender,
            },
        );
        let friends = vec!["friend-a".to_owned(), "friend-b".to_owned()];
        let signature =
            URL_SAFE_NO_PAD.encode(signing_key.sign(&friends_bytes(&friends)).to_bytes());

        assert!(
            update_friends(
                &clients,
                &public_key,
                connection_id,
                friends.clone(),
                &signature,
            )
            .await
        );
        assert_eq!(
            clients.lock().await.get(&public_key).unwrap().friends,
            friends
        );
    }

    #[tokio::test]
    async fn profile_sync_returns_only_connected_friends_for_the_current_session() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let connection_id = Uuid::new_v4();
        let (sender, _receiver) = mpsc::channel(1);
        let clients = Clients::default();
        let mut registry = clients.lock().await;
        registry.insert(
            "requester".to_owned(),
            Client {
                connection_id,
                key: signing_key.verifying_key(),
                profile: Profile {
                    id: "requester".to_owned(),
                    display_name: "Requester".to_owned(),
                },
                friends: vec![
                    "friend-b".to_owned(),
                    "offline".to_owned(),
                    "friend-a".to_owned(),
                ],
                sender: sender.clone(),
            },
        );
        for (id, display_name) in [("friend-a", "Alice"), ("friend-b", "Bob")] {
            registry.insert(
                id.to_owned(),
                Client {
                    connection_id: Uuid::new_v4(),
                    key: signing_key.verifying_key(),
                    profile: Profile {
                        id: id.to_owned(),
                        display_name: display_name.to_owned(),
                    },
                    friends: Vec::new(),
                    sender: sender.clone(),
                },
            );
        }
        drop(registry);

        let profiles = friend_profiles(&clients, "requester", connection_id)
            .await
            .expect("current session");
        assert_eq!(
            profiles
                .iter()
                .map(|profile| (profile.id.as_str(), profile.display_name.as_str()))
                .collect::<Vec<_>>(),
            [("friend-a", "Alice"), ("friend-b", "Bob")]
        );
        assert!(
            friend_profiles(&clients, "requester", Uuid::new_v4())
                .await
                .is_none()
        );
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
}
