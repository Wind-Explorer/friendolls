use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::error::Error;
use tauri::{AppHandle, Manager};

const DATABASE_FILE: &str = "wyd.sqlite";

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

    sqlx::migrate!("./migrations").run(&pool).await?;

    app.manage(AppDatabase::new(pool));

    Ok(())
}

#[inline]
pub fn command_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
