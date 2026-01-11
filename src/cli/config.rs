use crate::cli::args::{ConfigAction, SteeringAction};
use crate::db;
use crate::error::Result;
use crate::output::OutputFormat;
use crate::services::Workspace;

/// Handle config subcommands
pub async fn config(action: ConfigAction, _format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    match action {
        ConfigAction::Get { key } => {
            let value = db::config::get(&pool, &key).await?;
            match value {
                Some(v) => println!("{}", v),
                None => println!("(not set)"),
            }
        }

        ConfigAction::Set { key, value } => {
            db::config::set(&pool, &key, &value).await?;
            println!("Set {} = {}", key, value);
        }

        ConfigAction::List => {
            let items = db::config::list(&pool).await?;
            if items.is_empty() {
                println!("No config values set");
            } else {
                for (key, value) in items {
                    println!("{} = {}", key, value);
                }
            }
        }

        ConfigAction::Delete { key } => {
            let deleted = db::config::delete(&pool, &key).await?;
            if deleted {
                println!("Deleted {}", key);
            } else {
                println!("Key not found: {}", key);
            }
        }
    }

    Ok(())
}

/// Handle steering subcommands
pub async fn steering(action: SteeringAction, _format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    match action {
        SteeringAction::List => {
            let rows = sqlx::query_as::<_, (i64, String, String, String)>(
                "SELECT id, path, mode, created_at FROM steering ORDER BY path",
            )
            .fetch_all(&pool)
            .await?;

            if rows.is_empty() {
                println!("No steering files configured");
            } else {
                println!("Steering files:");
                for (_, path, mode, _) in rows {
                    println!("  {} [{}]", path, mode);
                }
            }
        }

        SteeringAction::Add { path, mode } => {
            let now = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                "INSERT INTO steering (path, mode, created_at) VALUES (?, ?, ?)
                 ON CONFLICT(path) DO UPDATE SET mode = ?",
            )
            .bind(&path)
            .bind(&mode)
            .bind(&now)
            .bind(&mode)
            .execute(&pool)
            .await?;

            println!("Added steering file: {} [{}]", path, mode);
        }

        SteeringAction::Rm { path } => {
            let result = sqlx::query("DELETE FROM steering WHERE path = ?")
                .bind(&path)
                .execute(&pool)
                .await?;

            if result.rows_affected() > 0 {
                println!("Removed steering file: {}", path);
            } else {
                println!("Steering file not found: {}", path);
            }
        }
    }

    Ok(())
}
