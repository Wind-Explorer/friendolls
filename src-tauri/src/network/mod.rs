use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use friendolls_common::{InteractionContent, InteractionDeliveryStatus};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;
use tokio::sync::{mpsc, oneshot, watch};

use crate::db::AppDatabase;
use crate::friends::{self, FriendsChanged};
use crate::keypair::AppKeypair;
use crate::live_data::{LiveData, LiveDataEnvelope, LiveDataKind};
use crate::remotes::{self, Remote, RemotesChanged};

mod connection;
mod presence;
mod routing;

use connection::{ConnectionInputs, InteractionRequest, ProfileLookupRequest, SkinLookupRequest};
use presence::{Change as FriendPresenceChange, FriendPresence};
use routing::SequenceTracker;

type Statuses = Arc<Mutex<HashMap<String, (u64, ConnectionStatus)>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
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
    cursor_sender: watch::Sender<Option<String>>,
    foreground_app_sender: watch::Sender<Option<String>>,
    interaction_sender: mpsc::Sender<InteractionRequest>,
    profile_lookup_sender: mpsc::Sender<ProfileLookupRequest>,
    skin_lookup_sender: mpsc::Sender<SkinLookupRequest>,
    task: tauri::async_runtime::JoinHandle<()>,
    generation: u64,
}

#[derive(Clone)]
struct ConnectionSenders {
    remote_id: String,
    priority: i32,
    interaction: mpsc::Sender<InteractionRequest>,
    profile_lookup: mpsc::Sender<ProfileLookupRequest>,
    skin_lookup: mpsc::Sender<SkinLookupRequest>,
}

pub struct Network {
    connections: Mutex<HashMap<String, Connection>>,
    statuses: Statuses,
    friend_presence: Arc<FriendPresence>,
    profile: watch::Sender<crate::user::User>,
    friends: watch::Sender<Vec<String>>,
    keypair: AppKeypair,
    next_generation: AtomicU64,
    live_session_id: String,
    next_cursor_sequence: AtomicU64,
    next_foreground_app_sequence: AtomicU64,
    received_sequences: SequenceTracker,
}

impl Network {
    pub(crate) fn public_key(&self) -> &str {
        self.keypair.public_key()
    }

    pub fn send_live_data(&self, data: LiveData) {
        let active_remotes = match self.friend_presence.active_remotes() {
            Ok(remotes) if remotes.is_empty() => return,
            Ok(remotes) => remotes,
            Err(error) => {
                eprintln!("failed to read friend presence for live data: {error}");
                return;
            }
        };
        let sequence = match data.kind() {
            LiveDataKind::Cursor => self.next_cursor_sequence.fetch_add(1, Ordering::Relaxed),
            LiveDataKind::ForegroundApp => self
                .next_foreground_app_sequence
                .fetch_add(1, Ordering::Relaxed),
        };
        let envelope = LiveDataEnvelope {
            session_id: self.live_session_id.clone(),
            sequence,
            data,
        };
        let Ok(payload) = serde_json::to_string(&envelope) else {
            eprintln!("failed to serialize live data");
            return;
        };
        let Ok(connections) = self.connections.lock() else {
            eprintln!("failed to lock remote connections for live data");
            return;
        };
        for (remote_id, connection) in connections.iter() {
            if !active_remotes.contains(remote_id) {
                continue;
            }
            match envelope.kind() {
                LiveDataKind::Cursor => {
                    connection.cursor_sender.send_replace(Some(payload.clone()));
                }
                LiveDataKind::ForegroundApp => {
                    connection
                        .foreground_app_sender
                        .send_replace(Some(payload.clone()));
                }
            }
        }
    }

    fn preferred_remote(
        &self,
        friend_id: &str,
        incoming_remote_id: Option<&str>,
    ) -> Option<String> {
        let mut remote_ids = self.friend_presence.remotes_for(friend_id).ok()?;
        if let Some(remote_id) = incoming_remote_id {
            remote_ids.insert(remote_id.to_owned());
        }
        let connections = self.connections.lock().ok()?;
        let priorities = connections
            .iter()
            .map(|(remote_id, connection)| (remote_id.clone(), connection.remote.priority))
            .collect();
        routing::preferred_remote(&remote_ids, &priorities)
    }

    pub(super) fn accept_source(&self, remote_id: &str, friend_id: &str) -> bool {
        self.preferred_remote(friend_id, Some(remote_id))
            .is_some_and(|preferred| preferred == remote_id)
    }

    fn receive_live_data(&self, handle: &AppHandle, friend_id: String, payload: &str) {
        let envelope = match serde_json::from_str::<LiveDataEnvelope>(payload) {
            Ok(envelope) => envelope,
            Err(error) => {
                eprintln!("failed to decode friend live data: {error}");
                return;
            }
        };
        match self.received_sequences.accept(&friend_id, &envelope) {
            Ok(true) => {}
            Ok(false) => return,
            Err(error) => {
                eprintln!("failed to track friend live data: {error}");
                return;
            }
        }
        match envelope.data {
            LiveData::Cursor { positions } => {
                crate::cursor::emit_position(handle, friend_id, positions);
            }
            LiveData::ForegroundApp { meta } => {
                crate::ufa::emit_friend_app(handle, friend_id, meta);
            }
        }
    }

    pub fn update_profile(&self, profile: crate::user::User) {
        self.profile.send_replace(profile);
    }

    fn ordered_senders(&self, friend_id: Option<&str>) -> Result<Vec<ConnectionSenders>, String> {
        let routes = friend_id
            .map(|friend_id| self.friend_presence.remotes_for(friend_id))
            .transpose()?
            .unwrap_or_default();
        let connections = self.connections.lock().map_err(|error| error.to_string())?;
        let statuses = self.statuses.lock().map_err(|error| error.to_string())?;
        let mut senders = connections
            .values()
            .filter(|connection| {
                statuses
                    .get(&connection.remote.id)
                    .is_some_and(|(_, status)| status.state == ConnectionState::Connected)
            })
            .filter(|connection| routes.is_empty() || routes.contains(&connection.remote.id))
            .map(|connection| ConnectionSenders {
                remote_id: connection.remote.id.clone(),
                priority: connection.remote.priority,
                interaction: connection.interaction_sender.clone(),
                profile_lookup: connection.profile_lookup_sender.clone(),
                skin_lookup: connection.skin_lookup_sender.clone(),
            })
            .collect::<Vec<_>>();
        senders.sort_by(|left, right| {
            (left.priority, &left.remote_id).cmp(&(right.priority, &right.remote_id))
        });
        Ok(senders)
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
        let senders = self.ordered_senders(Some(&recipient_id))?;
        if senders.is_empty() {
            return Err("No connected relay can currently reach this friend".to_owned());
        }

        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut statuses = Vec::new();
        for sender in senders {
            let (response, receiver) = oneshot::channel();
            let request = InteractionRequest {
                interaction_id: interaction_id.clone(),
                recipient_id: recipient_id.clone(),
                payload: payload.clone(),
                response,
            };
            if sender.interaction.try_send(request).is_err() {
                continue;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let result =
                tokio::time::timeout(remaining.min(Duration::from_secs(2)), receiver).await;
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

        let senders = self.ordered_senders(None)?;
        let request_id = uuid::Uuid::new_v4().to_string();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        for sender in senders {
            let (response, receiver) = oneshot::channel();
            let request = ProfileLookupRequest {
                request_id: request_id.clone(),
                user_id: user_id.clone(),
                response,
            };
            if sender.profile_lookup.try_send(request).is_err() {
                continue;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let result =
                tokio::time::timeout(remaining.min(Duration::from_secs(1)), receiver).await;
            if let Ok(Ok(Some(display_name))) = result {
                return Ok(Some(display_name));
            }
        }
        Ok(None)
    }

    pub(crate) async fn request_skin(
        &self,
        user_id: String,
        skin_hash: String,
    ) -> Result<Option<Vec<u8>>, String> {
        if user_id == self.keypair.public_key() {
            return Ok(None);
        }

        let senders = self.ordered_senders(Some(&user_id))?;
        let request_id = uuid::Uuid::new_v4().to_string();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        for sender in senders {
            let (response, receiver) = oneshot::channel();
            if sender
                .skin_lookup
                .try_send(SkinLookupRequest {
                    request_id: request_id.clone(),
                    user_id: user_id.clone(),
                    skin_hash: skin_hash.clone(),
                    response,
                })
                .is_err()
            {
                continue;
            }
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }
            let result =
                tokio::time::timeout(remaining.min(Duration::from_secs(1)), receiver).await;
            if let Ok(Ok(Some(data))) = result
                && let Some(bytes) = crate::skins::decode_response(&data, &skin_hash)
            {
                return Ok(Some(bytes));
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
            .filter(|(id, connection)| {
                desired
                    .get(*id)
                    .is_none_or(|remote| !same_connection_configuration(remote, &connection.remote))
            })
            .map(|(id, _)| id.clone())
            .collect();
        let mut presence_changes = Vec::new();

        for id in stale {
            if let Some(connection) = connections.remove(&id) {
                connection.task.abort();
                remove_status(&self.statuses, &id, connection.generation);
                presence_changes.push(self.friend_presence.remove(&id));
            }
        }

        let mut priority_changed = false;
        for remote in desired.into_values() {
            if let Some(connection) = connections.get_mut(&remote.id) {
                priority_changed |= connection.remote.priority != remote.priority;
                connection.remote = remote;
                continue;
            }

            let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
            let (cursor_sender, cursor_receiver) = watch::channel(None);
            let (foreground_app_sender, foreground_app_receiver) = watch::channel(None);
            let (interaction_sender, interaction_receiver) = mpsc::channel(16);
            let (profile_lookup_sender, profile_lookup_receiver) = mpsc::channel(16);
            let (skin_lookup_sender, skin_lookup_receiver) = mpsc::channel(16);
            set_initial(
                &self.statuses,
                &remote,
                generation,
                ConnectionState::Connecting,
            );
            let task = tauri::async_runtime::spawn(connection::run(
                handle.clone(),
                self.statuses.clone(),
                self.friend_presence.clone(),
                remote.clone(),
                generation,
                ConnectionInputs {
                    profiles: self.profile.subscribe(),
                    friends: self.friends.subscribe(),
                    keypair: self.keypair.clone(),
                    cursor_data: cursor_receiver,
                    foreground_app_data: foreground_app_receiver,
                    interactions: interaction_receiver,
                    profile_lookups: profile_lookup_receiver,
                    skin_lookups: skin_lookup_receiver,
                },
            ));
            connections.insert(
                remote.id.clone(),
                Connection {
                    remote,
                    cursor_sender,
                    foreground_app_sender,
                    interaction_sender,
                    profile_lookup_sender,
                    skin_lookup_sender,
                    task,
                    generation,
                },
            );
        }

        drop(connections);
        for change in presence_changes {
            apply_friend_presence_change(handle, change);
        }
        if priority_changed && let Err(error) = crate::live_data::publish_current(handle) {
            eprintln!("failed to republish live data after server reorder: {error}");
        }
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
        live_session_id: uuid::Uuid::new_v4().to_string(),
        next_cursor_sequence: AtomicU64::new(1),
        next_foreground_app_sequence: AtomicU64::new(1),
        received_sequences: SequenceTracker::default(),
    };
    handle.manage(network);
    handle
        .state::<Network>()
        .sync_remotes(handle, remotes::all(&database).await?)?;

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
        if let Err(error) = network.received_sequences.retain(&ids) {
            eprintln!("failed to prune live-data sequences: {error}");
        }
        apply_friend_presence_change(&listener_handle, network.friend_presence.retain(&ids));
    });
    Ok(())
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
            if let Err(error) = handle
                .state::<crate::ufa::ForegroundAppState>()
                .remove(&change.went_offline)
            {
                eprintln!("failed to remove offline foreground apps: {error}");
            }
            if change.route_added
                && let Err(error) = crate::live_data::publish_current(handle)
            {
                eprintln!("failed to publish current live data: {error}");
            }
            if change.online_changed
                && let Err(error) = (FriendStatusesChanged {
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

fn same_connection_configuration(left: &Remote, right: &Remote) -> bool {
    left.id == right.id
        && left.address == right.address
        && left.name == right.name
        && left.port == right.port
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
                skin_hash: None,
            },
            Friend {
                id: "self".to_owned(),
                display_name: None,
                skin_hash: None,
            },
            Friend {
                id: "friend-a".to_owned(),
                display_name: Some("Any name".to_owned()),
                skin_hash: None,
            },
        ];

        assert_eq!(friend_ids(friends, "self"), ["friend-a", "friend-b"]);
    }
}
