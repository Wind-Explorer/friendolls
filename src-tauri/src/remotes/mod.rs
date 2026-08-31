use crate::db::{self, AppDatabase};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Remote {
    pub id: String,
    pub address: String,
    pub name: Option<String>,
    pub port: Option<u16>,
    pub priority: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInput {
    pub address: String,
    pub name: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct RemotesChanged {
    pub remotes: Vec<Remote>,
}

pub async fn all(database: &AppDatabase) -> Result<Vec<Remote>, sqlx::Error> {
    sqlx::query_as::<_, Remote>(
        "SELECT id, address, name, port, priority FROM remotes \
         ORDER BY priority, id",
    )
    .fetch_all(database.pool())
    .await
}

async fn get(database: &AppDatabase, id: &str) -> Result<Option<Remote>, sqlx::Error> {
    sqlx::query_as::<_, Remote>(
        "SELECT id, address, name, port, priority FROM remotes WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(database.pool())
    .await
}

async fn insert(database: &AppDatabase, input: RemoteInput) -> Result<Remote, sqlx::Error> {
    let id = Uuid::new_v4().to_string();
    let priority = sqlx::query_scalar::<_, i32>(
        "INSERT INTO remotes (id, address, name, port, priority) \
         SELECT ?1, ?2, ?3, ?4, COALESCE(MAX(priority) + 1, 0) FROM remotes \
         RETURNING priority",
    )
    .bind(&id)
    .bind(&input.address)
    .bind(&input.name)
    .bind(input.port)
    .fetch_one(database.pool())
    .await?;
    let remote = Remote {
        id,
        address: input.address,
        name: input.name,
        port: input.port,
        priority,
    };

    Ok(remote)
}

async fn update(
    database: &AppDatabase,
    id: &str,
    input: RemoteInput,
) -> Result<Option<Remote>, sqlx::Error> {
    let result = sqlx::query("UPDATE remotes SET address = ?1, name = ?2, port = ?3 WHERE id = ?4")
        .bind(&input.address)
        .bind(&input.name)
        .bind(input.port)
        .bind(id)
        .execute(database.pool())
        .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    get(database, id).await
}

async fn reorder(database: &AppDatabase, ids: &[String]) -> Result<bool, String> {
    let existing = all(database).await.map_err(db::command_error)?;
    if ids.len() != existing.len() {
        return Err("Server order must contain every configured server exactly once".to_owned());
    }

    let mut expected = existing
        .into_iter()
        .map(|remote| remote.id)
        .collect::<Vec<_>>();
    let mut provided = ids.to_vec();
    expected.sort_unstable();
    provided.sort_unstable();
    provided.dedup();
    if provided != expected {
        return Err("Server order contains an unknown or duplicate server".to_owned());
    }

    let mut transaction = database.pool().begin().await.map_err(db::command_error)?;
    let mut changed = false;
    for (priority, id) in ids.iter().enumerate() {
        let result =
            sqlx::query("UPDATE remotes SET priority = ?1 WHERE id = ?2 AND priority != ?1")
                .bind(priority as i32)
                .bind(id)
                .execute(&mut *transaction)
                .await
                .map_err(db::command_error)?;
        changed |= result.rows_affected() > 0;
    }
    transaction.commit().await.map_err(db::command_error)?;
    Ok(changed)
}

async fn delete(database: &AppDatabase, id: &str) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM remotes WHERE id = ?1")
        .bind(id)
        .execute(database.pool())
        .await?;

    Ok(result.rows_affected() > 0)
}

async fn emit_changed(handle: &AppHandle, database: &AppDatabase) -> Result<(), String> {
    RemotesChanged {
        remotes: all(database).await.map_err(db::command_error)?,
    }
    .emit(handle)
    .map_err(db::command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn create_remote(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    remote: RemoteInput,
) -> Result<Remote, String> {
    let remote = insert(&database, remote).await.map_err(db::command_error)?;
    emit_changed(&handle, &database).await?;
    Ok(remote)
}

#[tauri::command]
#[specta::specta]
pub async fn list_remotes(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
) -> Result<Vec<Remote>, String> {
    let remotes = all(&database).await.map_err(db::command_error)?;

    RemotesChanged {
        remotes: remotes.clone(),
    }
    .emit(&handle)
    .map_err(db::command_error)?;

    Ok(remotes)
}

#[tauri::command]
#[specta::specta]
pub async fn get_remote(
    database: State<'_, AppDatabase>,
    id: String,
) -> Result<Option<Remote>, String> {
    get(&database, &id).await.map_err(db::command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn update_remote(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    id: String,
    remote: RemoteInput,
) -> Result<Option<Remote>, String> {
    let remote = update(&database, &id, remote)
        .await
        .map_err(db::command_error)?;

    if remote.is_some() {
        emit_changed(&handle, &database).await?;
    }

    Ok(remote)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_remote(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    id: String,
) -> Result<bool, String> {
    let changed = delete(&database, &id).await.map_err(db::command_error)?;

    if changed {
        emit_changed(&handle, &database).await?;
    }

    Ok(changed)
}

#[tauri::command]
#[specta::specta]
pub async fn reorder_remotes(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    ids: Vec<String>,
) -> Result<Vec<Remote>, String> {
    if reorder(&database, &ids).await? {
        emit_changed(&handle, &database).await?;
    }
    all(&database).await.map_err(db::command_error)
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
    async fn insert_generates_a_uuid_v4_and_preserves_optional_fields() {
        let database = database().await;
        let remote = insert(
            &database,
            RemoteInput {
                address: "play.example.com".to_owned(),
                name: None,
                port: None,
            },
        )
        .await
        .expect("insert remote");

        let id = Uuid::parse_str(&remote.id).expect("parse generated UUID");
        assert_eq!(id.get_version_num(), 4);
        assert_eq!(remote.address, "play.example.com");
        assert_eq!(remote.name, None);
        assert_eq!(remote.port, None);
        assert_eq!(remote.priority, 0);
        assert_eq!(get(&database, &remote.id).await.unwrap(), Some(remote));
    }

    #[tokio::test]
    async fn update_and_delete_cover_the_remote_lifecycle() {
        let database = database().await;
        let created = insert(
            &database,
            RemoteInput {
                address: "old.example.com".to_owned(),
                name: Some("Old name".to_owned()),
                port: Some(25565),
            },
        )
        .await
        .expect("insert remote");

        let updated = update(
            &database,
            &created.id,
            RemoteInput {
                address: "new.example.com".to_owned(),
                name: Some("New name".to_owned()),
                port: None,
            },
        )
        .await
        .expect("update remote")
        .expect("remote exists");

        assert_eq!(updated.id, created.id);
        assert_eq!(updated.address, "new.example.com");
        assert_eq!(updated.name.as_deref(), Some("New name"));
        assert_eq!(updated.port, None);
        assert_eq!(updated.priority, created.priority);
        assert_eq!(all(&database).await.unwrap(), vec![updated.clone()]);
        assert!(delete(&database, &updated.id).await.unwrap());
        assert!(!delete(&database, &updated.id).await.unwrap());
        assert_eq!(get(&database, &updated.id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn update_returns_none_for_an_unknown_remote() {
        let database = database().await;

        let updated = update(
            &database,
            "missing",
            RemoteInput {
                address: "example.com".to_owned(),
                name: None,
                port: None,
            },
        )
        .await
        .expect("update query succeeds");

        assert_eq!(updated, None);
    }

    #[tokio::test]
    async fn reorder_validates_and_persists_the_complete_priority_order() {
        let database = database().await;
        let first = insert(
            &database,
            RemoteInput {
                address: "first.example.com".to_owned(),
                name: None,
                port: None,
            },
        )
        .await
        .unwrap();
        let second = insert(
            &database,
            RemoteInput {
                address: "second.example.com".to_owned(),
                name: None,
                port: None,
            },
        )
        .await
        .unwrap();

        assert!(
            reorder(&database, &[second.id.clone(), first.id.clone()])
                .await
                .unwrap()
        );
        let ordered = all(&database).await.unwrap();
        assert_eq!(
            ordered.iter().map(|remote| &remote.id).collect::<Vec<_>>(),
            [&second.id, &first.id]
        );
        assert_eq!(
            ordered
                .iter()
                .map(|remote| remote.priority)
                .collect::<Vec<_>>(),
            [0, 1]
        );
        assert!(
            !reorder(&database, &[second.id.clone(), first.id.clone()])
                .await
                .unwrap()
        );
        assert!(
            reorder(&database, std::slice::from_ref(&first.id))
                .await
                .is_err()
        );
    }
}
