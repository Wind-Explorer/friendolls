use crate::db::{self, AppDatabase};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::RwLock;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

const SCENE_CONFIGURATION_ID: i64 = 1;
const MIN_PUPPET_SCALE: f64 = 0.5;
const MAX_PUPPET_SCALE: f64 = 2.0;
const MIN_PUPPET_OPACITY: f64 = 0.1;
const MAX_PUPPET_OPACITY: f64 = 1.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize, Type, sqlx::Type)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum PuppetMovementMode {
    #[default]
    Free,
    Bottom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct SceneConfiguration {
    pub puppet_scale: f64,
    pub puppet_opacity: f64,
    pub puppet_movement_mode: PuppetMovementMode,
    pub hide_local_puppet_when_alone: bool,
}

pub struct SceneConfigurationState(RwLock<SceneConfiguration>);

impl SceneConfigurationState {
    fn new(configuration: SceneConfiguration) -> Self {
        Self(RwLock::new(configuration))
    }

    fn replace(&self, configuration: SceneConfiguration) -> Result<(), String> {
        *self.0.write().map_err(|error| error.to_string())? = configuration;
        Ok(())
    }

    pub(crate) fn puppet_movement_mode(&self) -> Result<PuppetMovementMode, String> {
        self.0
            .read()
            .map(|configuration| configuration.puppet_movement_mode)
            .map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SceneConfigurationChanged {
    pub configuration: SceneConfiguration,
}

async fn get(database: &AppDatabase) -> Result<SceneConfiguration, sqlx::Error> {
    sqlx::query_as::<_, SceneConfiguration>(
        "SELECT puppet_scale, puppet_opacity, puppet_movement_mode, hide_local_puppet_when_alone FROM scene_configuration WHERE id = ?1",
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
    if !configuration.puppet_opacity.is_finite()
        || !(MIN_PUPPET_OPACITY..=MAX_PUPPET_OPACITY).contains(&configuration.puppet_opacity)
    {
        return Err(format!(
            "Puppet opacity must be between {MIN_PUPPET_OPACITY} and {MAX_PUPPET_OPACITY}."
        ));
    }

    sqlx::query(
        "UPDATE scene_configuration SET puppet_scale = ?1, puppet_opacity = ?2, puppet_movement_mode = ?3, hide_local_puppet_when_alone = ?4 WHERE id = ?5",
    )
    .bind(configuration.puppet_scale)
    .bind(configuration.puppet_opacity)
    .bind(configuration.puppet_movement_mode)
    .bind(configuration.hide_local_puppet_when_alone)
    .bind(SCENE_CONFIGURATION_ID)
    .execute(database.pool())
    .await
    .map_err(db::command_error)?;

    get(database).await.map_err(db::command_error)
}

pub async fn init(handle: &AppHandle) -> Result<(), sqlx::Error> {
    let configuration = get(&handle.state::<AppDatabase>()).await?;
    handle.manage(SceneConfigurationState::new(configuration));
    Ok(())
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
    state: State<'_, SceneConfigurationState>,
) -> Result<SceneConfiguration, String> {
    let configuration = get(&database).await.map_err(db::command_error)?;
    state.replace(configuration.clone())?;
    emit_changed(&handle, configuration.clone())?;
    Ok(configuration)
}

#[tauri::command]
#[specta::specta]
pub async fn update_scene_configuration(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
    state: State<'_, SceneConfigurationState>,
    configuration: SceneConfiguration,
) -> Result<SceneConfiguration, String> {
    let configuration = update(&database, configuration).await?;
    state.replace(configuration.clone())?;
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
    async fn scene_configuration_defaults_preserve_the_current_appearance() {
        let database = database().await;

        assert_eq!(
            get(&database).await.unwrap(),
            SceneConfiguration {
                puppet_scale: 1.0,
                puppet_opacity: 1.0,
                puppet_movement_mode: PuppetMovementMode::Free,
                hide_local_puppet_when_alone: false,
            }
        );
    }

    #[tokio::test]
    async fn scene_configuration_is_persisted_and_range_checked() {
        let database = database().await;

        let configuration = update(
            &database,
            SceneConfiguration {
                puppet_scale: 1.5,
                puppet_opacity: 0.4,
                puppet_movement_mode: PuppetMovementMode::Bottom,
                hide_local_puppet_when_alone: true,
            },
        )
        .await
        .expect("update scene configuration");
        assert_eq!(configuration.puppet_scale, 1.5);
        assert_eq!(configuration.puppet_opacity, 0.4);
        assert_eq!(
            configuration.puppet_movement_mode,
            PuppetMovementMode::Bottom
        );
        assert!(configuration.hide_local_puppet_when_alone);
        assert_eq!(get(&database).await.unwrap(), configuration);

        let error = update(
            &database,
            SceneConfiguration {
                puppet_scale: 2.1,
                puppet_opacity: 0.4,
                puppet_movement_mode: PuppetMovementMode::Bottom,
                hide_local_puppet_when_alone: true,
            },
        )
        .await
        .expect_err("reject an out-of-range puppet scale");
        assert_eq!(error, "Puppet scale must be between 0.5 and 2.");
        assert_eq!(get(&database).await.unwrap().puppet_scale, 1.5);

        let error = update(
            &database,
            SceneConfiguration {
                puppet_scale: 1.5,
                puppet_opacity: 0.05,
                puppet_movement_mode: PuppetMovementMode::Bottom,
                hide_local_puppet_when_alone: true,
            },
        )
        .await
        .expect_err("reject an out-of-range puppet opacity");
        assert_eq!(error, "Puppet opacity must be between 0.1 and 1.");
        assert_eq!(get(&database).await.unwrap().puppet_opacity, 0.4);
    }
}
