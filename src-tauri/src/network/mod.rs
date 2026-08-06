use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use wyd_common::{ClientMessage, Profile, ServerMessage, message_bytes, register_bytes};

use crate::db::AppDatabase;
use crate::keypair::AppKeypair;
use crate::remotes::{self, Remote};

pub struct Network {
    senders: Mutex<HashMap<String, mpsc::Sender<String>>>,
}

impl Network {
    #[allow(dead_code)] // Ready for the first domain message sender.
    pub fn send(&self, remote_id: &str, payload: String) -> Result<(), String> {
        let senders = self.senders.lock().map_err(|error| error.to_string())?;
        let sender = senders
            .get(remote_id)
            .ok_or_else(|| "remote is not configured".to_string())?;
        sender
            .try_send(payload)
            .map_err(|error| format!("remote is not ready: {error}"))
    }
}

pub async fn init(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let database = handle.state::<AppDatabase>();
    let keypair = handle.state::<AppKeypair>().inner().clone();
    let profile = crate::profile::get(&database, keypair.public_key()).await?;
    let remotes = remotes::all(&database).await?;
    let mut senders = HashMap::new();

    for remote in remotes {
        let (sender, receiver) = mpsc::channel(32);
        senders.insert(remote.id.clone(), sender);
        tauri::async_runtime::spawn(run(remote, profile.clone(), keypair.clone(), receiver));
    }

    handle.manage(Network {
        senders: Mutex::new(senders),
    });
    Ok(())
}

async fn run(
    remote: Remote,
    profile: crate::user::User,
    keypair: AppKeypair,
    mut outgoing: mpsc::Receiver<String>,
) {
    loop {
        if let Err(error) = connect(&remote, &profile, &keypair, &mut outgoing).await {
            eprintln!("remote {} disconnected: {error}", remote.id);
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn connect(
    remote: &Remote,
    profile: &crate::user::User,
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
    let profile = Profile {
        id: profile.id.clone(),
        display_name: profile.display_name.clone(),
    };
    send(
        &mut writer,
        &ClientMessage::Register {
            signature: keypair.sign(&register_bytes(&challenge, &profile)),
            profile,
        },
    )
    .await?;

    if !matches!(recv(&mut reader).await?, ServerMessage::Registered) {
        return Err("server rejected registration".into());
    }

    loop {
        tokio::select! {
            payload = outgoing.recv() => {
                let payload = payload.ok_or("network sender closed")?;
                send(&mut writer, &ClientMessage::Signed {
                    signature: keypair.sign(&message_bytes(&payload)),
                    payload,
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
