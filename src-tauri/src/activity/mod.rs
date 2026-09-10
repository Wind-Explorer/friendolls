use std::collections::HashMap;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

const MAX_ACTIVITY_SOURCES: usize = 8;
const MAX_ACTIVITY_SOURCE_BYTES: usize = 64;
const MAX_ACTIVITY_TEXT_BYTES: usize = 512;
const MAX_ACTIVITY_ICON_BYTES: usize = 50_000;
// Reserve space for the live-data envelope inside the shared transport limit.
const MAX_ACTIVITY_SET_BYTES: usize = friendolls_common::MAX_LIVE_DATA_PAYLOAD_BYTES - 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ActivityKind {
    Application,
    Listening,
    Watching,
    Playing,
    Custom,
}

/// Provider-neutral rich presence data for one activity source.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub kind: ActivityKind,
    pub name: Option<String>,
    pub details: Option<String>,
    pub icon: Option<String>,
}

impl Activity {
    fn validate(&self) -> Result<(), String> {
        for (label, value) in [("name", &self.name), ("details", &self.details)] {
            if value
                .as_ref()
                .is_some_and(|value| value.len() > MAX_ACTIVITY_TEXT_BYTES)
            {
                return Err(format!(
                    "activity {label} exceeds {MAX_ACTIVITY_TEXT_BYTES} bytes"
                ));
            }
        }
        if self
            .icon
            .as_ref()
            .is_some_and(|icon| icon.len() > MAX_ACTIVITY_ICON_BYTES)
        {
            return Err(format!(
                "activity icon exceeds {MAX_ACTIVITY_ICON_BYTES} bytes"
            ));
        }
        Ok(())
    }
}

pub type Activities = HashMap<String, Activity>;
type ActivitiesByUser = HashMap<String, Activities>;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ActivitiesChanged {
    pub user_id: String,
    pub activities: Activities,
}

#[derive(Default)]
pub(crate) struct ActivityState(RwLock<ActivitiesByUser>);

fn validate_activities(activities: &Activities) -> Result<(), String> {
    if activities.len() > MAX_ACTIVITY_SOURCES {
        return Err(format!(
            "activity update exceeds {MAX_ACTIVITY_SOURCES} sources"
        ));
    }
    for (source, activity) in activities {
        if source.trim().is_empty() || source.len() > MAX_ACTIVITY_SOURCE_BYTES {
            return Err(format!(
                "activity source must contain 1 to {MAX_ACTIVITY_SOURCE_BYTES} bytes"
            ));
        }
        activity.validate()?;
    }
    if serde_json::to_vec(activities)
        .map_err(|error| error.to_string())?
        .len()
        > MAX_ACTIVITY_SET_BYTES
    {
        return Err(format!(
            "activity update exceeds {MAX_ACTIVITY_SET_BYTES} serialized bytes"
        ));
    }
    Ok(())
}

impl ActivityState {
    fn set_source(
        &self,
        user_id: String,
        source: String,
        activity: Option<Activity>,
    ) -> Result<Activities, String> {
        let mut users = self.0.write().map_err(|error| error.to_string())?;
        let mut current = users.get(&user_id).cloned().unwrap_or_default();
        if let Some(activity) = activity {
            current.insert(source, activity);
        } else {
            current.remove(&source);
        }
        validate_activities(&current)?;
        if current.is_empty() {
            users.remove(&user_id);
        } else {
            users.insert(user_id, current.clone());
        }
        Ok(current)
    }

    fn replace(&self, user_id: String, activities: Activities) -> Result<(), String> {
        validate_activities(&activities)?;
        let mut users = self.0.write().map_err(|error| error.to_string())?;
        if activities.is_empty() {
            users.remove(&user_id);
        } else {
            users.insert(user_id, activities);
        }
        Ok(())
    }

    fn remove_users(&self, user_ids: &[String]) -> Result<Vec<String>, String> {
        let mut users = self.0.write().map_err(|error| error.to_string())?;
        Ok(user_ids
            .iter()
            .filter(|user_id| users.remove(*user_id).is_some())
            .cloned()
            .collect())
    }

    pub(crate) fn snapshot(&self) -> Result<ActivitiesByUser, String> {
        self.0
            .read()
            .map(|activities| activities.clone())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn get(&self, user_id: &str) -> Result<Option<Activities>, String> {
        self.0
            .read()
            .map(|users| users.get(user_id).cloned())
            .map_err(|error| error.to_string())
    }
}

pub fn init(handle: &AppHandle) {
    handle.manage(ActivityState::default());
}

fn emit_changed(handle: &AppHandle, user_id: String, activities: Activities) {
    if let Err(error) = (ActivitiesChanged {
        user_id,
        activities,
    })
    .emit(handle)
    {
        eprintln!("failed to emit activity change: {error}");
    }
}

pub(crate) fn update_local(
    handle: &AppHandle,
    source: impl Into<String>,
    activity: Option<Activity>,
) {
    let user_id = handle
        .state::<crate::keypair::AppKeypair>()
        .public_key()
        .to_owned();
    let activities =
        match handle
            .state::<ActivityState>()
            .set_source(user_id.clone(), source.into(), activity)
        {
            Ok(activities) => activities,
            Err(error) => {
                eprintln!("failed to cache local activity: {error}");
                return;
            }
        };
    emit_changed(handle, user_id, activities.clone());
    handle
        .state::<crate::network::Network>()
        .send_live_data(crate::live_data::LiveData::Activities { activities });
}

pub(crate) fn update_friend(handle: &AppHandle, friend_id: String, activities: Activities) {
    if let Err(error) = handle
        .state::<ActivityState>()
        .replace(friend_id.clone(), activities.clone())
    {
        eprintln!("failed to cache friend activity: {error}");
        return;
    }
    emit_changed(handle, friend_id, activities);
}

pub(crate) fn remove_users(handle: &AppHandle, user_ids: &[String]) {
    let removed = match handle.state::<ActivityState>().remove_users(user_ids) {
        Ok(removed) => removed,
        Err(error) => {
            eprintln!("failed to remove offline activities: {error}");
            return;
        }
    };
    for user_id in removed {
        emit_changed(handle, user_id, Activities::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn activity(name: &str) -> Activity {
        Activity {
            kind: ActivityKind::Application,
            name: Some(name.to_owned()),
            details: None,
            icon: None,
        }
    }

    #[test]
    fn activity_sources_coexist_and_can_be_cleared_independently() {
        let state = ActivityState::default();
        state
            .set_source(
                "user".to_owned(),
                "foreground-app".to_owned(),
                Some(activity("Editor")),
            )
            .unwrap();
        state
            .set_source(
                "user".to_owned(),
                "media".to_owned(),
                Some(activity("Song")),
            )
            .unwrap();

        let activities = state.get("user").unwrap().unwrap();
        assert_eq!(activities.len(), 2);

        state
            .set_source("user".to_owned(), "media".to_owned(), None)
            .unwrap();
        let activities = state.get("user").unwrap().unwrap();
        assert_eq!(activities.len(), 1);
        assert_eq!(activities["foreground-app"].name.as_deref(), Some("Editor"));
    }

    #[test]
    fn rejects_oversized_activity_sets_without_replacing_cached_data() {
        let state = ActivityState::default();
        state
            .replace(
                "user".to_owned(),
                HashMap::from([("foreground-app".to_owned(), activity("Editor"))]),
            )
            .unwrap();

        let oversized = (0..=MAX_ACTIVITY_SOURCES)
            .map(|index| (format!("source-{index}"), activity("Activity")))
            .collect();
        assert!(state.replace("user".to_owned(), oversized).is_err());
        assert_eq!(
            state.get("user").unwrap().unwrap()["foreground-app"]
                .name
                .as_deref(),
            Some("Editor")
        );
    }
}
