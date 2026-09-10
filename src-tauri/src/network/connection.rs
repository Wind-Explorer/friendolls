use std::collections::HashMap;
use std::time::Duration;

use friendolls_common::{
    ClientMessage, DEFAULT_SERVER_PORT, InteractionDeliveryStatus, Profile, ServerMessage,
    friends_bytes, interaction_bytes, message_bytes, profile_bytes, register_bytes,
};
use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Instant, sleep_until, timeout};
use tokio_tungstenite::tungstenite::Message;

use super::presence::FriendPresence;
use super::{
    ConnectionState, Statuses, apply_friend_presence_change, changed, update_friend_presence,
};
use crate::db::AppDatabase;
use crate::friends;
use crate::interactions;
use crate::keypair::AppKeypair;
use crate::remotes::Remote;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const SETUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Probe even when presence suppresses live data. Only the matching pong
/// acknowledges a probe; unrelated traffic must not hide a broken return path.
struct Heartbeat {
    next_check: Instant,
    sequence: u64,
    pending: bool,
}

impl Heartbeat {
    fn new() -> Self {
        Self {
            next_check: Instant::now() + HEARTBEAT_INTERVAL,
            sequence: 0,
            pending: false,
        }
    }

    async fn next_ping(&mut self) -> Result<Message, &'static str> {
        sleep_until(self.next_check).await;
        if self.pending {
            return Err("server heartbeat timed out");
        }
        self.sequence = self.sequence.wrapping_add(1);
        self.pending = true;
        self.next_check = Instant::now() + HEARTBEAT_INTERVAL;
        Ok(Message::Ping(self.sequence.to_be_bytes().to_vec().into()))
    }

    fn pong(&mut self, payload: &[u8]) {
        if payload == self.sequence.to_be_bytes() {
            self.pending = false;
        }
    }
}

pub(super) struct InteractionRequest {
    pub(super) interaction_id: String,
    pub(super) recipient_id: String,
    pub(super) payload: String,
    pub(super) response: oneshot::Sender<InteractionDeliveryStatus>,
}

pub(super) struct ProfileLookupRequest {
    pub(super) request_id: String,
    pub(super) user_id: String,
    pub(super) response: oneshot::Sender<Option<String>>,
}

pub(super) struct SkinLookupRequest {
    pub(super) request_id: String,
    pub(super) user_id: String,
    pub(super) skin_hash: String,
    pub(super) response: oneshot::Sender<Option<String>>,
}

pub(super) struct ConnectionInputs {
    pub(super) profiles: watch::Receiver<crate::user::User>,
    pub(super) friends: watch::Receiver<Vec<String>>,
    pub(super) keypair: AppKeypair,
    pub(super) cursor_data: watch::Receiver<Option<String>>,
    pub(super) activity_data: watch::Receiver<Option<String>>,
    pub(super) interactions: mpsc::Receiver<InteractionRequest>,
    pub(super) profile_lookups: mpsc::Receiver<ProfileLookupRequest>,
    pub(super) skin_lookups: mpsc::Receiver<SkinLookupRequest>,
}

pub(super) async fn run(
    handle: AppHandle,
    statuses: Statuses,
    friend_presence: std::sync::Arc<FriendPresence>,
    remote: Remote,
    generation: u64,
    mut inputs: ConnectionInputs,
) {
    loop {
        changed(
            &handle,
            &statuses,
            &remote,
            generation,
            ConnectionState::Connecting,
        );
        if let Err(error) = connect(
            &handle,
            &statuses,
            &friend_presence,
            &remote,
            generation,
            &mut inputs,
        )
        .await
        {
            eprintln!("remote {} disconnected: {error}", remote.id);
        }
        apply_friend_presence_change(&handle, friend_presence.remove(&remote.id));
        changed(
            &handle,
            &statuses,
            &remote,
            generation,
            ConnectionState::Disconnected,
        );
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn connect(
    handle: &AppHandle,
    statuses: &Statuses,
    friend_presence: &FriendPresence,
    remote: &Remote,
    generation: u64,
    inputs: &mut ConnectionInputs,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ConnectionInputs {
        profiles,
        friends,
        keypair,
        cursor_data,
        activity_data,
        interactions: active_outgoing,
        profile_lookups,
        skin_lookups,
    } = inputs;
    let (socket, _) =
        timeout(SETUP_TIMEOUT, tokio_tungstenite::connect_async(url(remote))).await??;
    let (mut writer, mut reader) = socket.split();

    let challenge = match timeout(SETUP_TIMEOUT, recv(&mut reader)).await?? {
        ServerMessage::Challenge { version, challenge }
            if version == friendolls_common::VERSION =>
        {
            challenge
        }
        _ => return Err("server did not send a compatible challenge".into()),
    };
    let current = profiles.borrow_and_update().clone();
    let registration_profile = Profile {
        id: current.id,
        display_name: current.display_name,
        skin_hash: current.skin_hash,
    };
    let registration_friends = friends.borrow_and_update().clone();
    send(
        &mut writer,
        &ClientMessage::Register {
            signature: keypair.sign(&register_bytes(
                &challenge,
                &registration_profile,
                &registration_friends,
            )),
            profile: registration_profile,
            friends: registration_friends,
        },
    )
    .await?;

    if !matches!(
        timeout(SETUP_TIMEOUT, recv(&mut reader)).await??,
        ServerMessage::Registered
    ) {
        return Err("server rejected registration".into());
    }
    cursor_data.borrow_and_update();
    activity_data.borrow_and_update();
    while let Ok(request) = active_outgoing.try_recv() {
        let _ = request
            .response
            .send(InteractionDeliveryStatus::Unavailable);
    }
    send(&mut writer, &ClientMessage::SyncFriendProfiles).await?;
    send(&mut writer, &ClientMessage::SyncFriendStatuses).await?;
    changed(
        handle,
        statuses,
        remote,
        generation,
        ConnectionState::Connected,
    );

    let mut heartbeat = Heartbeat::new();
    let mut pending_interactions = HashMap::new();
    let mut pending_profile_lookups: HashMap<String, (String, oneshot::Sender<Option<String>>)> =
        HashMap::new();
    let mut pending_skin_lookups: HashMap<
        String,
        (String, String, oneshot::Sender<Option<String>>),
    > = HashMap::new();
    loop {
        tokio::select! {
            ping = heartbeat.next_ping() => {
                send_frame(&mut writer, ping?).await?;
            }
            changed = cursor_data.changed() => {
                changed.map_err(|_| "cursor sender closed")?;
                let payload = { cursor_data.borrow_and_update().clone() };
                if let Some(payload) = payload
                    && friend_presence.has_online_friends(&remote.id)?
                {
                    send(&mut writer, &ClientMessage::Signed {
                        signature: keypair.sign(&message_bytes(&payload)),
                        payload,
                    }).await?;
                }
            }
            changed = activity_data.changed() => {
                changed.map_err(|_| "activity sender closed")?;
                let payload = { activity_data.borrow_and_update().clone() };
                if let Some(payload) = payload
                    && friend_presence.has_online_friends(&remote.id)?
                {
                    send(&mut writer, &ClientMessage::Signed {
                        signature: keypair.sign(&message_bytes(&payload)),
                        payload,
                    }).await?;
                }
            }
            request = active_outgoing.recv() => {
                let request = request.ok_or("interaction sender closed")?;
                let signature = keypair.sign(&interaction_bytes(
                    &request.interaction_id,
                    &request.recipient_id,
                    &request.payload,
                ));
                send(&mut writer, &ClientMessage::Interaction {
                    interaction_id: request.interaction_id.clone(),
                    recipient_id: request.recipient_id,
                    payload: request.payload,
                    signature,
                }).await?;
                pending_interactions.insert(request.interaction_id, request.response);
            }
            request = profile_lookups.recv() => {
                let request = request.ok_or("profile lookup sender closed")?;
                if request.response.is_closed() {
                    continue;
                }
                pending_profile_lookups.retain(|_, (_, response)| !response.is_closed());
                send(&mut writer, &ClientMessage::ResolveProfile {
                    request_id: request.request_id.clone(),
                    user_id: request.user_id.clone(),
                }).await?;
                pending_profile_lookups.insert(
                    request.request_id,
                    (request.user_id, request.response),
                );
            }
            request = skin_lookups.recv() => {
                let request = request.ok_or("skin lookup sender closed")?;
                if request.response.is_closed() {
                    continue;
                }
                pending_skin_lookups.retain(|_, (_, _, response)| !response.is_closed());
                send(&mut writer, &ClientMessage::RequestSkin {
                    request_id: request.request_id.clone(),
                    user_id: request.user_id.clone(),
                    skin_hash: request.skin_hash.clone(),
                }).await?;
                pending_skin_lookups.insert(
                    request.request_id,
                    (request.user_id, request.skin_hash, request.response),
                );
            }
            changed = profiles.changed() => {
                changed.map_err(|_| "profile sender closed")?;
                let current = profiles.borrow_and_update().clone();
                let profile = Profile {
                    id: current.id,
                    display_name: current.display_name,
                    skin_hash: current.skin_hash,
                };
                send(&mut writer, &ClientMessage::ProfileUpdated {
                    signature: keypair.sign(&profile_bytes(&profile)),
                    profile,
                }).await?;
            }
            changed = friends.changed() => {
                changed.map_err(|_| "friends sender closed")?;
                let friends = friends.borrow_and_update().clone();
                send(&mut writer, &ClientMessage::FriendsUpdated {
                    signature: keypair.sign(&friends_bytes(&friends)),
                    friends,
                }).await?;
            }
            message = reader.next() => match message.ok_or("server closed the socket")?? {
                Message::Text(text) => match serde_json::from_str(&text)? {
                    ServerMessage::FriendProfileUpdated { profile } => {
                        if handle.state::<super::Network>().accept_source(&remote.id, &profile.id) {
                            let database = handle.state::<AppDatabase>();
                            if let Err(error) = friends::apply_profile_update(handle, &database, profile).await {
                                eprintln!("failed to update friend profile: {error}");
                            }
                        }
                    }
                    ServerMessage::FriendProfiles { profiles } => {
                        let network = handle.state::<super::Network>();
                        let profiles = profiles
                            .into_iter()
                            .filter(|profile| network.accept_source(&remote.id, &profile.id))
                            .collect();
                        let database = handle.state::<AppDatabase>();
                        if let Err(error) = friends::apply_profile_sync(handle, &database, profiles).await {
                            eprintln!("failed to synchronize friend profiles: {error}");
                        }
                    }
                    ServerMessage::FriendStatusChanged { friend_id, online } => {
                        update_friend_presence(
                            handle,
                            friend_presence,
                            &remote.id,
                            friend_id,
                            online,
                        );
                    }
                    ServerMessage::FriendStatuses { friend_ids } => {
                        apply_friend_presence_change(
                            handle,
                            friend_presence.replace(&remote.id, friend_ids),
                        );
                    }
                    ServerMessage::FriendLiveData { friend_id, payload } => {
                        handle.state::<super::Network>().receive_live_data(
                            handle,
                            friend_id,
                            &payload,
                        );
                    }
                    ServerMessage::FriendInteraction {
                        interaction_id,
                        friend_id,
                        payload,
                    } => {
                        interactions::receive(handle, interaction_id, friend_id, &payload);
                    }
                    ServerMessage::InteractionDelivery { interaction_id, status } => {
                        if let Some(response) = pending_interactions.remove(&interaction_id) {
                            let _ = response.send(status);
                        }
                    }
                    ServerMessage::ProfileResolved { request_id, profile } => {
                        if let Some((user_id, response)) = pending_profile_lookups.remove(&request_id) {
                            let display_name = profile
                                .filter(|profile| profile.id == user_id)
                                .map(|profile| profile.display_name);
                            let _ = response.send(display_name);
                        }
                    }
                    ServerMessage::SkinRequested {
                        request_id,
                        requester_id,
                        skin_hash,
                    } => {
                        let data = profiles
                            .borrow()
                            .skin_hash
                            .as_deref()
                            .filter(|current_hash| *current_hash == skin_hash)
                            .and_then(|_| crate::skins::read_local_base64(handle, &skin_hash));
                        send(&mut writer, &ClientMessage::ProvideSkin {
                            request_id,
                            requester_id,
                            skin_hash,
                            data,
                        }).await?;
                    }
                    ServerMessage::SkinResolved {
                        request_id,
                        user_id,
                        skin_hash,
                        data,
                    } => {
                        if let Some((expected_user_id, expected_hash, response)) =
                            pending_skin_lookups.remove(&request_id)
                        {
                            let data = (user_id == expected_user_id && skin_hash == expected_hash)
                                .then_some(data)
                                .flatten();
                            let _ = response.send(data);
                        }
                    }
                    _ => {}
                },
                Message::Ping(data) => send_frame(&mut writer, Message::Pong(data)).await?,
                Message::Pong(data) => heartbeat.pong(&data),
                Message::Close(_) => return Ok(()),
                _ => {}
            }
        }
    }
}

async fn send<S>(
    writer: &mut S,
    message: &ClientMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    send_frame(
        writer,
        Message::Text(serde_json::to_string(message)?.into()),
    )
    .await
}

async fn send_frame<S>(
    writer: &mut S,
    message: Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    // A blocked write must not prevent the session from detecting disconnection.
    timeout(WRITE_TIMEOUT, writer.send(message)).await??;
    Ok(())
}

async fn recv<S>(reader: &mut S) -> Result<ServerMessage, Box<dyn std::error::Error + Send + Sync>>
where
    S: futures_util::Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    while let Some(message) = reader.next().await {
        if let Message::Text(text) = message? {
            return Ok(serde_json::from_str(&text)?);
        }
    }
    Err("server closed the socket".into())
}

fn url(remote: &Remote) -> String {
    let address = remote.address.trim_end_matches('/');
    let address = address
        .strip_prefix("http://")
        .or_else(|| address.strip_prefix("https://"))
        .or_else(|| address.strip_prefix("ws://"))
        .or_else(|| address.strip_prefix("wss://"))
        .unwrap_or(address);
    let (scheme, default_port) =
        if remote.address.starts_with("https://") || remote.address.starts_with("wss://") {
            ("wss", 443)
        } else if remote.address.starts_with("http://") || remote.address.starts_with("ws://") {
            ("ws", 80)
        } else {
            ("ws", DEFAULT_SERVER_PORT)
        };
    let port = remote.port.unwrap_or(default_port);
    format!("{scheme}://{address}:{port}/v1/ws")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn idle_connection_times_out_without_pong() {
        let start = Instant::now();
        let mut heartbeat = Heartbeat::new();
        assert!(matches!(
            heartbeat.next_ping().await.unwrap(),
            Message::Ping(_)
        ));
        assert_eq!(Instant::now() - start, HEARTBEAT_INTERVAL);
        assert_eq!(
            heartbeat.next_ping().await.unwrap_err(),
            "server heartbeat timed out"
        );
        assert_eq!(Instant::now() - start, HEARTBEAT_INTERVAL * 2);
    }

    #[tokio::test(start_paused = true)]
    async fn stale_or_unsolicited_pongs_do_not_keep_connection_alive() {
        let mut heartbeat = Heartbeat::new();
        let Message::Ping(first) = heartbeat.next_ping().await.unwrap() else {
            panic!("expected ping");
        };
        heartbeat.pong(&first);
        heartbeat.next_ping().await.unwrap();
        heartbeat.pong(&first);
        heartbeat.pong(&[]);
        assert!(heartbeat.next_ping().await.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn websocket_automatic_pongs_keep_idle_connection_alive() {
        use tokio_tungstenite::{WebSocketStream, tungstenite::protocol::Role};

        let (client, server) = tokio::io::duplex(1024);
        let mut client = WebSocketStream::from_raw_socket(client, Role::Client, None).await;
        let mut server = WebSocketStream::from_raw_socket(server, Role::Server, None).await;
        let mut heartbeat = Heartbeat::new();
        for _ in 0..3 {
            send_frame(&mut client, heartbeat.next_ping().await.unwrap())
                .await
                .unwrap();
            assert!(matches!(
                server.next().await.unwrap().unwrap(),
                Message::Ping(_)
            ));
            // Tungstenite queues the server's automatic pong when reading the ping.
            server.flush().await.unwrap();
            let Message::Pong(payload) = client.next().await.unwrap().unwrap() else {
                panic!("expected pong");
            };
            heartbeat.pong(&payload);
        }
        // Simulate a silent network failure after a previously healthy session.
        send_frame(&mut client, heartbeat.next_ping().await.unwrap())
            .await
            .unwrap();
        assert!(heartbeat.next_ping().await.is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn blocked_write_times_out() {
        let mut writer = Box::pin(futures_util::sink::unfold((), |(), _: Message| async {
            std::future::pending::<Result<(), std::io::Error>>().await
        }));
        let start = Instant::now();
        let error = send_frame(&mut writer, Message::Ping(Vec::new().into()))
            .await
            .unwrap_err();
        assert!(error.is::<tokio::time::error::Elapsed>());
        assert_eq!(Instant::now() - start, WRITE_TIMEOUT);
    }

    fn remote(address: &str, port: Option<u16>) -> Remote {
        Remote {
            id: "remote-id".to_owned(),
            address: address.to_owned(),
            name: None,
            port,
            priority: 0,
        }
    }

    #[test]
    fn url_uses_direct_server_port_when_unspecified() {
        assert_eq!(
            url(&remote("example.net", None)),
            "ws://example.net:27520/v1/ws"
        );
    }

    #[test]
    fn url_uses_https_default_port_when_unspecified() {
        assert_eq!(
            url(&remote("https://example.net", None)),
            "wss://example.net:443/v1/ws"
        );
    }

    #[test]
    fn url_uses_http_default_port_when_unspecified() {
        assert_eq!(
            url(&remote("http://example.net", None)),
            "ws://example.net:80/v1/ws"
        );
    }

    #[test]
    fn url_preserves_explicit_port() {
        assert_eq!(
            url(&remote("https://example.net", Some(443))),
            "wss://example.net:443/v1/ws"
        );
    }
}
