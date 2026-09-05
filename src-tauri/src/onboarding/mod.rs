use tauri::{AppHandle, State};

use crate::db::{self, AppDatabase};

#[tauri::command]
#[specta::specta]
pub async fn complete_onboarding(
    handle: AppHandle,
    database: State<'_, AppDatabase>,
) -> Result<(), String> {
    crate::settings::set_onboarding_done(&database, true)
        .await
        .map_err(db::command_error)?;
    crate::application::start(&handle);
    Ok(())
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;
    use crate::settings::AppSettings;

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
            crate::settings::get(&database).await.unwrap(),
            AppSettings {
                onboarding_done: false,
                locale_preference: "system".to_owned(),
            }
        );

        crate::settings::set_onboarding_done(&database, true)
            .await
            .unwrap();
        assert_eq!(
            crate::settings::get(&database).await.unwrap(),
            AppSettings {
                onboarding_done: true,
                locale_preference: "system".to_owned(),
            }
        );
    }
}
