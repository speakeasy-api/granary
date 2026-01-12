use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

use crate::error::Result;

/// Create a connection pool for the SQLite database
pub async fn create_pool(db_path: &Path) -> Result<SqlitePool> {
    let url = format!("sqlite:{}?mode=rwc", db_path.display());

    let options = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(30));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    Ok(pool)
}

/// Run database migrations using sqlx's migration system
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    // Enable foreign keys (needs to be set per-connection in SQLite)
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(pool)
        .await?;

    // Run embedded migrations from the migrations/ directory
    sqlx::migrate!("./migrations").run(pool).await?;

    Ok(())
}
