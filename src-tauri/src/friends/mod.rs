use crate::db::{self, AppDatabase};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::keypair::AppKeypair;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
/// A configured public-key relationship with optional cached remote metadata.
/// The display name remains absent until learned from a signed remote profile.
pub struct Friend {
    pub id: String,
    pub display_name: Option<String>,
    pub skin_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct FriendsChanged {
    pub friends: Vec<Friend>,
}

pub(crate) async fn all(database: &AppDatabase) -> Result<Vec<Friend>, sqlx::Error> {
    sqlx::query_as::<_, Friend>(
        "SELECT id, display_name, skin_hash FROM friends \
         ORDER BY display_name IS NULL, display_name COLLATE NOCASE, id",
    )
    .fetch_all(database.pool())
    .await
}

async fn emit_changed(handle: &AppHandle, database: &AppDatabase) -> Result<(), String> {
    FriendsChanged {
        friends: all(database).await.map_err(db::command_error)?,
    }
    .emit(handle)
    .map_err(db::command_error)
}

async fn update_profiles(
    database: &AppDatabase,
    profiles: &[wyd_common::Profile],
) -> Result<bool, sqlx::Error> {
    let mut transaction = database.pool().begin().await?;
    let mut changed = false;
    for profile in profiles {
        if profile
            .skin_hash
            .as_deref()
            .is_some_and(|hash| !wyd_common::is_skin_hash(hash))
        {
            continue;
        }
        let result = sqlx::query(
            "UPDATE friends SET display_name = ?1, skin_hash = ?2 \
             WHERE id = ?3 AND (display_name IS NOT ?1 OR skin_hash IS NOT ?2)",
        )
        .bind(&profile.display_name)
        .bind(&profile.skin_hash)
        .bind(&profile.id)
        .execute(&mut *transaction)
        .await?;
        changed |= result.rows_affected() > 0;
    }
    transaction.commit().await?;
    Ok(changed)
}

pub(crate) async fn apply_profile_update(
    handle: &AppHandle,
    database: &AppDatabase,
    profile: wyd_common::Profile,
) -> Result<bool, String> {
    apply_profile_sync(handle, database, vec![profile]).await
}

pub(crate) async fn apply_profile_sync(
    handle: &AppHandle,
    database: &AppDatabase,
    profiles: Vec<wyd_common::Profile>,
) -> Result<bool, String> {
    let changed = update_profiles(database, &profiles)
        .await
        .map_err(db::command_error)?;
    if changed {
        emit_changed(handle, database).await?;
    }
    Ok(changed)
}

#[tauri::command]
#[specta::specta]
pub async fn create_friend(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    keypair: State<'_, AppKeypair>,
    id: String,
) -> Result<Friend, String> {
    let id = id.trim().to_owned();
    crate::user::validate_id(&id)?;
    if id == keypair.public_key() {
        return Err("You cannot add your own identification key.".to_owned());
    }

    let friend = Friend {
        id,
        display_name: None,
        skin_hash: None,
    };
    sqlx::query("INSERT INTO friends (id, display_name) VALUES (?1, NULL)")
        .bind(&friend.id)
        .execute(database.pool())
        .await
        .map_err(db::command_error)?;

    emit_changed(&handle, &database).await?;

    Ok(friend)
}

#[tauri::command]
#[specta::specta]
pub async fn list_friends(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
) -> Result<Vec<Friend>, String> {
    let friends = all(&database).await.map_err(db::command_error)?;

    FriendsChanged {
        friends: friends.clone(),
    }
    .emit(&handle)
    .map_err(db::command_error)?;

    Ok(friends)
}

#[tauri::command]
#[specta::specta]
pub async fn get_friend(
    database: State<'_, AppDatabase>,
    id: String,
) -> Result<Option<Friend>, String> {
    sqlx::query_as::<_, Friend>("SELECT id, display_name, skin_hash FROM friends WHERE id = ?1")
        .bind(id)
        .fetch_optional(database.pool())
        .await
        .map_err(db::command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_friend(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    id: String,
) -> Result<bool, String> {
    let result = sqlx::query("DELETE FROM friends WHERE id = ?1")
        .bind(id)
        .execute(database.pool())
        .await
        .map_err(db::command_error)?;

    let changed = result.rows_affected() > 0;

    if changed {
        emit_changed(&handle, &database).await?;
    }

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    async fn database() -> AppDatabase {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect to in-memory SQLite");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run database migrations");
        AppDatabase::new(pool)
    }

    #[tokio::test]
    async fn profile_update_changes_only_an_existing_friend() {
        let database = database().await;
        sqlx::query("INSERT INTO friends (id, display_name) VALUES (?1, ?2)")
            .bind("friend-id")
            .bind("Old")
            .execute(database.pool())
            .await
            .unwrap();

        assert!(
            update_profiles(
                &database,
                &[
                    wyd_common::Profile {
                        id: "friend-id".to_owned(),
                        display_name: "New".to_owned(),
                        skin_hash: Some("a".repeat(64)),
                    },
                    wyd_common::Profile {
                        id: "missing".to_owned(),
                        display_name: "Name".to_owned(),
                        skin_hash: None,
                    },
                ],
            )
            .await
            .unwrap()
        );
        assert!(
            !update_profiles(
                &database,
                &[wyd_common::Profile {
                    id: "friend-id".to_owned(),
                    display_name: "New".to_owned(),
                    skin_hash: Some("a".repeat(64)),
                }],
            )
            .await
            .unwrap()
        );
        assert_eq!(
            all(&database).await.unwrap(),
            [Friend {
                id: "friend-id".to_owned(),
                display_name: Some("New".to_owned()),
                skin_hash: Some("a".repeat(64)),
            }]
        );
    }
}
