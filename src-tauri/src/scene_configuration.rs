use crate::db::{self, AppDatabase};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;

const SCENE_CONFIGURATION_ID: i64 = 1;
const MIN_PUPPET_SCALE: f64 = 0.5;
const MAX_PUPPET_SCALE: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SceneConfiguration {
    pub puppet_scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SceneConfigurationChanged {
    pub configuration: SceneConfiguration,
}

async fn get(database: &AppDatabase) -> Result<SceneConfiguration, sqlx::Error> {
    sqlx::query_as::<_, SceneConfiguration>(
        "SELECT puppet_scale FROM scene_configuration WHERE id = ?1",
    )
    .bind(SCENE_CONFIGURATION_ID)
    .fetch_one(database.pool())
    .await
}

async fn update(
    database: &AppDatabase,
    configuration: SceneConfiguration,
) -> Result<SceneConfiguration, String> {
    if !configuration.puppet_scale.is_finite()
        || !(MIN_PUPPET_SCALE..=MAX_PUPPET_SCALE).contains(&configuration.puppet_scale)
    {
        return Err(format!(
            "Puppet scale must be between {MIN_PUPPET_SCALE} and {MAX_PUPPET_SCALE}."
        ));
    }

    sqlx::query("UPDATE scene_configuration SET puppet_scale = ?1 WHERE id = ?2")
        .bind(configuration.puppet_scale)
        .bind(SCENE_CONFIGURATION_ID)
        .execute(database.pool())
        .await
        .map_err(db::command_error)?;

    get(database).await.map_err(db::command_error)
}

fn emit_changed(handle: &AppHandle, configuration: SceneConfiguration) -> Result<(), String> {
    SceneConfigurationChanged { configuration }
        .emit(handle)
        .map_err(db::command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn get_scene_configuration(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
) -> Result<SceneConfiguration, String> {
    let configuration = get(&database).await.map_err(db::command_error)?;
    emit_changed(&handle, configuration.clone())?;
    Ok(configuration)
}

#[tauri::command]
#[specta::specta]
pub async fn update_scene_configuration(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    configuration: SceneConfiguration,
) -> Result<SceneConfiguration, String> {
    let configuration = update(&database, configuration).await?;
    emit_changed(&handle, configuration.clone())?;
    Ok(configuration)
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
    async fn scene_configuration_defaults_to_unit_puppet_scale() {
        let database = database().await;

        assert_eq!(
            get(&database).await.unwrap(),
            SceneConfiguration { puppet_scale: 1.0 }
        );
    }

    #[tokio::test]
    async fn puppet_scale_is_persisted_and_range_checked() {
        let database = database().await;

        let configuration = update(&database, SceneConfiguration { puppet_scale: 1.5 })
            .await
            .expect("update scene configuration");
        assert_eq!(configuration.puppet_scale, 1.5);
        assert_eq!(get(&database).await.unwrap(), configuration);

        let error = update(&database, SceneConfiguration { puppet_scale: 2.1 })
            .await
            .expect_err("reject an out-of-range puppet scale");
        assert_eq!(error, "Puppet scale must be between 0.5 and 2.");
        assert_eq!(get(&database).await.unwrap().puppet_scale, 1.5);
    }
}
