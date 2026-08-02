use crate::db::{self, AppDatabase};
use crate::user::User;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct FriendsChanged {
    pub friends: Vec<User>,
}

async fn all(database: &AppDatabase) -> Result<Vec<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT id, display_name FROM friends ORDER BY display_name COLLATE NOCASE, id",
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

#[tauri::command]
#[specta::specta]
pub async fn create_friend(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    friend: User,
) -> Result<User, String> {
    sqlx::query("INSERT INTO friends (id, display_name) VALUES (?1, ?2)")
        .bind(&friend.id)
        .bind(&friend.display_name)
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
) -> Result<Vec<User>, String> {
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
) -> Result<Option<User>, String> {
    sqlx::query_as::<_, User>("SELECT id, display_name FROM friends WHERE id = ?1")
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
