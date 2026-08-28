use axum::extract::ws::Message;
use tokio::sync::mpsc;
use uuid::Uuid;
use wyd_common::{Profile, ServerMessage, profile_bytes};

use super::{Client, Clients, verify};

pub(super) async fn update(
    clients: &Clients,
    public_key: &str,
    connection_id: Uuid,
    profile: Profile,
    signature: &str,
) -> bool {
    let mut clients = clients.lock().await;
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
    let recipients = subscribers(&clients, public_key);
    drop(clients);

    send_update(profile, recipients).await;
    true
}

pub(super) async fn broadcast(clients: &Clients, public_key: &str, connection_id: Uuid) {
    let clients = clients.lock().await;
    let Some(profile) = clients
        .get(public_key)
        .filter(|client| client.connection_id == connection_id)
        .map(|client| client.profile.clone())
    else {
        return;
    };
    let recipients = subscribers(&clients, public_key);
    drop(clients);

    send_update(profile, recipients).await;
}

pub(super) async fn snapshot(
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

pub(super) async fn resolve(
    clients: &Clients,
    requester_id: &str,
    connection_id: Uuid,
    user_id: &str,
) -> Option<Option<Profile>> {
    let clients = clients.lock().await;
    clients
        .get(requester_id)
        .filter(|client| client.connection_id == connection_id)?;
    Some(clients.get(user_id).map(|client| client.profile.clone()))
}

fn subscribers(
    clients: &std::collections::HashMap<String, Client>,
    user_id: &str,
) -> Vec<mpsc::Sender<Message>> {
    clients
        .iter()
        .filter(|(id, client)| {
            id.as_str() != user_id && client.friends.iter().any(|friend| friend == user_id)
        })
        .map(|(_, client)| client.sender.clone())
        .collect()
}

async fn send_update(profile: Profile, recipients: Vec<mpsc::Sender<Message>>) {
    let message = Message::Text(
        serde_json::to_string(&ServerMessage::FriendProfileUpdated { profile })
            .expect("friend profile update serializes")
            .into(),
    );
    for recipient in recipients {
        let _ = recipient.send(message.clone()).await;
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn client(
        signing_key: &SigningKey,
        id: &str,
        display_name: &str,
        connection_id: Uuid,
        friends: Vec<String>,
        sender: mpsc::Sender<Message>,
    ) -> Client {
        Client {
            connection_id,
            key: signing_key.verifying_key(),
            profile: Profile {
                id: id.to_owned(),
                display_name: display_name.to_owned(),
            },
            friends,
            sender,
        }
    }

    #[tokio::test]
    async fn authenticated_update_changes_the_registered_client() {
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
                client(
                    &signing_key,
                    &public_key,
                    "Old",
                    connection_id,
                    Vec::new(),
                    sender,
                ),
            );
            clients.insert(
                "friend".to_owned(),
                client(
                    &signing_key,
                    "friend",
                    "Friend",
                    Uuid::new_v4(),
                    vec![public_key.clone()],
                    friend_sender,
                ),
            );
        }
        let profile = Profile {
            id: public_key.clone(),
            display_name: "New".to_owned(),
        };
        let signature =
            URL_SAFE_NO_PAD.encode(signing_key.sign(&profile_bytes(&profile)).to_bytes());

        assert!(update(&clients, &public_key, connection_id, profile, &signature).await);
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
        let Message::Text(announcement) = friend_receiver
            .recv()
            .await
            .expect("friend receives update")
        else {
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
            !update(
                &clients,
                &public_key,
                Uuid::new_v4(),
                stale_profile,
                &stale_signature,
            )
            .await
        );
        assert!(friend_receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn first_connection_announces_to_existing_subscribers() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let public_key = URL_SAFE_NO_PAD.encode(signing_key.verifying_key().to_bytes());
        let connection_id = Uuid::new_v4();
        let (source_sender, _source_receiver) = mpsc::channel(1);
        let (friend_sender, mut friend_receiver) = mpsc::channel(1);
        let (unrelated_sender, mut unrelated_receiver) = mpsc::channel(1);
        let clients = Clients::default();
        {
            let mut clients = clients.lock().await;
            clients.insert(
                public_key.clone(),
                client(
                    &signing_key,
                    &public_key,
                    "Newly connected",
                    connection_id,
                    Vec::new(),
                    source_sender,
                ),
            );
            clients.insert(
                "friend".to_owned(),
                client(
                    &signing_key,
                    "friend",
                    "Friend",
                    Uuid::new_v4(),
                    vec![public_key.clone()],
                    friend_sender,
                ),
            );
            clients.insert(
                "unrelated".to_owned(),
                client(
                    &signing_key,
                    "unrelated",
                    "Unrelated",
                    Uuid::new_v4(),
                    Vec::new(),
                    unrelated_sender,
                ),
            );
        }

        broadcast(&clients, &public_key, connection_id).await;

        let Message::Text(announcement) = friend_receiver
            .recv()
            .await
            .expect("subscriber receives connected profile")
        else {
            panic!("expected text announcement");
        };
        let ServerMessage::FriendProfileUpdated { profile } =
            serde_json::from_str(&announcement).expect("decode announcement")
        else {
            panic!("expected friend profile update");
        };
        assert_eq!(profile.id, public_key);
        assert_eq!(profile.display_name, "Newly connected");
        assert!(unrelated_receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn snapshot_returns_only_connected_friends_for_the_current_session() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let connection_id = Uuid::new_v4();
        let (sender, _receiver) = mpsc::channel(1);
        let clients = Clients::default();
        let mut registry = clients.lock().await;
        registry.insert(
            "requester".to_owned(),
            client(
                &signing_key,
                "requester",
                "Requester",
                connection_id,
                vec![
                    "friend-b".to_owned(),
                    "offline".to_owned(),
                    "friend-a".to_owned(),
                ],
                sender.clone(),
            ),
        );
        for (id, display_name) in [("friend-a", "Alice"), ("friend-b", "Bob")] {
            registry.insert(
                id.to_owned(),
                client(
                    &signing_key,
                    id,
                    display_name,
                    Uuid::new_v4(),
                    Vec::new(),
                    sender.clone(),
                ),
            );
        }
        drop(registry);

        let profiles = snapshot(&clients, "requester", connection_id)
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
            snapshot(&clients, "requester", Uuid::new_v4())
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn lookup_resolves_connected_users_for_only_the_current_session() {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let connection_id = Uuid::new_v4();
        let (sender, _receiver) = mpsc::channel(1);
        let clients = Clients::default();
        {
            let mut clients = clients.lock().await;
            for (id, name, session) in [
                ("requester", "Requester", connection_id),
                ("target", "Resolved name", Uuid::new_v4()),
            ] {
                clients.insert(
                    id.to_owned(),
                    client(&signing_key, id, name, session, Vec::new(), sender.clone()),
                );
            }
        }

        assert_eq!(
            resolve(&clients, "requester", connection_id, "target")
                .await
                .expect("current requester session")
                .expect("connected target")
                .display_name,
            "Resolved name"
        );
        assert!(
            resolve(&clients, "requester", connection_id, "offline")
                .await
                .expect("current requester session")
                .is_none()
        );
        assert!(
            resolve(&clients, "requester", Uuid::new_v4(), "target")
                .await
                .is_none()
        );
    }
}
