use crate::cli::args::{CliOutputFormat, SessionAction};
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::error::{GranaryError, Result};
use crate::models::*;
use crate::output::{Output, json, prompt, table};
use crate::services::{self, Workspace};
use std::time::Duration;

// =============================================================================
// Output Types
// =============================================================================

/// Output for a list of sessions
pub struct SessionsOutput {
    pub sessions: Vec<Session>,
}

impl Output for SessionsOutput {
    fn to_json(&self) -> String {
        json::format_sessions(&self.sessions)
    }

    fn to_prompt(&self) -> String {
        prompt::format_sessions(&self.sessions)
    }

    fn to_text(&self) -> String {
        table::format_sessions(&self.sessions)
    }
}

/// Output for a single session
pub struct SessionOutput {
    pub session: Session,
}

impl Output for SessionOutput {
    fn to_json(&self) -> String {
        json::format_session(&self.session)
    }

    fn to_prompt(&self) -> String {
        prompt::format_session(&self.session)
    }

    fn to_text(&self) -> String {
        table::format_session(&self.session)
    }
}

/// List sessions
pub async fn list_sessions(
    include_closed: bool,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    if watch {
        let interval = Duration::from_secs(interval);
        watch_loop(interval, || async {
            let output = fetch_and_format_sessions(include_closed, cli_format).await?;
            Ok(format!("{}\n{}", watch_status_line(interval), output))
        })
        .await
    } else {
        let output = fetch_and_format_sessions(include_closed, cli_format).await?;
        println!("{}", output);
        Ok(())
    }
}

/// Fetch sessions and format them as a string
async fn fetch_and_format_sessions(
    include_closed: bool,
    cli_format: Option<CliOutputFormat>,
) -> Result<String> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let sessions = services::list_sessions(&pool, include_closed).await?;

    let output = SessionsOutput { sessions };
    Ok(output.format(cli_format))
}

/// Handle session subcommands
pub async fn session(action: SessionAction, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    match action {
        SessionAction::Start {
            name_positional,
            name_flag,
            owner,
            mode,
        } => {
            let name = name_positional.or(name_flag).ok_or_else(|| {
                GranaryError::InvalidArgument(
                    "Session name is required. Usage: granary session start <name>".to_string(),
                )
            })?;
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
            let output = SessionOutput { session };
            println!("{}", output.format(cli_format));
            eprintln!(
                "\nIMPORTANT: Remember to close this session when done with: granary session close --summary \"your summary here...\""
            );
        }

        SessionAction::Current => {
            match services::get_current_session(&pool, &workspace).await? {
                Some(session) => {
                    let output = SessionOutput {
                        session: session.clone(),
                    };
                    println!("{}", output.format(cli_format));

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
            let output = SessionOutput { session };
            println!("{}", output.format(cli_format));
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

        SessionAction::Add { args } => {
            let session_id = workspace
                .current_session_id()
                .ok_or(GranaryError::NoActiveSession)?;

            // Parse args: either [id] (auto-detect) or [type, id] (explicit)
            let (item_type_str, item_id) = match args.len() {
                1 => {
                    // Auto-detect type from ID
                    let id = &args[0];
                    let kind = crate::cli::show::detect_entity_kind(id);
                    let type_str = match kind {
                        crate::cli::show::EntityKind::Project => "project",
                        crate::cli::show::EntityKind::Task => "task",
                        _ => {
                            return Err(GranaryError::InvalidArgument(format!(
                                "Cannot add {} to session scope (only projects and tasks are supported)",
                                match kind {
                                    crate::cli::show::EntityKind::Session => "sessions",
                                    crate::cli::show::EntityKind::Checkpoint => "checkpoints",
                                    crate::cli::show::EntityKind::Comment => "comments",
                                    crate::cli::show::EntityKind::Artifact => "artifacts",
                                    _ => "unknown entities",
                                }
                            )));
                        }
                    };
                    (type_str.to_string(), id.clone())
                }
                2 => {
                    // Explicit type provided
                    (args[0].clone(), args[1].clone())
                }
                _ => {
                    return Err(GranaryError::InvalidArgument(
                        "Expected: session add <id> or session add <type> <id>".to_string(),
                    ));
                }
            };

            let item_type_enum: ScopeItemType = item_type_str.parse().map_err(|_| {
                GranaryError::InvalidArgument(format!("Invalid item type: {}", item_type_str))
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
            println!("Added {} {} to session scope", item_type_str, item_id);
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
