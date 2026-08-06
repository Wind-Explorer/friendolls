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

type Clients = Arc<Mutex<HashMap<String, Client>>>;

struct Client {
    connection_id: Uuid,
    key: VerifyingKey,
    #[allow(dead_code)] // Used when presence and profile lookup are exposed.
    profile: Profile,
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
    let Ok(ClientMessage::Register { profile, signature }) = serde_json::from_str(&text) else {
        return;
    };
    let Ok(key) = key(&profile.id) else { return };
    if !verify(&key, &register_bytes(&challenge, &profile), &signature) {
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
                Some(Ok(Message::Text(text))) => {
                    let Ok(ClientMessage::Signed { payload, signature }) = serde_json::from_str(&text) else { break };
                    let registered_key = clients.lock().await.get(&public_key)
                        .filter(|client| client.connection_id == connection_id)
                        .map(|client| client.key);
                    let Some(registered_key) = registered_key else { break };
                    if !verify(&registered_key, &message_bytes(&payload), &signature) { break; }
                    // The message is authenticated. Domain routing comes next.
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
        let registration = register_bytes("challenge", &profile);
        let signature = URL_SAFE_NO_PAD.encode(signing_key.sign(&registration).to_bytes());

        assert!(verify(
            &signing_key.verifying_key(),
            &registration,
            &signature
        ));
        assert!(!verify(
            &signing_key.verifying_key(),
            &register_bytes("another challenge", &profile),
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
}
