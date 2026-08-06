use crate::db::{self, AppDatabase};
use crate::keypair::AppKeypair;
use crate::user::User;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;

const PROFILE_ID: i64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ProfileChanged {
    pub profile: User,
}

pub async fn get(database: &AppDatabase, public_key: &str) -> Result<User, sqlx::Error> {
    let display_name =
        sqlx::query_scalar::<_, String>("SELECT display_name FROM profile WHERE id = ?1")
            .bind(PROFILE_ID)
            .fetch_one(database.pool())
            .await?;

    Ok(User {
        id: public_key.to_owned(),
        display_name: if display_name.len() <= 0 {
            "Anonymous".to_string()
        } else {
            display_name
        },
    })
}

async fn update(
    database: &AppDatabase,
    public_key: &str,
    display_name: String,
) -> Result<User, sqlx::Error> {
    sqlx::query("UPDATE profile SET display_name = ?1 WHERE id = ?2")
        .bind(display_name)
        .bind(PROFILE_ID)
        .execute(database.pool())
        .await?;

    get(database, public_key).await
}

fn emit_changed(handle: &AppHandle, profile: User) -> Result<(), String> {
    ProfileChanged { profile }
        .emit(handle)
        .map_err(db::command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn get_profile(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    keypair: State<'_, AppKeypair>,
) -> Result<User, String> {
    let profile = get(&database, keypair.public_key())
        .await
        .map_err(db::command_error)?;
    emit_changed(&handle, profile.clone())?;
    Ok(profile)
}

#[tauri::command]
#[specta::specta]
pub async fn update_profile(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    keypair: State<'_, AppKeypair>,
    display_name: String,
) -> Result<User, String> {
    let profile = update(&database, keypair.public_key(), display_name)
        .await
        .map_err(db::command_error)?;
    emit_changed(&handle, profile.clone())?;
    Ok(profile)
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
    async fn profile_uses_the_supplied_keypair_identity() {
        let database = database().await;

        let profile = update(&database, "public-key", "Wind".to_owned())
            .await
            .expect("update profile");

        assert_eq!(
            profile,
            User {
                id: "public-key".to_owned(),
                display_name: "Wind".to_owned(),
            }
        );
        assert_eq!(
            get(&database, "rotated-public-key").await.unwrap(),
            User {
                id: "rotated-public-key".to_owned(),
                display_name: "Wind".to_owned(),
            }
        );
    }

    #[tokio::test]
    async fn profile_table_does_not_duplicate_the_public_key() {
        let database = database().await;
        let columns = sqlx::query_scalar::<_, String>(
            "SELECT name FROM pragma_table_info('profile') ORDER BY cid",
        )
        .fetch_all(database.pool())
        .await
        .expect("read profile columns");

        assert_eq!(columns, vec!["id", "display_name"]);
    }
}
