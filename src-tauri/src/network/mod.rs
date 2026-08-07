use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;
use wyd_common::{
    ClientMessage, Profile, ServerMessage, message_bytes, profile_bytes, register_bytes,
};

use crate::db::AppDatabase;
use crate::keypair::AppKeypair;
use crate::remotes::{self, Remote, RemotesChanged};

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

struct Connection {
    remote: Remote,
    sender: mpsc::Sender<String>,
    task: tauri::async_runtime::JoinHandle<()>,
    generation: u64,
}

pub struct Network {
    connections: Mutex<HashMap<String, Connection>>,
    statuses: Statuses,
    profile: watch::Sender<crate::user::User>,
    keypair: AppKeypair,
    next_generation: AtomicU64,
}

impl Network {
    #[allow(dead_code)] // Ready for the first domain message sender.
    pub fn send(&self, remote_id: &str, payload: String) -> Result<(), String> {
        let connections = self.connections.lock().map_err(|error| error.to_string())?;
        let sender = connections
            .get(remote_id)
            .map(|connection| &connection.sender)
            .ok_or_else(|| "remote is not configured".to_string())?;
        sender
            .try_send(payload)
            .map_err(|error| format!("remote is not ready: {error}"))
    }

    pub fn update_profile(&self, profile: crate::user::User) {
        self.profile.send_replace(profile);
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
            }
        }

        for remote in desired.into_values() {
            if connections.contains_key(&remote.id) {
                continue;
            }

            let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
            let (sender, receiver) = mpsc::channel(32);
            set_initial(
                &self.statuses,
                &remote,
                generation,
                ConnectionState::Connecting,
            );
            let task = tauri::async_runtime::spawn(run(
                handle.clone(),
                self.statuses.clone(),
                remote.clone(),
                generation,
                self.profile.subscribe(),
                self.keypair.clone(),
                receiver,
            ));
            connections.insert(
                remote.id.clone(),
                Connection {
                    remote,
                    sender,
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
    let network = Network {
        connections: Mutex::new(HashMap::new()),
        statuses: Statuses::default(),
        profile,
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
    Ok(())
}

async fn run(
    handle: AppHandle,
    statuses: Statuses,
    remote: Remote,
    generation: u64,
    mut profiles: watch::Receiver<crate::user::User>,
    keypair: AppKeypair,
    mut outgoing: mpsc::Receiver<String>,
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
            &remote,
            generation,
            &mut profiles,
            &keypair,
            &mut outgoing,
        )
        .await
        {
            eprintln!("remote {} disconnected: {error}", remote.id);
        }
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
    remote: &Remote,
    generation: u64,
    profiles: &mut watch::Receiver<crate::user::User>,
    keypair: &AppKeypair,
    outgoing: &mut mpsc::Receiver<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    send(
        &mut writer,
        &ClientMessage::Register {
            signature: keypair.sign(&register_bytes(&challenge, &registration_profile)),
            profile: registration_profile,
        },
    )
    .await?;

    if !matches!(recv(&mut reader).await?, ServerMessage::Registered) {
        return Err("server rejected registration".into());
    }
    changed(
        handle,
        statuses,
        remote,
        generation,
        ConnectionState::Connected,
    );

    loop {
        tokio::select! {
            payload = outgoing.recv() => {
                let payload = payload.ok_or("network sender closed")?;
                send(&mut writer, &ClientMessage::Signed {
                    signature: keypair.sign(&message_bytes(&payload)),
                    payload,
                }).await?;
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
            message = reader.next() => match message.ok_or("server closed the socket")?? {
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
