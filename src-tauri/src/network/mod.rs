use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;
use tokio::sync::{mpsc, oneshot, watch};
use wyd_common::{InteractionContent, InteractionDeliveryStatus};

use crate::db::AppDatabase;
use crate::friends::{self, FriendsChanged};
use crate::keypair::AppKeypair;
use crate::live_data::LiveData;
use crate::remotes::{self, Remote, RemotesChanged};

mod connection;
mod presence;

use connection::{ConnectionInputs, InteractionRequest, ProfileLookupRequest, SkinLookupRequest};
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
    skin_lookup_sender: mpsc::Sender<SkinLookupRequest>,
    task: tauri::async_runtime::JoinHandle<()>,
    generation: u64,
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
    pub(crate) fn public_key(&self) -> &str {
        self.keypair.public_key()
    }

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

    pub(crate) async fn request_skin(
        &self,
        user_id: String,
        skin_hash: String,
    ) -> Result<Option<Vec<u8>>, String> {
        if user_id == self.keypair.public_key() {
            return Ok(None);
        }

        let senders: Vec<_> = self
            .connections
            .lock()
            .map_err(|error| error.to_string())?
            .values()
            .map(|connection| connection.skin_lookup_sender.clone())
            .collect();
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut responses = Vec::new();
        for sender in senders {
            let (response, receiver) = oneshot::channel();
            if sender
                .try_send(SkinLookupRequest {
                    request_id: request_id.clone(),
                    user_id: user_id.clone(),
                    skin_hash: skin_hash.clone(),
                    response,
                })
                .is_ok()
            {
                responses.push(receiver);
            }
        }

        let mut pending: FuturesUnordered<_> = responses
            .into_iter()
            .map(|response| tokio::time::timeout(Duration::from_secs(3), response))
            .collect();
        while let Some(result) = pending.next().await {
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
            .filter(|(id, connection)| desired.get(*id) != Some(&connection.remote))
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

        for remote in desired.into_values() {
            if connections.contains_key(&remote.id) {
                continue;
            }

            let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
            let (sender, receiver) = mpsc::channel(32);
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
                    live_data: receiver,
                    interactions: interaction_receiver,
                    profile_lookups: profile_lookup_receiver,
                    skin_lookups: skin_lookup_receiver,
                },
            ));
            connections.insert(
                remote.id.clone(),
                Connection {
                    remote,
                    sender,
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
            if !change.came_online.is_empty()
                && let Err(error) = crate::live_data::publish_current(handle)
            {
                eprintln!("failed to publish current live data: {error}");
            }
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
