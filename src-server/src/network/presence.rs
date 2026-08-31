use axum::extract::ws::Message;
use friendolls_common::{ServerMessage, friends_bytes};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::{Clients, are_mutual_friends, verify};

pub(super) async fn connected(
    clients: &Clients,
    public_key: &str,
    previous_friends: Option<Vec<String>>,
) {
    if let Some(previous_friends) = previous_friends {
        broadcast_friendship_changes(clients, public_key, &previous_friends).await;
    } else {
        broadcast_status(clients, public_key, true).await;
    }
}

pub(super) async fn update_friends(
    clients: &Clients,
    public_key: &str,
    connection_id: Uuid,
    friends: Vec<String>,
    signature: &str,
) -> Option<Vec<String>> {
    let mut clients = clients.lock().await;
    let previous_friends = {
        let client = clients
            .get_mut(public_key)
            .filter(|client| client.connection_id == connection_id)?;
        if !verify(&client.key, &friends_bytes(&friends), signature) {
            return None;
        }
        std::mem::replace(&mut client.friends, friends.clone())
    };

    let mut notifications = Vec::new();
    let mut online_friend_ids = Vec::new();
    for (friend_id, friend) in clients.iter().filter(|(id, _)| id.as_str() != public_key) {
        let was_visible = mutual_friends(public_key, &previous_friends, friend_id, &friend.friends);
        let is_visible = mutual_friends(public_key, &friends, friend_id, &friend.friends);
        if is_visible {
            online_friend_ids.push(friend_id.clone());
        }
        if was_visible != is_visible {
            notifications.push((friend.sender.clone(), public_key.to_owned(), is_visible));
        }
    }
    drop(clients);

    send_notifications(notifications).await;
    online_friend_ids.sort_unstable();
    Some(online_friend_ids)
}

pub(super) async fn snapshot(
    clients: &Clients,
    public_key: &str,
    connection_id: Uuid,
) -> Option<Vec<String>> {
    let clients = clients.lock().await;
    let source = clients
        .get(public_key)
        .filter(|client| client.connection_id == connection_id)?;
    let mut friend_ids = clients
        .iter()
        .filter(|(friend_id, friend)| {
            friend_id.as_str() != public_key && visible_to(public_key, source, friend_id, friend)
        })
        .map(|(friend_id, _)| friend_id.clone())
        .collect::<Vec<_>>();
    friend_ids.sort_unstable();
    Some(friend_ids)
}

pub(super) async fn broadcast_status(clients: &Clients, public_key: &str, online: bool) {
    let recipients = {
        let clients = clients.lock().await;
        let Some(source) = clients.get(public_key) else {
            return;
        };
        clients
            .iter()
            .filter(|(friend_id, friend)| {
                friend_id.as_str() != public_key
                    && visible_to(public_key, source, friend_id, friend)
            })
            .map(|(_, friend)| friend.sender.clone())
            .collect::<Vec<_>>()
    };
    let message = status_message(public_key.to_owned(), online);
    for recipient in recipients {
        let _ = recipient.send(message.clone()).await;
    }
}

pub(super) async fn disconnected(clients: &Clients, public_key: &str, connection_id: Uuid) {
    let recipients = {
        let mut clients = clients.lock().await;
        if !clients
            .get(public_key)
            .is_some_and(|client| client.connection_id == connection_id)
        {
            return;
        }
        let source = clients
            .remove(public_key)
            .expect("checked registered client");
        clients
            .iter()
            .filter(|(friend_id, friend)| visible_to(public_key, &source, friend_id, friend))
            .map(|(_, friend)| friend.sender.clone())
            .collect::<Vec<_>>()
    };
    let message = status_message(public_key.to_owned(), false);
    for recipient in recipients {
        let _ = recipient.send(message.clone()).await;
    }
}

async fn broadcast_friendship_changes(
    clients: &Clients,
    public_key: &str,
    previous_friends: &[String],
) {
    let notifications = {
        let clients = clients.lock().await;
        let Some(source) = clients.get(public_key) else {
            return;
        };
        clients
            .iter()
            .filter(|(friend_id, _)| friend_id.as_str() != public_key)
            .filter_map(|(friend_id, friend)| {
                let was_visible =
                    mutual_friends(public_key, previous_friends, friend_id, &friend.friends);
                let is_visible = visible_to(public_key, source, friend_id, friend);
                (was_visible != is_visible)
                    .then(|| (friend.sender.clone(), public_key.to_owned(), is_visible))
            })
            .collect::<Vec<_>>()
    };
    send_notifications(notifications).await;
}

async fn send_notifications(notifications: Vec<(mpsc::Sender<Message>, String, bool)>) {
    for (recipient, friend_id, online) in notifications {
        let _ = recipient.send(status_message(friend_id, online)).await;
    }
}

fn visible_to(
    source_id: &str,
    source: &super::Client,
    viewer_id: &str,
    viewer: &super::Client,
) -> bool {
    // Apply the source profile's per-friend online-visibility preference here.
    are_mutual_friends(source_id, source, viewer_id, viewer)
}

fn mutual_friends(
    source_id: &str,
    source_friends: &[String],
    viewer_id: &str,
    viewer_friends: &[String],
) -> bool {
    source_friends.iter().any(|friend| friend == viewer_id)
        && viewer_friends.iter().any(|friend| friend == source_id)
}

fn status_message(friend_id: String, online: bool) -> Message {
    Message::Text(
        serde_json::to_string(&ServerMessage::FriendStatusChanged { friend_id, online })
            .expect("friend status serializes")
            .into(),
    )
}
