use crate::cli::args::{ConfigAction, RunnersAction, SteeringAction};
use crate::db;
use crate::error::Result;
use crate::models::global_config::RunnerConfig;
use crate::output::OutputFormat;
use crate::services::{Workspace, global_config_service};
use std::collections::HashMap;

/// Handle config subcommands
pub async fn config(action: ConfigAction, _format: OutputFormat) -> Result<()> {
    match action {
        // Workspace-level config commands need workspace
        ConfigAction::Get { key } => {
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
            let value = db::config::get(&pool, &key).await?;
            match value {
                Some(v) => println!("{}", v),
                None => println!("(not set)"),
            }
        }

        ConfigAction::Set { key, value } => {
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
            db::config::set(&pool, &key, &value).await?;
            println!("Set {} = {}", key, value);
        }

        ConfigAction::List => {
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
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
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
            let deleted = db::config::delete(&pool, &key).await?;
            if deleted {
                println!("Deleted {}", key);
            } else {
                println!("Key not found: {}", key);
            }
        }

        // Global config commands (don't need workspace)
        ConfigAction::Edit => {
            let path = global_config_service::config_path()?;
            println!("Opening {} in editor...", path.display());
            global_config_service::edit_config()?;
            println!("Config file saved.");
        }

        ConfigAction::Runners { action } => {
            handle_runners_action(action).await?;
        }
    }

    Ok(())
}

/// Handle runners subcommands
async fn handle_runners_action(action: Option<RunnersAction>) -> Result<()> {
    match action {
        None => {
            // List all runners
            let config = global_config_service::load()?;
            if config.runners.is_empty() {
                println!("No runners configured.");
                println!("\nAdd a runner with:");
                println!("  granary config runners add <name> --command <cmd>");
            } else {
                println!("Configured runners:\n");
                for (name, runner) in &config.runners {
                    println!("  {} -> {} {}", name, runner.command, runner.args.join(" "));
                    if let Some(c) = runner.concurrency {
                        println!("    concurrency: {}", c);
                    }
                    if let Some(ref on) = runner.on {
                        println!("    on: {}", on);
                    }
                    if !runner.env.is_empty() {
                        println!(
                            "    env: {}",
                            runner
                                .env
                                .iter()
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
            }
        }

        Some(RunnersAction::Add {
            name,
            command,
            args,
            concurrency,
            on,
            env_vars,
        }) => {
            let env = parse_env_vars(&env_vars);
            let runner = RunnerConfig {
                command,
                args,
                concurrency,
                on,
                env,
            };
            global_config_service::set_runner(&name, runner)?;
            println!("Added runner: {}", name);
        }

        Some(RunnersAction::Update {
            name,
            command,
            args,
            concurrency,
            on,
            env_vars,
        }) => {
            let existing = global_config_service::get_runner(&name)?;
            match existing {
                Some(mut runner) => {
                    if let Some(cmd) = command {
                        runner.command = cmd;
                    }
                    if let Some(a) = args {
                        runner.args = a;
                    }
                    if concurrency.is_some() {
                        runner.concurrency = concurrency;
                    }
                    if on.is_some() {
                        runner.on = on;
                    }
                    if let Some(env_vec) = env_vars {
                        runner.env = parse_env_vars(&env_vec);
                    }
                    global_config_service::set_runner(&name, runner)?;
                    println!("Updated runner: {}", name);
                }
                None => {
                    println!("Runner not found: {}", name);
                    std::process::exit(3);
                }
            }
        }

        Some(RunnersAction::Rm { name }) => {
            let removed = global_config_service::remove_runner(&name)?;
            if removed {
                println!("Removed runner: {}", name);
            } else {
                println!("Runner not found: {}", name);
                std::process::exit(3);
            }
        }

        Some(RunnersAction::Show { name }) => match global_config_service::get_runner(&name)? {
            Some(runner) => {
                println!("Runner: {}\n", name);
                println!("  command: {}", runner.command);
                if !runner.args.is_empty() {
                    println!("  args: {:?}", runner.args);
                }
                if let Some(c) = runner.concurrency {
                    println!("  concurrency: {}", c);
                }
                if let Some(ref on) = runner.on {
                    println!("  on: {}", on);
                }
                if !runner.env.is_empty() {
                    println!("  env:");
                    for (k, v) in &runner.env {
                        println!("    {}={}", k, v);
                    }
                }
            }
            None => {
                println!("Runner not found: {}", name);
                std::process::exit(3);
            }
        },
    }

    Ok(())
}

/// Parse environment variables from "KEY=VALUE" format
fn parse_env_vars(env_vars: &[String]) -> HashMap<String, String> {
    env_vars
        .iter()
        .filter_map(|s| {
            let mut parts = s.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(key), Some(value)) => Some((key.to_string(), value.to_string())),
                _ => None,
            }
        })
        .collect()
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
