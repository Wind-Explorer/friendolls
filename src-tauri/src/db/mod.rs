use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::error::Error;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

const DATABASE_FILE: &str = "friendolls.sqlite";
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub struct AppDatabase {
    pool: Pool<Sqlite>,
}

impl AppDatabase {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}

pub async fn init(app: &AppHandle) -> Result<(), Box<dyn Error>> {
    let app_data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_data_dir)?;

    let db_path = app_data_dir.join(DATABASE_FILE);
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    if database_requires_newer_app(&pool).await? {
        show_outdated_app_dialog(app);
        return std::future::pending().await;
    }

    MIGRATOR.run(&pool).await?;

    app.manage(AppDatabase::new(pool));

    Ok(())
}

async fn database_requires_newer_app(pool: &Pool<Sqlite>) -> Result<bool, sqlx::Error> {
    let has_migration_history = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;

    if !has_migration_history {
        return Ok(false);
    }

    let latest_applied =
        sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await?;
    let latest_supported = MIGRATOR.iter().map(|migration| migration.version).max();

    Ok(latest_applied
        .zip(latest_supported)
        .is_some_and(|(applied, supported)| applied > supported))
}

fn show_outdated_app_dialog(app: &AppHandle) {
    app.dialog()
        .message(crate::settings::system_text(
            crate::settings::NativeText::OutdatedMessage,
        ))
        .title(crate::settings::system_text(
            crate::settings::NativeText::OutdatedTitle,
        ))
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCustom(
            crate::settings::system_text(crate::settings::NativeText::CloseFriendolls).into(),
        ))
        .show(move |_| std::process::exit(0));
}

#[inline]
pub fn command_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn database_without_migration_history_does_not_require_newer_app() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("connect to in-memory database");

        assert!(!database_requires_newer_app(&pool).await.unwrap());
    }

    #[tokio::test]
    async fn database_with_newer_migration_requires_newer_app() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .expect("connect to in-memory database");
        sqlx::query("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY)")
            .execute(&pool)
            .await
            .expect("create migration history");
        let newer_version = MIGRATOR
            .iter()
            .map(|migration| migration.version)
            .max()
            .expect("at least one embedded migration")
            + 1;
        sqlx::query("INSERT INTO _sqlx_migrations (version) VALUES (?)")
            .bind(newer_version)
            .execute(&pool)
            .await
            .expect("insert newer migration");

        assert!(database_requires_newer_app(&pool).await.unwrap());
    }
}
