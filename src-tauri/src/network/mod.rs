use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::stream::FuturesUnordered;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_tungstenite::tungstenite::Message;
use wyd_common::{
    ClientMessage, InteractionContent, InteractionDeliveryStatus, Profile, ServerMessage,
    friends_bytes, interaction_bytes, message_bytes, profile_bytes, register_bytes,
};

use crate::db::AppDatabase;
use crate::friends::{self, FriendsChanged};
use crate::interactions;
use crate::keypair::AppKeypair;
use crate::live_data::LiveData;
use crate::remotes::{self, Remote, RemotesChanged};

mod presence;

use presence::{Change as FriendPresenceChange, FriendPresence};

type Statuses = Arc<Mutex<HashMap<String, (u64, ConnectionStatus)>>>;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionStatus {
    pub remote_id: String,
    pub address: String,
    pub name: Option<String>,
    pub state: ConnectionState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatusChanged {
    pub statuses: Vec<ConnectionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct FriendStatusesChanged {
    pub friend_ids: Vec<String>,
}

struct Connection {
    remote: Remote,
    sender: mpsc::Sender<String>,
    interaction_sender: mpsc::Sender<InteractionRequest>,
    profile_lookup_sender: mpsc::Sender<ProfileLookupRequest>,
    task: tauri::async_runtime::JoinHandle<()>,
    generation: u64,
}

struct InteractionRequest {
    interaction_id: String,
    recipient_id: String,
    payload: String,
    response: oneshot::Sender<InteractionDeliveryStatus>,
}

struct ProfileLookupRequest {
    request_id: String,
    user_id: String,
    response: oneshot::Sender<Option<String>>,
}

struct ConnectionInputs {
    profiles: watch::Receiver<crate::user::User>,
    friends: watch::Receiver<Vec<String>>,
    keypair: AppKeypair,
    live_data: mpsc::Receiver<String>,
    interactions: mpsc::Receiver<InteractionRequest>,
    profile_lookups: mpsc::Receiver<ProfileLookupRequest>,
}

pub struct Network {
    connections: Mutex<HashMap<String, Connection>>,
    statuses: Statuses,
    friend_presence: Arc<FriendPresence>,
    profile: watch::Sender<crate::user::User>,
    friends: watch::Sender<Vec<String>>,
    keypair: AppKeypair,
    next_generation: AtomicU64,
}

impl Network {
    pub fn send_live_data(&self, data: LiveData) {
        let Ok(payload) = serde_json::to_string(&data) else {
            eprintln!("failed to serialize live data");
            return;
        };
        let Ok(connections) = self.connections.lock() else {
            eprintln!("failed to lock remote connections for live data");
            return;
        };
        for connection in connections.values() {
            match connection.sender.try_send(payload.clone()) {
                Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    eprintln!("remote {} cannot accept live data", connection.remote.id);
                }
            }
        }
    }

    pub fn update_profile(&self, profile: crate::user::User) {
        self.profile.send_replace(profile);
    }

    pub async fn send_interaction(
        &self,
        recipient_id: String,
        content: InteractionContent,
    ) -> Result<(), String> {
        if recipient_id == self.keypair.public_key() {
            return Err("Interactions can only be sent to a friend".to_owned());
        }
        let payload = serde_json::to_string(&content).map_err(|error| error.to_string())?;
        let interaction_id = uuid::Uuid::new_v4().to_string();
        let senders: Vec<_> = self
            .connections
            .lock()
            .map_err(|error| error.to_string())?
            .values()
            .map(|connection| connection.interaction_sender.clone())
            .collect();
        if senders.is_empty() {
            return Err("No relay connections are configured".to_owned());
        }

        let mut responses = Vec::new();
        for sender in senders {
            let (response, receiver) = oneshot::channel();
            let request = InteractionRequest {
                interaction_id: interaction_id.clone(),
                recipient_id: recipient_id.clone(),
                payload: payload.clone(),
                response,
            };
            if sender.try_send(request).is_ok() {
                responses.push(receiver);
            }
        }
        if responses.is_empty() {
            return Err("Relay connections are busy or disconnected".to_owned());
        }

        let mut pending: FuturesUnordered<_> = responses
            .into_iter()
            .map(|response| tokio::time::timeout(Duration::from_secs(5), response))
            .collect();
        let mut statuses = Vec::new();
        while let Some(result) = pending.next().await {
            if let Ok(Ok(status)) = result {
                if status == InteractionDeliveryStatus::Delivered {
                    return Ok(());
                }
                statuses.push(status);
            }
        }
        if statuses.contains(&InteractionDeliveryStatus::Busy) {
            return Err("Friend is busy; try again".to_owned());
        }
        if statuses.contains(&InteractionDeliveryStatus::Rejected) {
            return Err("Relay rejected the interaction".to_owned());
        }
        Err("Friend is no longer available".to_owned())
    }

    pub async fn resolve_profile(&self, user_id: String) -> Result<Option<String>, String> {
        crate::user::validate_id(&user_id)?;
        if user_id == self.keypair.public_key() {
            return Err("You cannot add your own identification key.".to_owned());
        }

        let senders: Vec<_> = self
            .connections
            .lock()
            .map_err(|error| error.to_string())?
            .values()
            .map(|connection| connection.profile_lookup_sender.clone())
            .collect();
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut responses = Vec::new();
        for sender in senders {
            let (response, receiver) = oneshot::channel();
            let request = ProfileLookupRequest {
                request_id: request_id.clone(),
                user_id: user_id.clone(),
                response,
            };
            if sender.try_send(request).is_ok() {
                responses.push(receiver);
            }
        }

        let mut pending: FuturesUnordered<_> = responses
            .into_iter()
            .map(|response| tokio::time::timeout(Duration::from_secs(3), response))
            .collect();
        while let Some(result) = pending.next().await {
            if let Ok(Ok(Some(display_name))) = result {
                return Ok(Some(display_name));
            }
        }
        Ok(None)
    }

    fn sync_remotes(&self, handle: &AppHandle, remotes: Vec<Remote>) -> Result<(), String> {
        let desired: HashMap<_, _> = remotes
            .into_iter()
            .map(|remote| (remote.id.clone(), remote))
            .collect();
        let mut connections = self.connections.lock().map_err(|error| error.to_string())?;

        let stale: Vec<_> = connections
            .iter()
            .filter(|(id, connection)| desired.get(*id) != Some(&connection.remote))
            .map(|(id, _)| id.clone())
            .collect();

        for id in stale {
            if let Some(connection) = connections.remove(&id) {
                connection.task.abort();
                remove_status(&self.statuses, &id, connection.generation);
                apply_friend_presence_change(handle, self.friend_presence.remove(&id));
            }
        }

        for remote in desired.into_values() {
            if connections.contains_key(&remote.id) {
                continue;
            }

            let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
            let (sender, receiver) = mpsc::channel(32);
            let (interaction_sender, interaction_receiver) = mpsc::channel(16);
            let (profile_lookup_sender, profile_lookup_receiver) = mpsc::channel(16);
            set_initial(
                &self.statuses,
                &remote,
                generation,
                ConnectionState::Connecting,
            );
            let task = tauri::async_runtime::spawn(run(
                handle.clone(),
                self.statuses.clone(),
                self.friend_presence.clone(),
                remote.clone(),
                generation,
                ConnectionInputs {
                    profiles: self.profile.subscribe(),
                    friends: self.friends.subscribe(),
                    keypair: self.keypair.clone(),
                    live_data: receiver,
                    interactions: interaction_receiver,
                    profile_lookups: profile_lookup_receiver,
                },
            ));
            connections.insert(
                remote.id.clone(),
                Connection {
                    remote,
                    sender,
                    interaction_sender,
                    profile_lookup_sender,
                    task,
                    generation,
                },
            );
        }

        drop(connections);
        emit_statuses(handle, &self.statuses)
    }
}

pub async fn init(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let database = handle.state::<AppDatabase>();
    let keypair = handle.state::<AppKeypair>().inner().clone();
    let profile = crate::profile::get(&database, keypair.public_key()).await?;
    let (profile, _) = watch::channel(profile);
    let friends = friend_ids(friends::all(&database).await?, keypair.public_key());
    let (friends, _) = watch::channel(friends);
    let network = Network {
        connections: Mutex::new(HashMap::new()),
        statuses: Statuses::default(),
        friend_presence: Arc::default(),
        profile,
        friends,
        keypair,
        next_generation: AtomicU64::new(1),
    };
    network.sync_remotes(handle, remotes::all(&database).await?)?;
    handle.manage(network);

    let listener_handle = handle.clone();
    RemotesChanged::listen(handle, move |event| {
        let network = listener_handle.state::<Network>();
        if let Err(error) = network.sync_remotes(&listener_handle, event.payload.remotes) {
            eprintln!("failed to synchronize remote connections: {error}");
        }
    });

    let listener_handle = handle.clone();
    FriendsChanged::listen(handle, move |event| {
        let network = listener_handle.state::<Network>();
        let ids = friend_ids(event.payload.friends, network.keypair.public_key());
        network.friends.send_if_modified(|current| {
            if *current == ids {
                return false;
            }
            *current = ids.clone();
            true
        });
        apply_friend_presence_change(&listener_handle, network.friend_presence.retain(&ids));
    });
    Ok(())
}

async fn run(
    handle: AppHandle,
    statuses: Statuses,
    friend_presence: Arc<FriendPresence>,
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
        live_data,
        interactions: active_outgoing,
        profile_lookups,
    } = inputs;
    let (socket, _) = tokio_tungstenite::connect_async(url(remote)).await?;
    let (mut writer, mut reader) = socket.split();

    let challenge = match recv(&mut reader).await? {
        ServerMessage::Challenge { version, challenge } if version == wyd_common::VERSION => {
            challenge
        }
        _ => return Err("server did not send a compatible challenge".into()),
    };
    let current = profiles.borrow_and_update().clone();
    let registration_profile = Profile {
        id: current.id,
        display_name: current.display_name,
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

    if !matches!(recv(&mut reader).await?, ServerMessage::Registered) {
        return Err("server rejected registration".into());
    }
    while live_data.try_recv().is_ok() {}
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

    let mut pending_interactions = HashMap::new();
    let mut pending_profile_lookups: HashMap<String, (String, oneshot::Sender<Option<String>>)> =
        HashMap::new();
    loop {
        tokio::select! {
            payload = live_data.recv() => {
                let payload = payload.ok_or("network sender closed")?;
                send(&mut writer, &ClientMessage::Signed {
                    signature: keypair.sign(&message_bytes(&payload)),
                    payload,
                }).await?;
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
            changed = profiles.changed() => {
                changed.map_err(|_| "profile sender closed")?;
                let current = profiles.borrow_and_update().clone();
                let profile = Profile {
                    id: current.id,
                    display_name: current.display_name,
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
                        let database = handle.state::<AppDatabase>();
                        if let Err(error) = friends::apply_profile_update(handle, &database, profile).await {
                            eprintln!("failed to update friend profile: {error}");
                        }
                    }
                    ServerMessage::FriendProfiles { profiles } => {
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
                        match serde_json::from_str(&payload) {
                            Ok(LiveData::Cursor { positions }) => {
                                crate::cursor::emit_position(handle, friend_id, positions);
                            }
                            Ok(LiveData::ForegroundApp { meta }) => {
                                crate::ufa::emit_friend_app(handle, friend_id, meta);
                            }
                            Err(error) => {
                                eprintln!("failed to decode friend live data: {error}");
                            }
                        }
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
                    _ => {}
                },
                Message::Ping(data) => writer.send(Message::Pong(data)).await?,
                Message::Close(_) => return Ok(()),
                _ => {}
            }
        }
    }
}

#[tauri::command]
#[specta::specta]
pub fn list_statuses(
    handle: AppHandle,
    network: State<'_, Network>,
) -> Result<Vec<ConnectionStatus>, String> {
    let statuses = snapshot(&network.statuses)?;
    NetworkStatusChanged {
        statuses: statuses.clone(),
    }
    .emit(&handle)
    .map_err(|error| error.to_string())?;
    Ok(statuses)
}

#[tauri::command]
#[specta::specta]
pub fn list_friend_statuses(network: State<'_, Network>) -> Result<Vec<String>, String> {
    network.friend_presence.snapshot()
}

#[tauri::command]
#[specta::specta]
pub async fn resolve_friend_display_name(
    user_id: String,
    network: State<'_, Network>,
) -> Result<Option<String>, String> {
    network.resolve_profile(user_id.trim().to_owned()).await
}

fn update_friend_presence(
    handle: &AppHandle,
    presence: &FriendPresence,
    remote_id: &str,
    friend_id: String,
    online: bool,
) {
    apply_friend_presence_change(handle, presence.update(remote_id, friend_id, online));
}

fn apply_friend_presence_change(
    handle: &AppHandle,
    result: Result<Option<FriendPresenceChange>, String>,
) {
    match result {
        Ok(Some(change)) => {
            crate::cursor::remove_positions(handle, &change.went_offline);
            if let Err(error) = (FriendStatusesChanged {
                friend_ids: change.online,
            })
            .emit(handle)
            {
                eprintln!("failed to emit friend statuses: {error}");
            }
        }
        Ok(None) => {}
        Err(error) => eprintln!("failed to update friend presence: {error}"),
    }
}

fn changed(
    handle: &AppHandle,
    statuses: &Statuses,
    remote: &Remote,
    generation: u64,
    state: ConnectionState,
) {
    if set(statuses, remote, generation, state) {
        let _ = emit_statuses(handle, statuses);
    }
}

fn set_initial(statuses: &Statuses, remote: &Remote, generation: u64, state: ConnectionState) {
    if let Ok(mut statuses) = statuses.lock() {
        statuses.insert(remote.id.clone(), (generation, status(remote, state)));
    }
}

fn set(statuses: &Statuses, remote: &Remote, generation: u64, state: ConnectionState) -> bool {
    let Ok(mut statuses) = statuses.lock() else {
        return false;
    };
    let Some((current_generation, current)) = statuses.get_mut(&remote.id) else {
        return false;
    };
    if *current_generation != generation {
        return false;
    }
    *current = status(remote, state);
    true
}

fn remove_status(statuses: &Statuses, remote_id: &str, generation: u64) {
    if let Ok(mut statuses) = statuses.lock()
        && statuses
            .get(remote_id)
            .is_some_and(|(current, _)| *current == generation)
    {
        statuses.remove(remote_id);
    }
}

fn status(remote: &Remote, state: ConnectionState) -> ConnectionStatus {
    ConnectionStatus {
        remote_id: remote.id.clone(),
        address: remote.address.clone(),
        name: remote.name.clone(),
        state,
    }
}

fn emit_statuses(handle: &AppHandle, statuses: &Statuses) -> Result<(), String> {
    NetworkStatusChanged {
        statuses: snapshot(statuses)?,
    }
    .emit(handle)
    .map_err(|error| error.to_string())
}

fn snapshot(statuses: &Statuses) -> Result<Vec<ConnectionStatus>, String> {
    let mut statuses: Vec<_> = statuses
        .lock()
        .map_err(|error| error.to_string())?
        .values()
        .map(|(_, status)| status.clone())
        .collect();
    statuses.sort_by(|a, b| a.remote_id.cmp(&b.remote_id));
    Ok(statuses)
}

async fn send<S>(
    writer: &mut S,
    message: &ClientMessage,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    writer
        .send(Message::Text(serde_json::to_string(message)?.into()))
        .await?;
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
    let scheme = if remote.address.starts_with("https://") || remote.address.starts_with("wss://") {
        "wss"
    } else {
        "ws"
    };
    let port = remote
        .port
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    format!("{scheme}://{address}{port}/v1/ws")
}

fn friend_ids(friends: Vec<crate::friends::Friend>, own_id: &str) -> Vec<String> {
    let mut ids: Vec<_> = friends
        .into_iter()
        .map(|friend| friend.id)
        .filter(|id| id != own_id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[cfg(test)]
mod tests {
    use super::friend_ids;
    use crate::friends::Friend;

    #[test]
    fn friend_ids_discards_display_names_and_normalizes_ids() {
        let friends = vec![
            Friend {
                id: "friend-b".to_owned(),
                display_name: Some("Old name".to_owned()),
            },
            Friend {
                id: "self".to_owned(),
                display_name: None,
            },
            Friend {
                id: "friend-a".to_owned(),
                display_name: Some("Any name".to_owned()),
            },
        ];

        assert_eq!(friend_ids(friends, "self"), ["friend-a", "friend-b"]);
    }
}
