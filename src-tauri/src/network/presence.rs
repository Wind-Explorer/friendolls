use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Change {
    pub(super) online: Vec<String>,
    pub(super) came_online: Vec<String>,
    pub(super) went_offline: Vec<String>,
    pub(super) online_changed: bool,
    pub(super) route_added: bool,
}

#[derive(Default)]
pub(super) struct FriendPresence(Mutex<HashMap<String, HashSet<String>>>);

impl FriendPresence {
    pub(super) fn active_remotes(&self) -> Result<HashSet<String>, String> {
        let by_remote = self.0.lock().map_err(|error| error.to_string())?;
        Ok(by_remote.keys().cloned().collect())
    }

    pub(super) fn has_online_friends(&self, remote_id: &str) -> Result<bool, String> {
        let by_remote = self.0.lock().map_err(|error| error.to_string())?;
        Ok(by_remote.contains_key(remote_id))
    }

    pub(super) fn remotes_for(&self, friend_id: &str) -> Result<HashSet<String>, String> {
        let by_remote = self.0.lock().map_err(|error| error.to_string())?;
        Ok(by_remote
            .iter()
            .filter(|(_, friend_ids)| friend_ids.contains(friend_id))
            .map(|(remote_id, _)| remote_id.clone())
            .collect())
    }

    pub(super) fn replace(
        &self,
        remote_id: &str,
        friend_ids: Vec<String>,
    ) -> Result<Option<Change>, String> {
        let mut by_remote = self.0.lock().map_err(|error| error.to_string())?;
        let before = by_remote.clone();
        let friend_ids = friend_ids.into_iter().collect::<HashSet<_>>();
        if friend_ids.is_empty() {
            by_remote.remove(remote_id);
        } else {
            by_remote.insert(remote_id.to_owned(), friend_ids);
        }
        Ok(diff(before, &by_remote))
    }

    pub(super) fn update(
        &self,
        remote_id: &str,
        friend_id: String,
        online: bool,
    ) -> Result<Option<Change>, String> {
        let mut by_remote = self.0.lock().map_err(|error| error.to_string())?;
        let before = by_remote.clone();
        if online {
            by_remote
                .entry(remote_id.to_owned())
                .or_default()
                .insert(friend_id);
        } else if let Some(friend_ids) = by_remote.get_mut(remote_id) {
            friend_ids.remove(&friend_id);
            if friend_ids.is_empty() {
                by_remote.remove(remote_id);
            }
        }
        Ok(diff(before, &by_remote))
    }

    pub(super) fn remove(&self, remote_id: &str) -> Result<Option<Change>, String> {
        let mut by_remote = self.0.lock().map_err(|error| error.to_string())?;
        let before = by_remote.clone();
        by_remote.remove(remote_id);
        Ok(diff(before, &by_remote))
    }

    pub(super) fn retain(&self, known_friend_ids: &[String]) -> Result<Option<Change>, String> {
        let known_friend_ids = known_friend_ids.iter().collect::<HashSet<_>>();
        let mut by_remote = self.0.lock().map_err(|error| error.to_string())?;
        let before = by_remote.clone();
        by_remote.retain(|_, friend_ids| {
            friend_ids.retain(|friend_id| known_friend_ids.contains(friend_id));
            !friend_ids.is_empty()
        });
        Ok(diff(before, &by_remote))
    }

    pub(super) fn snapshot(&self) -> Result<Vec<String>, String> {
        let by_remote = self.0.lock().map_err(|error| error.to_string())?;
        let mut friend_ids = aggregate(&by_remote).into_iter().collect::<Vec<_>>();
        friend_ids.sort_unstable();
        Ok(friend_ids)
    }
}

fn aggregate(by_remote: &HashMap<String, HashSet<String>>) -> HashSet<String> {
    by_remote
        .values()
        .flat_map(|friend_ids| friend_ids.iter().cloned())
        .collect()
}

fn diff(
    before_by_remote: HashMap<String, HashSet<String>>,
    by_remote: &HashMap<String, HashSet<String>>,
) -> Option<Change> {
    if before_by_remote == *by_remote {
        return None;
    }
    let before = aggregate(&before_by_remote);
    let after = aggregate(by_remote);
    let online_changed = before != after;
    let route_added = by_remote.iter().any(|(remote_id, friend_ids)| {
        before_by_remote
            .get(remote_id)
            .is_none_or(|before| !friend_ids.is_subset(before))
    });
    let mut went_offline = before.difference(&after).cloned().collect::<Vec<_>>();
    let mut came_online = after.difference(&before).cloned().collect::<Vec<_>>();
    let mut online = after.into_iter().collect::<Vec<_>>();
    came_online.sort_unstable();
    went_offline.sort_unstable();
    online.sort_unstable();
    Some(Change {
        online,
        came_online,
        went_offline,
        online_changed,
        route_added,
    })
}

#[cfg(test)]
mod tests {
    use super::{Change, FriendPresence};

    #[test]
    fn live_data_routes_follow_each_remotes_online_friends() {
        let presence = FriendPresence::default();
        assert!(presence.active_remotes().unwrap().is_empty());
        assert!(!presence.has_online_friends("remote-a").unwrap());

        presence
            .replace("remote-a", vec!["friend-a".into(), "friend-b".into()])
            .unwrap();
        presence.replace("remote-b", Vec::new()).unwrap();
        assert_eq!(
            presence.active_remotes().unwrap(),
            ["remote-a".to_owned()].into()
        );
        assert!(!presence.has_online_friends("remote-b").unwrap());

        presence
            .update("remote-a", "friend-a".into(), false)
            .unwrap();
        assert!(presence.has_online_friends("remote-a").unwrap());
        presence
            .update("remote-a", "friend-b".into(), false)
            .unwrap();
        assert!(!presence.has_online_friends("remote-a").unwrap());
        assert!(presence.active_remotes().unwrap().is_empty());

        let change = presence
            .update("remote-b", "friend-a".into(), true)
            .unwrap()
            .unwrap();
        assert!(
            change.route_added,
            "resuming must trigger current-state publication"
        );
        assert!(presence.has_online_friends("remote-b").unwrap());
        assert!(!presence.has_online_friends("remote-a").unwrap());
    }

    #[test]
    fn disconnect_snapshot_and_friend_removal_disable_live_data_routes() {
        let presence = FriendPresence::default();
        for remote in ["disconnected", "empty-snapshot", "removed-friend"] {
            presence.update(remote, "friend".into(), true).unwrap();
        }
        presence.remove("disconnected").unwrap();
        presence.replace("empty-snapshot", Vec::new()).unwrap();
        assert!(!presence.has_online_friends("disconnected").unwrap());
        assert!(!presence.has_online_friends("empty-snapshot").unwrap());
        assert!(presence.has_online_friends("removed-friend").unwrap());
        presence.retain(&[]).unwrap();
        assert!(presence.active_remotes().unwrap().is_empty());
        assert!(!presence.has_online_friends("removed-friend").unwrap());

        let change = presence
            .replace("disconnected", vec!["friend".into()])
            .unwrap()
            .unwrap();
        assert!(change.route_added);
        assert!(presence.has_online_friends("disconnected").unwrap());
    }

    #[test]
    fn friend_stays_online_while_any_remote_reports_presence() {
        let presence = FriendPresence::default();
        assert_eq!(
            presence
                .replace("remote-a", vec!["friend".to_owned()])
                .unwrap(),
            Some(Change {
                online: vec!["friend".to_owned()],
                came_online: vec!["friend".to_owned()],
                went_offline: Vec::new(),
                online_changed: true,
                route_added: true,
            })
        );
        assert_eq!(
            presence
                .replace("remote-b", vec!["friend".to_owned()])
                .unwrap(),
            Some(Change {
                online: vec!["friend".to_owned()],
                came_online: Vec::new(),
                went_offline: Vec::new(),
                online_changed: false,
                route_added: true,
            })
        );
        assert_eq!(
            presence.remove("remote-a").unwrap(),
            Some(Change {
                online: vec!["friend".to_owned()],
                came_online: Vec::new(),
                went_offline: Vec::new(),
                online_changed: false,
                route_added: false,
            })
        );
        assert_eq!(
            presence.remove("remote-b").unwrap(),
            Some(Change {
                online: Vec::new(),
                came_online: Vec::new(),
                went_offline: vec!["friend".to_owned()],
                online_changed: true,
                route_added: false,
            })
        );
    }

    #[test]
    fn incremental_updates_and_friend_removal_share_the_same_snapshot() {
        let presence = FriendPresence::default();
        presence
            .replace("remote", vec!["kept".to_owned(), "removed".to_owned()])
            .unwrap();

        let change = presence
            .update("remote", "added".to_owned(), true)
            .unwrap()
            .expect("friend came online");
        assert_eq!(change.online, ["added", "kept", "removed"]);
        assert_eq!(change.came_online, ["added"]);
        assert_eq!(
            presence
                .update("remote", "added".to_owned(), false)
                .unwrap()
                .expect("friend went offline")
                .went_offline,
            ["added"]
        );
        assert_eq!(
            presence
                .retain(&["kept".to_owned()])
                .unwrap()
                .expect("unknown friend was removed")
                .went_offline,
            ["removed"]
        );
        assert_eq!(presence.snapshot().unwrap(), ["kept"]);
    }
}
