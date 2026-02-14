use crate::cli::args::{
    ActionsAction, CliOutputFormat, ConfigAction, RunnersAction, SteeringAction,
};
use crate::db;
use crate::error::Result;
use crate::models::{ActionConfig, RunnerConfig};
use crate::output::Output;
use crate::services::{Workspace, global_config_service};
use std::collections::HashMap;

/// Output for `config get` - wraps a resolved config subtree
pub struct ConfigGetOutput {
    pub value: serde_json::Value,
    pub key: Option<String>,
}

impl Output for ConfigGetOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.value).unwrap_or_else(|_| "null".to_string())
    }

    fn to_prompt(&self) -> String {
        match &self.key {
            Some(k) => format!("Config `{}`:\n{}", k, self.format_value_text(&self.value)),
            None => format!("Global config:\n{}", self.format_value_text(&self.value)),
        }
    }

    fn to_text(&self) -> String {
        self.format_value_text(&self.value)
    }
}

impl ConfigGetOutput {
    fn format_value_text(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => "(not set)".to_string(),
            // For objects/arrays, use pretty TOML-like display via JSON pretty-print
            _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| format!("{}", value)),
        }
    }
}

/// Output for `config set` - confirmation message
pub struct ConfigSetOutput {
    pub key: String,
    pub value: String,
}

impl Output for ConfigSetOutput {
    fn to_json(&self) -> String {
        serde_json::json!({"status": "set", "key": &self.key, "value": &self.value}).to_string()
    }

    fn to_prompt(&self) -> String {
        format!("Set `{}` = `{}`", self.key, self.value)
    }

    fn to_text(&self) -> String {
        format!("Set {} = {}", self.key, self.value)
    }
}

/// Output for `config delete` - confirmation message
pub struct ConfigDeleteOutput {
    pub key: String,
    pub deleted: bool,
}

impl Output for ConfigDeleteOutput {
    fn to_json(&self) -> String {
        serde_json::json!({"status": if self.deleted { "deleted" } else { "not_found" }, "key": &self.key}).to_string()
    }

    fn to_prompt(&self) -> String {
        if self.deleted {
            format!("Deleted `{}`", self.key)
        } else {
            format!("Key not found: `{}`", self.key)
        }
    }

    fn to_text(&self) -> String {
        if self.deleted {
            format!("Deleted {}", self.key)
        } else {
            format!("Key not found: {}", self.key)
        }
    }
}

/// Output for `config list` - workspace key-value pairs
pub struct ConfigListOutput {
    pub items: Vec<(String, String)>,
}

impl Output for ConfigListOutput {
    fn to_json(&self) -> String {
        let entries: Vec<serde_json::Value> = self
            .items
            .iter()
            .map(|(k, v)| serde_json::json!({"key": k, "value": v}))
            .collect();
        serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
    }

    fn to_prompt(&self) -> String {
        if self.items.is_empty() {
            "No config values set.".to_string()
        } else {
            self.items
                .iter()
                .map(|(k, v)| format!("- `{}` = `{}`", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    fn to_text(&self) -> String {
        if self.items.is_empty() {
            "No config values set".to_string()
        } else {
            self.items
                .iter()
                .map(|(k, v)| format!("{} = {}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }
}

/// Handle config subcommands
pub async fn config(action: ConfigAction, format: Option<CliOutputFormat>) -> Result<()> {
    match action {
        // Global config get with dot-path access
        ConfigAction::Get { key } => {
            let value = global_config_service::get_by_path(key.as_deref())?;
            let output = ConfigGetOutput {
                value,
                key: key.clone(),
            };
            println!("{}", output.format(format));
        }

        // Workspace-level config commands
        ConfigAction::Set { key, value } => {
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
            db::config::set(&pool, &key, &value).await?;
            let output = ConfigSetOutput {
                key: key.clone(),
                value: value.clone(),
            };
            println!("{}", output.format(format));
        }

        ConfigAction::List => {
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
            let items = db::config::list(&pool).await?;
            let output = ConfigListOutput { items };
            println!("{}", output.format(format));
        }

        ConfigAction::Delete { key } => {
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
            let deleted = db::config::delete(&pool, &key).await?;
            let output = ConfigDeleteOutput {
                key: key.clone(),
                deleted,
            };
            println!("{}", output.format(format));
        }

        // Global config commands (don't need workspace)
        ConfigAction::Edit => {
            let path = global_config_service::config_path()?;
            println!("Opening {} in editor...", path.display());
            global_config_service::edit_config()?;
            println!("Config file saved.");
        }

        ConfigAction::Runners { action } => {
            handle_runners_action(action, format).await?;
        }

        ConfigAction::Actions { action } => {
            handle_actions_action(action, format).await?;
        }
    }

    Ok(())
}

/// Output for `config runners` (list) - all configured runners
pub struct RunnersListOutput {
    pub runners: HashMap<String, RunnerConfig>,
}

impl Output for RunnersListOutput {
    fn to_json(&self) -> String {
        serde_json::to_string(&self.runners).unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        if self.runners.is_empty() {
            "No runners configured.".to_string()
        } else {
            self.runners
                .iter()
                .map(|(name, runner)| {
                    format!(
                        "- `{}` -> `{} {}`",
                        name,
                        runner.command,
                        runner.args.join(" ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    fn to_text(&self) -> String {
        if self.runners.is_empty() {
            "No runners configured.\n\nAdd a runner with:\n  granary config runners add <name> --command <cmd>".to_string()
        } else {
            let mut lines = vec!["Configured runners:\n".to_string()];
            for (name, runner) in &self.runners {
                lines.push(format!(
                    "  {} -> {} {}",
                    name,
                    runner.command,
                    runner.args.join(" ")
                ));
                if let Some(c) = runner.concurrency {
                    lines.push(format!("    concurrency: {}", c));
                }
                if let Some(ref on) = runner.on {
                    lines.push(format!("    on: {}", on));
                }
                if !runner.env.is_empty() {
                    lines.push(format!(
                        "    env: {}",
                        runner
                            .env
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            lines.join("\n")
        }
    }
}

/// Output for `config runners show` - single runner details
pub struct RunnerShowOutput {
    pub name: String,
    pub runner: RunnerConfig,
}

impl Output for RunnerShowOutput {
    fn to_json(&self) -> String {
        serde_json::json!({"name": &self.name, "runner": &self.runner}).to_string()
    }

    fn to_prompt(&self) -> String {
        format!(
            "Runner `{}`:\n- command: `{}`",
            self.name, self.runner.command
        )
    }

    fn to_text(&self) -> String {
        let mut lines = vec![
            format!("Runner: {}\n", self.name),
            format!("  command: {}", self.runner.command),
        ];
        if !self.runner.args.is_empty() {
            lines.push(format!("  args: {:?}", self.runner.args));
        }
        if let Some(c) = self.runner.concurrency {
            lines.push(format!("  concurrency: {}", c));
        }
        if let Some(ref on) = self.runner.on {
            lines.push(format!("  on: {}", on));
        }
        if !self.runner.env.is_empty() {
            lines.push("  env:".to_string());
            for (k, v) in &self.runner.env {
                lines.push(format!("    {}={}", k, v));
            }
        }
        lines.join("\n")
    }
}

/// Output for `config runners rm` - removal confirmation
pub struct RunnerRmOutput {
    pub name: String,
    pub removed: bool,
}

impl Output for RunnerRmOutput {
    fn to_json(&self) -> String {
        serde_json::json!({"status": if self.removed { "removed" } else { "not_found" }, "name": &self.name}).to_string()
    }

    fn to_prompt(&self) -> String {
        if self.removed {
            format!("Removed runner `{}`", self.name)
        } else {
            format!("Runner not found: `{}`", self.name)
        }
    }

    fn to_text(&self) -> String {
        if self.removed {
            format!("Removed runner: {}", self.name)
        } else {
            format!("Runner not found: {}", self.name)
        }
    }
}

/// Output for `config actions` (list) - all configured actions
pub struct ActionsListOutput {
    pub actions: Vec<(String, ActionConfig)>,
}

impl Output for ActionsListOutput {
    fn to_json(&self) -> String {
        let map: HashMap<&str, &ActionConfig> =
            self.actions.iter().map(|(n, a)| (n.as_str(), a)).collect();
        serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        if self.actions.is_empty() {
            "No actions configured.".to_string()
        } else {
            self.actions
                .iter()
                .map(|(name, action)| {
                    format!(
                        "- `{}` -> `{} {}`",
                        name,
                        action.command,
                        action.args.join(" ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    fn to_text(&self) -> String {
        if self.actions.is_empty() {
            "No actions configured.\n\nAdd an action with:\n  granary config actions add <name> --command <cmd>".to_string()
        } else {
            let mut lines = vec!["Configured actions:\n".to_string()];
            for (name, action) in &self.actions {
                lines.push(format!(
                    "  {} -> {} {}",
                    name,
                    action.command,
                    action.args.join(" ")
                ));
                if let Some(c) = action.concurrency {
                    lines.push(format!("    concurrency: {}", c));
                }
                if let Some(ref on) = action.on {
                    lines.push(format!("    on: {}", on));
                }
                if !action.env.is_empty() {
                    lines.push(format!(
                        "    env: {}",
                        action
                            .env
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }
            lines.join("\n")
        }
    }
}

/// Output for `config actions show` - single action details
pub struct ActionShowOutput {
    pub name: String,
    pub action: ActionConfig,
}

impl Output for ActionShowOutput {
    fn to_json(&self) -> String {
        serde_json::json!({"name": &self.name, "action": &self.action}).to_string()
    }

    fn to_prompt(&self) -> String {
        format!(
            "Action `{}`:\n- command: `{}`",
            self.name, self.action.command
        )
    }

    fn to_text(&self) -> String {
        let mut lines = vec![
            format!("Action: {}\n", self.name),
            format!("  command: {}", self.action.command),
        ];
        if !self.action.args.is_empty() {
            lines.push(format!("  args: {:?}", self.action.args));
        }
        if let Some(c) = self.action.concurrency {
            lines.push(format!("  concurrency: {}", c));
        }
        if let Some(ref on) = self.action.on {
            lines.push(format!("  on: {}", on));
        }
        if !self.action.env.is_empty() {
            lines.push("  env:".to_string());
            for (k, v) in &self.action.env {
                lines.push(format!("    {}={}", k, v));
            }
        }
        lines.join("\n")
    }
}

/// Output for `config actions rm` - removal confirmation
pub struct ActionRmOutput {
    pub name: String,
    pub removed: bool,
}

impl Output for ActionRmOutput {
    fn to_json(&self) -> String {
        serde_json::json!({"status": if self.removed { "removed" } else { "not_found" }, "name": &self.name}).to_string()
    }

    fn to_prompt(&self) -> String {
        if self.removed {
            format!("Removed action `{}`", self.name)
        } else {
            format!("Action not found: `{}`", self.name)
        }
    }

    fn to_text(&self) -> String {
        if self.removed {
            format!("Removed action: {}", self.name)
        } else {
            format!("Action not found: {}", self.name)
        }
    }
}

/// Handle actions subcommands
async fn handle_actions_action(
    action: Option<ActionsAction>,
    format: Option<CliOutputFormat>,
) -> Result<()> {
    match action {
        None => {
            // List all actions (inline + file-based)
            let actions = global_config_service::list_all_actions()?;
            let output = ActionsListOutput { actions };
            println!("{}", output.format(format));
        }

        Some(ActionsAction::Add {
            name,
            command,
            args,
            concurrency,
            on,
            env_vars,
        }) => {
            let env = parse_env_vars(&env_vars);
            let action = ActionConfig {
                command,
                args,
                concurrency,
                on,
                env,
                action: None,
            };
            global_config_service::set_action(&name, action)?;
            println!("Added action: {}", name);
        }

        Some(ActionsAction::Update {
            name,
            command,
            args,
            concurrency,
            on,
            env_vars,
        }) => {
            let existing = global_config_service::get_action(&name)?;
            match existing {
                Some(mut action) => {
                    if let Some(cmd) = command {
                        action.command = cmd;
                    }
                    if let Some(a) = args {
                        action.args = a;
                    }
                    if concurrency.is_some() {
                        action.concurrency = concurrency;
                    }
                    if on.is_some() {
                        action.on = on;
                    }
                    if let Some(env_vec) = env_vars {
                        action.env = parse_env_vars(&env_vec);
                    }
                    global_config_service::set_action(&name, action)?;
                    println!("Updated action: {}", name);
                }
                None => {
                    println!("Action not found: {}", name);
                    std::process::exit(3);
                }
            }
        }

        Some(ActionsAction::Rm { name }) => {
            let removed = global_config_service::remove_action(&name)?;
            let output = ActionRmOutput {
                name: name.clone(),
                removed,
            };
            println!("{}", output.format(format));
            if !removed {
                std::process::exit(3);
            }
        }

        Some(ActionsAction::Show { name }) => match global_config_service::get_action(&name)? {
            Some(action) => {
                let output = ActionShowOutput {
                    name: name.clone(),
                    action,
                };
                println!("{}", output.format(format));
            }
            None => {
                println!("Action not found: {}", name);
                std::process::exit(3);
            }
        },
    }

    Ok(())
}

/// Handle runners subcommands
async fn handle_runners_action(
    action: Option<RunnersAction>,
    format: Option<CliOutputFormat>,
) -> Result<()> {
    match action {
        None => {
            // List all runners
            let config = global_config_service::load()?;
            let output = RunnersListOutput {
                runners: config.runners,
            };
            println!("{}", output.format(format));
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
                action: None,
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
            let output = RunnerRmOutput {
                name: name.clone(),
                removed,
            };
            println!("{}", output.format(format));
            if !removed {
                std::process::exit(3);
            }
        }

        Some(RunnersAction::Show { name }) => match global_config_service::get_runner(&name)? {
            Some(runner) => {
                let output = RunnerShowOutput {
                    name: name.clone(),
                    runner,
                };
                println!("{}", output.format(format));
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

/// Output for `steering list` - all steering files
pub struct SteeringListOutput {
    pub files: Vec<db::steering::SteeringFile>,
}

impl Output for SteeringListOutput {
    fn to_json(&self) -> String {
        serde_json::to_string(&self.files).unwrap_or_else(|_| "[]".to_string())
    }

    fn to_prompt(&self) -> String {
        if self.files.is_empty() {
            "No steering files configured.".to_string()
        } else {
            self.files
                .iter()
                .map(|f| format!("- `{}` [{}]", f.path, f.scope_display()))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    fn to_text(&self) -> String {
        if self.files.is_empty() {
            "No steering files configured".to_string()
        } else {
            let mut lines = vec!["Steering files:".to_string()];
            for file in &self.files {
                lines.push(format!("  {} [{}]", file.path, file.scope_display()));
            }
            lines.join("\n")
        }
    }
}

/// Output for `steering add` - confirmation
pub struct SteeringAddOutput {
    pub path: String,
    pub scope_display: String,
}

impl Output for SteeringAddOutput {
    fn to_json(&self) -> String {
        serde_json::json!({"status": "added", "path": &self.path, "scope": &self.scope_display})
            .to_string()
    }

    fn to_prompt(&self) -> String {
        format!(
            "Added steering file: `{}` [{}]",
            self.path, self.scope_display
        )
    }

    fn to_text(&self) -> String {
        format!(
            "Added steering file: {} [{}]",
            self.path, self.scope_display
        )
    }
}

/// Output for `steering rm` - removal confirmation
pub struct SteeringRmOutput {
    pub path: String,
    pub removed: bool,
}

impl Output for SteeringRmOutput {
    fn to_json(&self) -> String {
        serde_json::json!({"status": if self.removed { "removed" } else { "not_found" }, "path": &self.path}).to_string()
    }

    fn to_prompt(&self) -> String {
        if self.removed {
            format!("Removed steering file: `{}`", self.path)
        } else {
            format!("Steering file not found: `{}`", self.path)
        }
    }

    fn to_text(&self) -> String {
        if self.removed {
            format!("Removed steering file: {}", self.path)
        } else {
            format!("Steering file not found: {}", self.path)
        }
    }
}

/// Handle steering subcommands
pub async fn steering(action: SteeringAction, format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    match action {
        SteeringAction::List => {
            let files = db::steering::list(&pool).await?;
            let output = SteeringListOutput { files };
            println!("{}", output.format(format));
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
            let output = SteeringAddOutput {
                path: path.clone(),
                scope_display,
            };
            println!("{}", output.format(format));
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

            let output = SteeringRmOutput {
                path: path.clone(),
                removed,
            };
            println!("{}", output.format(format));
        }
    }

    Ok(())
}
