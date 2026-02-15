use std::collections::HashMap;
use std::path::PathBuf;

use crate::cli::args::{ActionCommand, CliOutputFormat};
use crate::cli::config::{ActionShowOutput, ActionsListOutput, parse_env_vars};
use crate::error::{GranaryError, Result};
use crate::models::{ActionConfig, Event, OnError};
use crate::output::Output;
use crate::services::global_config_service;
use crate::services::runner::{PipelineStepResult, spawn_runner_piped};
use crate::services::template::{
    PipelineContext, StepOutput, substitute_all_with_context, substitute_with_context,
};

const GITHUB_REGISTRY_BASE: &str =
    "https://raw.githubusercontent.com/speakeasy-api/granary/main/actions";

/// Handle the top-level `granary action` / `granary actions` command
pub async fn action(action: Option<ActionCommand>, format: Option<CliOutputFormat>) -> Result<()> {
    match action {
        None => {
            // List all actions (same as `granary config actions`)
            let actions = global_config_service::list_all_actions()?;
            let output = ActionsListOutput { actions };
            println!("{}", output.format(format));
        }

        Some(ActionCommand::Add {
            name,
            command,
            description,
            args,
            concurrency,
            on,
            env_vars,
        }) => {
            let env = parse_env_vars(&env_vars);
            let action_config = ActionConfig {
                description,
                command: Some(command),
                args,
                concurrency,
                on,
                env,
                action: None,
                steps: None,
            };
            global_config_service::set_action(&name, action_config)?;
            println!("Added action: {}", name);
        }

        Some(ActionCommand::Install { name }) => {
            install_action(&name).await?;
        }

        Some(ActionCommand::Remove { name }) => {
            remove_action(&name)?;
        }

        Some(ActionCommand::Update { name }) => {
            update_action(&name).await?;
        }

        Some(ActionCommand::Show { name }) => match global_config_service::get_action(&name)? {
            Some(action_config) => {
                let output = ActionShowOutput {
                    name: name.clone(),
                    action: action_config,
                };
                println!("{}", output.format(format));
            }
            None => {
                println!("Action not found: {}", name);
                std::process::exit(3);
            }
        },

        Some(ActionCommand::Run {
            name,
            vars,
            cwd,
            dry_run,
        }) => {
            run_action(&name, &vars, cwd, dry_run, format).await?;
        }
    }

    Ok(())
}

/// Fetch an action TOML from the GitHub registry
async fn fetch_action_from_registry(name: &str) -> Result<String> {
    let url = format!("{}/{}.toml", GITHUB_REGISTRY_BASE, name);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "granary-cli")
        .send()
        .await
        .map_err(|e| GranaryError::Network(e.to_string()))?;

    if !response.status().is_success() {
        return Err(GranaryError::Network(format!(
            "Action '{}' not found in registry (HTTP {})",
            name,
            response.status()
        )));
    }

    let content = response
        .text()
        .await
        .map_err(|e| GranaryError::Network(e.to_string()))?;

    Ok(content)
}

/// Install an action from the GitHub registry
async fn install_action(name: &str) -> Result<()> {
    let content = fetch_action_from_registry(name).await?;

    // Validate it parses as ActionConfig
    let _action: ActionConfig = toml::from_str(&content).map_err(|e| {
        GranaryError::GlobalConfig(format!("Failed to parse registry action '{}': {}", name, e))
    })?;

    // Ensure actions dir exists
    let actions_dir = global_config_service::actions_dir()?;
    std::fs::create_dir_all(&actions_dir)?;

    // Write to file, creating parent directories for namespaced names (e.g., git/worktree-create)
    let path = actions_dir.join(format!("{}.toml", name));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &content)?;

    println!("Installed action: {}", name);
    Ok(())
}

/// Remove a local action file
fn remove_action(name: &str) -> Result<()> {
    let actions_dir = global_config_service::actions_dir()?;
    let path = actions_dir.join(format!("{}.toml", name));

    if path.exists() {
        std::fs::remove_file(&path)?;
        // Clean up empty parent directories for namespaced actions
        let mut dir = path.parent();
        while let Some(parent) = dir {
            if parent == actions_dir {
                break;
            }
            if std::fs::read_dir(parent)?.next().is_none() {
                std::fs::remove_dir(parent)?;
            } else {
                break;
            }
            dir = parent.parent();
        }
        println!("Removed action: {}", name);
    } else {
        // Also try removing from inline config
        let removed = global_config_service::remove_action(name)?;
        if removed {
            println!("Removed action: {}", name);
        } else {
            println!("Action not found: {}", name);
        }
    }

    Ok(())
}

/// Update an action from the GitHub registry (re-fetch)
async fn update_action(name: &str) -> Result<()> {
    let actions_dir = global_config_service::actions_dir()?;
    let path = actions_dir.join(format!("{}.toml", name));

    if !path.exists() {
        return Err(GranaryError::InvalidArgument(format!(
            "Action '{}' is not installed locally. Use `granary action install {}` first.",
            name, name
        )));
    }

    let content = fetch_action_from_registry(name).await?;

    // Validate it parses as ActionConfig
    let _action: ActionConfig = toml::from_str(&content).map_err(|e| {
        GranaryError::GlobalConfig(format!("Failed to parse registry action '{}': {}", name, e))
    })?;

    std::fs::write(&path, &content)?;

    println!("Updated action: {}", name);
    Ok(())
}

/// Result of a single step in an action run.
pub struct StepResultOutput {
    pub name: String,
    pub exit_code: i32,
    pub stdout: String,
}

/// Output for `action run` - per-step outcomes for pipelines or single action result.
pub struct ActionRunOutput {
    pub action_name: String,
    pub status: String,
    pub steps: Vec<StepResultOutput>,
}

impl Output for ActionRunOutput {
    fn to_json(&self) -> String {
        let steps_json: Vec<serde_json::Value> = self
            .steps
            .iter()
            .map(|s| {
                let mut v = serde_json::json!({
                    "name": s.name,
                    "exit_code": s.exit_code,
                });
                if s.stdout.is_empty() {
                    v["stdout"] = serde_json::json!("");
                } else if s.stdout.len() > 200 {
                    v["stdout_bytes"] = serde_json::json!(s.stdout.len());
                } else {
                    v["stdout"] = serde_json::json!(s.stdout);
                }
                v
            })
            .collect();
        serde_json::json!({
            "action": self.action_name,
            "status": self.status,
            "steps": steps_json,
        })
        .to_string()
    }

    fn to_prompt(&self) -> String {
        let total = self.steps.len();
        let succeeded = self.steps.iter().filter(|s| s.exit_code == 0).count();

        if total == 1 {
            let step = &self.steps[0];
            if step.exit_code == 0 {
                let mut msg = format!("Action \"{}\" completed successfully.", self.action_name);
                if !step.stdout.is_empty() {
                    msg.push_str(&format!(" Output: \"{}\".", step.stdout));
                }
                msg
            } else {
                format!(
                    "Action \"{}\" failed with exit code {}.",
                    self.action_name, step.exit_code
                )
            }
        } else {
            let mut msg = format!(
                "Pipeline \"{}\" {}. {}/{} steps passed.",
                self.action_name,
                if self.status == "completed" {
                    "completed successfully"
                } else {
                    "failed"
                },
                succeeded,
                total,
            );
            for step in &self.steps {
                if !step.stdout.is_empty() && step.stdout.len() <= 200 {
                    msg.push_str(&format!(
                        "\nStep outputs: {} produced \"{}\".",
                        step.name, step.stdout
                    ));
                }
            }
            msg
        }
    }

    fn to_text(&self) -> String {
        let total = self.steps.len();
        let succeeded = self.steps.iter().filter(|s| s.exit_code == 0).count();

        let mut lines = vec![];
        for (i, step) in self.steps.iter().enumerate() {
            let status_str = if step.exit_code == 0 { "ok" } else { "FAIL" };
            let stdout_display = if step.stdout.is_empty() {
                String::new()
            } else if step.stdout.len() > 80 {
                format!("   ({} bytes)", step.stdout.len())
            } else {
                format!("   {}", step.stdout)
            };
            lines.push(format!(
                "  Step {}/{}  {}    {}{}",
                i + 1,
                total,
                step.name,
                status_str,
                stdout_display,
            ));
        }

        lines.push(String::new());
        if total == 1 {
            if succeeded == 1 {
                lines.push("  Action completed successfully.".to_string());
            } else {
                lines.push(format!(
                    "  Action failed (exit code {}).",
                    self.steps[0].exit_code
                ));
            }
        } else {
            lines.push(format!(
                "  Pipeline {} ({}/{} steps succeeded)",
                self.status, succeeded, total,
            ));
        }
        lines.join("\n")
    }
}

/// Build a synthetic event from --set key=value vars.
fn build_synthetic_event(vars: &[String]) -> Event {
    let mut payload_map = serde_json::Map::new();
    for var in vars {
        if let Some((key, value)) = var.split_once('=') {
            payload_map.insert(
                key.to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }
    Event {
        id: 0,
        event_type: "action.run".to_string(),
        entity_type: "action".to_string(),
        entity_id: "cli".to_string(),
        actor: Some("cli".to_string()),
        session_id: None,
        payload: serde_json::to_string(&payload_map).unwrap_or_else(|_| "{}".to_string()),
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

/// Resolved fields for a single pipeline step: (command, args, env, cwd).
type ResolvedStep = (String, Vec<String>, HashMap<String, String>, Option<String>);

/// Resolve a step's effective command, args, env, and cwd by merging
/// the step config with its referenced action (if any).
fn resolve_step(
    step: &crate::models::StepConfig,
    pipeline_env: &HashMap<String, String>,
) -> Result<ResolvedStep> {
    let (command, args, action_env) = if let Some(ref action_name) = step.action {
        let action = global_config_service::get_action(action_name)?.ok_or_else(|| {
            GranaryError::InvalidArgument(format!(
                "Step references action '{}' which was not found",
                action_name
            ))
        })?;
        let cmd = step
            .command
            .clone()
            .or(action.command.clone())
            .ok_or_else(|| {
                GranaryError::InvalidArgument(format!(
                    "Action '{}' has no command defined",
                    action_name
                ))
            })?;
        let args = step.args.clone().unwrap_or(action.args.clone());
        (cmd, args, action.env.clone())
    } else if let Some(ref cmd) = step.command {
        (
            cmd.clone(),
            step.args.clone().unwrap_or_default(),
            HashMap::new(),
        )
    } else {
        return Err(GranaryError::InvalidArgument(
            "Step has neither 'action' nor 'command' set".to_string(),
        ));
    };

    // Merge env: pipeline-level < action-level < step-level
    let mut env = pipeline_env.clone();
    env.extend(action_env);
    if let Some(ref step_env) = step.env {
        env.extend(step_env.clone());
    }

    Ok((command, args, env, step.cwd.clone()))
}

/// Execute `granary action run`.
pub async fn run_action(
    name: &str,
    vars: &[String],
    cwd: Option<PathBuf>,
    dry_run: bool,
    format: Option<CliOutputFormat>,
) -> Result<()> {
    let action_config = global_config_service::get_action(name)?
        .ok_or_else(|| GranaryError::InvalidArgument(format!("Action '{}' not found", name)))?;

    if let Err(e) = action_config.validate() {
        return Err(GranaryError::InvalidArgument(format!(
            "Invalid action '{}': {}",
            name, e
        )));
    }

    let working_dir =
        cwd.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let event = build_synthetic_event(vars);
    let mut pipeline_ctx = PipelineContext::new();
    let mut step_results: Vec<StepResultOutput> = vec![];

    if action_config.is_pipeline() {
        let steps = action_config.steps.as_ref().unwrap();
        let total = steps.len();

        for (i, step) in steps.iter().enumerate() {
            let step_name = step.resolved_name(i);
            let (command, args, env, step_cwd_template) = resolve_step(step, &action_config.env)?;

            // Resolve templates in command and args
            let resolved_command = substitute_with_context(&command, &event, Some(&pipeline_ctx))?;
            let resolved_args = substitute_all_with_context(&args, &event, Some(&pipeline_ctx))?;

            // Resolve cwd template if set
            let step_working_dir = if let Some(ref cwd_tmpl) = step_cwd_template {
                let resolved = substitute_with_context(cwd_tmpl, &event, Some(&pipeline_ctx))?;
                PathBuf::from(resolved)
            } else {
                working_dir.clone()
            };

            // Resolve env var templates
            let resolved_env: Vec<(String, String)> = env
                .iter()
                .map(|(k, v)| {
                    let rv = substitute_with_context(v, &event, Some(&pipeline_ctx))
                        .unwrap_or_else(|_| v.clone());
                    (k.clone(), rv)
                })
                .collect();

            if dry_run {
                println!("  Step {}/{}  {}", i + 1, total, step_name,);
                println!("    {} {}", resolved_command, resolved_args.join(" "));
                if step_working_dir != working_dir {
                    println!("    cwd: {}", step_working_dir.display());
                }
                if !resolved_env.is_empty() {
                    for (k, v) in &resolved_env {
                        println!("    env: {}={}", k, v);
                    }
                }
                println!();
                // For dry-run, record a fake successful step so later steps can reference it
                pipeline_ctx.add_step(
                    step_name.clone(),
                    StepOutput {
                        stdout: format!("<dry-run:{}>", step_name),
                        exit_code: 0,
                    },
                );
                continue;
            }

            // Create a temp log file for stderr
            let log_dir = std::env::temp_dir().join("granary-action-run");
            std::fs::create_dir_all(&log_dir)?;
            let log_file = log_dir.join(format!(
                "{}-{}.log",
                name.replace('/', "-"),
                step_name.replace('/', "-")
            ));

            let handle = spawn_runner_piped(
                &resolved_command,
                &resolved_args,
                &step_working_dir,
                &resolved_env,
                &log_file,
            )
            .await?;

            let PipelineStepResult { stdout, exit_code } = handle.wait().await?;

            pipeline_ctx.add_step(
                step_name.clone(),
                StepOutput {
                    stdout: stdout.clone(),
                    exit_code,
                },
            );

            step_results.push(StepResultOutput {
                name: step_name.clone(),
                exit_code,
                stdout,
            });

            // Check on_error policy
            if exit_code != 0 {
                let on_error = step.on_error.as_ref().unwrap_or(&OnError::Stop);
                if *on_error == OnError::Stop {
                    // Pipeline stops here
                    let output = ActionRunOutput {
                        action_name: name.to_string(),
                        status: "failed".to_string(),
                        steps: step_results,
                    };
                    println!("{}", output.format(format));
                    std::process::exit(1);
                }
            }
        }

        if !dry_run {
            // Only count a step as a hard failure if its on_error policy is Stop (the default).
            // Steps with on_error=Continue that exited non-zero are expected and should not
            // cause the overall pipeline to be marked as failed.
            let has_hard_failure = steps.iter().zip(step_results.iter()).any(|(step, result)| {
                result.exit_code != 0
                    && *step.on_error.as_ref().unwrap_or(&OnError::Stop) == OnError::Stop
            });
            let output = ActionRunOutput {
                action_name: name.to_string(),
                status: if has_hard_failure {
                    "failed".to_string()
                } else {
                    "completed".to_string()
                },
                steps: step_results,
            };
            println!("{}", output.format(format));
            if has_hard_failure {
                std::process::exit(1);
            }
        }
    } else {
        // Simple action
        let command = action_config.command.as_deref().ok_or_else(|| {
            GranaryError::InvalidArgument(format!("Action '{}' has no command defined", name))
        })?;

        let resolved_command = substitute_with_context(command, &event, None)?;
        let resolved_args = substitute_all_with_context(&action_config.args, &event, None)?;
        let resolved_env: Vec<(String, String)> = action_config
            .env
            .iter()
            .map(|(k, v)| {
                let rv = substitute_with_context(v, &event, None).unwrap_or_else(|_| v.clone());
                (k.clone(), rv)
            })
            .collect();

        if dry_run {
            println!("  {} {}", resolved_command, resolved_args.join(" "));
            if !resolved_env.is_empty() {
                for (k, v) in &resolved_env {
                    println!("  env: {}={}", k, v);
                }
            }
            return Ok(());
        }

        let log_dir = std::env::temp_dir().join("granary-action-run");
        std::fs::create_dir_all(&log_dir)?;
        let log_file = log_dir.join(format!("{}.log", name.replace('/', "-")));

        let handle = spawn_runner_piped(
            &resolved_command,
            &resolved_args,
            &working_dir,
            &resolved_env,
            &log_file,
        )
        .await?;

        let PipelineStepResult { stdout, exit_code } = handle.wait().await?;

        let output = ActionRunOutput {
            action_name: name.to_string(),
            status: if exit_code == 0 {
                "completed".to_string()
            } else {
                "failed".to_string()
            },
            steps: vec![StepResultOutput {
                name: name.to_string(),
                exit_code,
                stdout,
            }],
        };
        println!("{}", output.format(format));
        if exit_code != 0 {
            std::process::exit(1);
        }
    }

    Ok(())
}
