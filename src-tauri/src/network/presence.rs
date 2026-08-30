use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Change {
    pub(super) online: Vec<String>,
    pub(super) came_online: Vec<String>,
    pub(super) went_offline: Vec<String>,
}

#[derive(Default)]
pub(super) struct FriendPresence(Mutex<HashMap<String, HashSet<String>>>);

impl FriendPresence {
    pub(super) fn replace(
        &self,
        remote_id: &str,
        friend_ids: Vec<String>,
    ) -> Result<Option<Change>, String> {
        let mut by_remote = self.0.lock().map_err(|error| error.to_string())?;
        let before = aggregate(&by_remote);
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
        let before = aggregate(&by_remote);
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
        let before = aggregate(&by_remote);
        by_remote.remove(remote_id);
        Ok(diff(before, &by_remote))
    }

    pub(super) fn retain(&self, known_friend_ids: &[String]) -> Result<Option<Change>, String> {
        let known_friend_ids = known_friend_ids.iter().collect::<HashSet<_>>();
        let mut by_remote = self.0.lock().map_err(|error| error.to_string())?;
        let before = aggregate(&by_remote);
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

fn diff(before: HashSet<String>, by_remote: &HashMap<String, HashSet<String>>) -> Option<Change> {
    let after = aggregate(by_remote);
    if before == after {
        return None;
    }
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
    })
}

#[cfg(test)]
mod tests {
    use super::{Change, FriendPresence};

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
            })
        );
        assert!(
            presence
                .replace("remote-b", vec!["friend".to_owned()])
                .unwrap()
                .is_none()
        );
        assert!(presence.remove("remote-a").unwrap().is_none());
        assert_eq!(
            presence.remove("remote-b").unwrap(),
            Some(Change {
                online: Vec::new(),
                came_online: Vec::new(),
                went_offline: vec!["friend".to_owned()],
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
