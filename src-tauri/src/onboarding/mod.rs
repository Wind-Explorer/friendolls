use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::db::{self, AppDatabase};

const SETTINGS_ID: i64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AppSettings {
    pub onboarding_done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingStatus {
    pub onboarding_done: bool,
    pub macos_accessibility_permission_granted: bool,
    pub requires_accessibility_permission: bool,
}

impl OnboardingStatus {
    fn new(handle: &AppHandle, settings: AppSettings) -> Self {
        Self {
            onboarding_done: settings.onboarding_done,
            macos_accessibility_permission_granted: crate::macos::accessibility_permission_granted(
                handle,
            ),
            requires_accessibility_permission: cfg!(target_os = "macos"),
        }
    }
}

pub async fn get(database: &AppDatabase) -> Result<AppSettings, sqlx::Error> {
    sqlx::query_as::<_, AppSettings>("SELECT onboarding_done FROM app_settings WHERE id = ?1")
        .bind(SETTINGS_ID)
        .fetch_one(database.pool())
        .await
}

async fn set_onboarding_done(
    database: &AppDatabase,
    onboarding_done: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE app_settings SET onboarding_done = ?1 WHERE id = ?2")
        .bind(onboarding_done)
        .bind(SETTINGS_ID)
        .execute(database.pool())
        .await?;
    Ok(())
}

pub(crate) async fn emit_status(
    handle: &AppHandle,
    database: &AppDatabase,
) -> Result<OnboardingStatus, String> {
    let status = OnboardingStatus::new(handle, get(database).await.map_err(db::command_error)?);
    status.clone().emit(handle).map_err(db::command_error)?;
    Ok(status)
}

#[tauri::command]
#[specta::specta]
pub async fn get_onboarding_status(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
) -> Result<OnboardingStatus, String> {
    let settings = get(&database).await.map_err(db::command_error)?;
    Ok(OnboardingStatus::new(&handle, settings))
}

#[tauri::command]
#[specta::specta]
pub async fn complete_onboarding(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
) -> Result<(), String> {
    if cfg!(target_os = "macos") && !crate::macos::accessibility_permission_granted(&handle) {
        return Err("Accessibility access must be granted before setup can finish.".to_owned());
    }

    set_onboarding_done(&database, true)
        .await
        .map_err(db::command_error)?;
    emit_status(&handle, &database).await?;
    crate::application::start(&handle).await;
    Ok(())
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
    async fn onboarding_completion_is_persisted() {
        let database = database().await;
        assert_eq!(
            get(&database).await.unwrap(),
            AppSettings {
                onboarding_done: false,
            }
        );

        set_onboarding_done(&database, true).await.unwrap();
        assert_eq!(
            get(&database).await.unwrap(),
            AppSettings {
                onboarding_done: true,
            }
        );
    }
}
