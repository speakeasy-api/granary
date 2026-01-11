use crate::cli::args::SessionAction;
use crate::error::{GranaryError, Result};
use crate::models::*;
use crate::output::{Formatter, OutputFormat};
use crate::services::{self, Workspace};

/// List sessions
pub async fn list_sessions(include_closed: bool, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let sessions = services::list_sessions(&pool, include_closed).await?;

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_sessions(&sessions));

    Ok(())
}

/// Handle session subcommands
pub async fn session(action: SessionAction, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;
    let formatter = Formatter::new(format);

    match action {
        SessionAction::Start { name, owner, mode } => {
            let mode = mode.parse().unwrap_or_default();

            let session = services::create_session(
                &pool,
                CreateSession {
                    name: Some(name),
                    owner,
                    mode,
                },
            )
            .await?;

            // Set as current session
            workspace.set_current_session(&session.id)?;

            println!("Started session: {}", session.id);
            println!("{}", formatter.format_session(&session));
        }

        SessionAction::Current => {
            match services::get_current_session(&pool, &workspace).await? {
                Some(session) => {
                    println!("{}", formatter.format_session(&session));

                    // Also show scope
                    let scope = services::get_scope(&pool, &session.id).await?;
                    if !scope.is_empty() {
                        println!("\nScope:");
                        for item in scope {
                            println!("  {} {}", item.item_type, item.item_id);
                        }
                    }
                }
                None => {
                    println!("No active session");
                }
            }
        }

        SessionAction::Use { session_id } => {
            // Verify session exists
            let session = services::get_session(&pool, &session_id).await?;

            if session.is_closed() {
                return Err(GranaryError::Conflict(format!(
                    "Session {} is closed",
                    session_id
                )));
            }

            workspace.set_current_session(&session_id)?;
            println!("Now using session: {}", session_id);
            println!("{}", formatter.format_session(&session));
        }

        SessionAction::Close {
            session_id,
            summary,
        } => {
            let session_id = session_id
                .or_else(|| workspace.current_session_id())
                .ok_or(GranaryError::NoActiveSession)?;

            let session =
                services::close_session(&pool, &session_id, summary.as_deref(), &workspace).await?;

            println!("Closed session: {}", session.id);
        }

        SessionAction::Add { item_type, item_id } => {
            let session_id = workspace
                .current_session_id()
                .ok_or(GranaryError::NoActiveSession)?;

            let item_type_enum: ScopeItemType = item_type.parse().map_err(|_| {
                GranaryError::InvalidArgument(format!("Invalid item type: {}", item_type))
            })?;

            // Verify the item exists
            match item_type_enum {
                ScopeItemType::Project => {
                    services::get_project(&pool, &item_id).await?;
                }
                ScopeItemType::Task => {
                    services::get_task(&pool, &item_id).await?;
                }
                _ => {}
            }

            services::add_to_scope(&pool, &session_id, item_type_enum, &item_id).await?;
            println!("Added {} {} to session scope", item_type, item_id);
        }

        SessionAction::Rm { item_type, item_id } => {
            let session_id = workspace
                .current_session_id()
                .ok_or(GranaryError::NoActiveSession)?;

            let item_type_enum: ScopeItemType = item_type.parse().map_err(|_| {
                GranaryError::InvalidArgument(format!("Invalid item type: {}", item_type))
            })?;

            let removed =
                services::remove_from_scope(&pool, &session_id, item_type_enum, &item_id).await?;

            if removed {
                println!("Removed {} {} from session scope", item_type, item_id);
            } else {
                println!("Item not found in session scope");
            }
        }

        SessionAction::Env => {
            let session_id = workspace
                .current_session_id()
                .ok_or(GranaryError::NoActiveSession)?;

            let env =
                services::get_session_env(&session_id, workspace.root.to_str().unwrap_or("."));
            print!("{}", env);
        }
    }

    Ok(())
}
