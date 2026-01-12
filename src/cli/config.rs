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
            let files = db::steering::list(&pool).await?;

            if files.is_empty() {
                println!("No steering files configured");
            } else {
                println!("Steering files:");
                for file in files {
                    println!("  {} [{}]", file.path, file.scope_display());
                }
            }
        }

        SteeringAction::Add {
            path,
            mode,
            project,
            task,
            for_session,
        } => {
            // Determine scope
            let (scope_type, scope_id): (Option<&str>, Option<String>) =
                if let Some(ref proj_id) = project {
                    (Some("project"), Some(proj_id.clone()))
                } else if let Some(ref task_id) = task {
                    (Some("task"), Some(task_id.clone()))
                } else if for_session {
                    // Get current session ID
                    let session_id = workspace
                        .current_session_id()
                        .ok_or_else(|| crate::error::GranaryError::NoActiveSession)?;
                    (Some("session"), Some(session_id))
                } else {
                    (None, None)
                };

            db::steering::add(&pool, &path, &mode, scope_type, scope_id.as_deref()).await?;

            let scope_display = match (scope_type, &scope_id) {
                (None, _) => "global".to_string(),
                (Some(t), Some(id)) => format!("{}: {}", t, id),
                _ => "unknown".to_string(),
            };
            println!("Added steering file: {} [{}]", path, scope_display);
        }

        SteeringAction::Rm {
            path,
            project,
            task,
            for_session,
        } => {
            // Determine scope
            let (scope_type, scope_id): (Option<&str>, Option<String>) =
                if let Some(ref proj_id) = project {
                    (Some("project"), Some(proj_id.clone()))
                } else if let Some(ref task_id) = task {
                    (Some("task"), Some(task_id.clone()))
                } else if for_session {
                    // Get current session ID
                    let session_id = workspace
                        .current_session_id()
                        .ok_or_else(|| crate::error::GranaryError::NoActiveSession)?;
                    (Some("session"), Some(session_id))
                } else {
                    (None, None)
                };

            let removed =
                db::steering::remove(&pool, &path, scope_type, scope_id.as_deref()).await?;

            if removed {
                let scope_display = match (scope_type, &scope_id) {
                    (None, _) => "global".to_string(),
                    (Some(t), Some(id)) => format!("{}: {}", t, id),
                    _ => "unknown".to_string(),
                };
                println!("Removed steering file: {} [{}]", path, scope_display);
            } else {
                println!("Steering file not found: {}", path);
            }
        }
    }

    Ok(())
}
