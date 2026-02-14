//! Async functions for granary CLI interaction

use tracing::error;

use granary_types::{
    ActionConfig, Comment, Initiative, InitiativeSummary, Project, Run, RunnerConfig,
    Task as GranaryTask, TaskDependency, TaskPriority, TaskStatus, Worker,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

// =============================================================================
// Error Type
// =============================================================================

/// Error type for CLI operations
#[derive(Debug, Clone)]
pub struct CliError(pub String);

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CliError {}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        CliError(s)
    }
}

impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        CliError(s.to_string())
    }
}

// =============================================================================
// Core CLI Execution
// =============================================================================

/// Execute a granary command and return raw JSON output
async fn execute_granary(args: &[&str], workspace: &Path) -> Result<String, CliError> {
    let output = Command::new("granary")
        .args(args)
        .arg("--format")
        .arg("json")
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| CliError(format!("Failed to execute granary: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError(format!("Granary command failed: {}", stderr)));
    }

    String::from_utf8(output.stdout).map_err(|e| CliError(format!("Invalid UTF-8 output: {}", e)))
}

/// Execute a granary command without JSON output (for commands that don't return data)
#[allow(dead_code)]
async fn execute_granary_no_json(args: &[&str], workspace: &Path) -> Result<String, CliError> {
    let output = Command::new("granary")
        .args(args)
        .current_dir(workspace)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| CliError(format!("Failed to execute granary: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CliError(format!("Granary command failed: {}", stderr)));
    }

    String::from_utf8(output.stdout).map_err(|e| CliError(format!("Invalid UTF-8 output: {}", e)))
}

// =============================================================================
// Primary List Functions (as per task requirements)
// =============================================================================

/// List all projects in the workspace
pub async fn list_projects(workspace: &Path) -> Result<serde_json::Value, CliError> {
    let output = execute_granary(&["projects", "list"], workspace).await?;
    parse_json_logged(&output, "projects list").map_err(CliError)
}

/// List all initiatives in the workspace
pub async fn list_initiatives(workspace: &Path) -> Result<serde_json::Value, CliError> {
    let output = execute_granary(&["initiatives", "list"], workspace).await?;
    parse_json_logged(&output, "initiatives list").map_err(CliError)
}

/// List all tasks, optionally filtered by project
pub async fn list_tasks(
    workspace: &Path,
    project_id: Option<&str>,
) -> Result<serde_json::Value, CliError> {
    let mut args = vec!["tasks", "list"];
    if let Some(id) = project_id {
        args.push("--project");
        args.push(id);
    }
    let output = execute_granary(&args, workspace).await?;
    parse_json_logged(&output, "tasks list").map_err(CliError)
}

/// Get workers status
pub async fn list_workers(workspace: &Path) -> Result<serde_json::Value, CliError> {
    let output = execute_granary(&["workers", "list"], workspace).await?;
    parse_json_logged(&output, "workers list").map_err(CliError)
}

/// Steering file information returned from CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteeringFile {
    pub id: i64,
    pub path: String,
    pub mode: String,
    pub scope_type: Option<String>,
    pub scope_id: Option<String>,
    pub created_at: String,
}

/// Config key-value entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
}

/// Runner with its name (used for list output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerWithName {
    pub name: String,
    #[serde(flatten)]
    pub config: RunnerConfig,
}

pub async fn run_granary(workspace: &PathBuf, args: &[&str]) -> Result<String, String> {
    let cmd_str = format!("granary {}", args.join(" "));
    let output = Command::new("granary")
        .args(args)
        .current_dir(workspace)
        .output()
        .await
        .map_err(|e| format!("Failed to run '{}': {}", cmd_str, e))?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .map_err(|e| format!("Invalid UTF-8 from '{}': {}", cmd_str, e))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Command '{}' failed: {}", cmd_str, stderr))
    }
}

pub async fn load_projects(workspace: PathBuf) -> Result<Vec<Project>, String> {
    let output = run_granary(&workspace, &["projects", "--all", "--json"]).await?;
    parse_json_logged(&output, "projects --json")
}

/// Parse JSON with logging on failure
fn parse_json_logged<T: serde::de::DeserializeOwned>(
    output: &str,
    context: &str,
) -> Result<T, String> {
    serde_json::from_str(output).map_err(|e| {
        error!(
            context = context,
            error = %e,
            raw_output = output,
            "Failed to parse CLI output"
        );
        format!("Parse error for '{}': {}", context, e)
    })
}

pub async fn load_tasks(
    workspace: PathBuf,
    project_id: String,
) -> Result<Vec<GranaryTask>, String> {
    let output = run_granary(&workspace, &["project", &project_id, "tasks", "--json"]).await?;
    parse_json_logged(&output, &format!("project {} tasks --json", project_id))
}

/// Re-open a task (move from done/blocked back to todo)
pub async fn reopen_task(workspace: PathBuf, task_id: String) -> Result<(), String> {
    // Use the update command to set status to todo
    run_granary(
        &workspace,
        &["task", &task_id, "update", "--status", "todo"],
    )
    .await?;
    Ok(())
}

pub async fn create_task(
    workspace: PathBuf,
    project_id: String,
    title: String,
) -> Result<(), String> {
    run_granary(
        &workspace,
        &["project", &project_id, "tasks", "create", &title],
    )
    .await?;
    Ok(())
}

pub async fn start_task(workspace: PathBuf, task_id: String) -> Result<(), String> {
    run_granary(&workspace, &["task", &task_id, "start"]).await?;
    Ok(())
}

pub async fn complete_task(workspace: PathBuf, task_id: String) -> Result<(), String> {
    run_granary(&workspace, &["task", &task_id, "done"]).await?;
    Ok(())
}

pub async fn archive_project(workspace: PathBuf, project_id: String) -> Result<(), String> {
    run_granary(&workspace, &["project", &project_id, "archive"]).await?;
    Ok(())
}

pub async fn ready_project(workspace: PathBuf, project_id: String) -> Result<(), String> {
    run_granary(&workspace, &["project", &project_id, "ready"]).await?;
    Ok(())
}

pub async fn unarchive_project(workspace: PathBuf, project_id: String) -> Result<(), String> {
    run_granary(
        &workspace,
        &["project", &project_id, "update", "--status", "active"],
    )
    .await?;
    Ok(())
}

pub async fn create_project(
    workspace: PathBuf,
    name: String,
    description: Option<String>,
    owner: Option<String>,
    tags: Option<String>,
) -> Result<(), String> {
    let mut args = vec!["projects", "create", &name];

    let desc_flag;
    if let Some(ref d) = description {
        desc_flag = d.as_str();
        args.push("--description");
        args.push(desc_flag);
    }

    let owner_flag;
    if let Some(ref o) = owner {
        owner_flag = o.as_str();
        args.push("--owner");
        args.push(owner_flag);
    }

    let tags_flag;
    if let Some(ref t) = tags {
        tags_flag = t.as_str();
        args.push("--tags");
        args.push(tags_flag);
    }

    run_granary(&workspace, &args).await?;
    Ok(())
}

pub async fn update_project(
    workspace: PathBuf,
    project_id: String,
    name: Option<String>,
    description: Option<String>,
    owner: Option<String>,
    status: Option<String>,
    tags: Option<String>,
) -> Result<(), String> {
    let mut args = vec!["project", project_id.as_str(), "update"];

    let name_flag;
    if let Some(ref n) = name {
        name_flag = n.as_str();
        args.push("--name");
        args.push(name_flag);
    }

    let desc_flag;
    if let Some(ref d) = description {
        desc_flag = d.as_str();
        args.push("--description");
        args.push(desc_flag);
    }

    let owner_flag;
    if let Some(ref o) = owner {
        owner_flag = o.as_str();
        args.push("--owner");
        args.push(owner_flag);
    }

    let status_flag;
    if let Some(ref s) = status {
        status_flag = s.as_str();
        args.push("--status");
        args.push(status_flag);
    }

    let tags_flag;
    if let Some(ref t) = tags {
        tags_flag = t.as_str();
        args.push("--tags");
        args.push(tags_flag);
    }

    run_granary(&workspace, &args).await?;
    Ok(())
}

/// Fetch worker logs
///
/// # Arguments
/// - `workspace`: Path to the granary workspace
/// - `worker_id`: Worker ID to fetch logs for
/// - `lines`: Number of lines from end (default 50)
pub async fn load_worker_logs(
    workspace: PathBuf,
    worker_id: String,
    lines: Option<u32>,
) -> Result<Vec<String>, String> {
    let lines_arg = lines.unwrap_or(50).to_string();
    let output = run_granary(
        &workspace,
        &["worker", &worker_id, "logs", "-n", &lines_arg],
    )
    .await?;
    Ok(output.lines().map(String::from).collect())
}

/// Fetch run logs
///
/// # Arguments
/// - `workspace`: Path to the granary workspace
/// - `run_id`: Run ID to fetch logs for
/// - `lines`: Number of lines from end (default 100)
pub async fn load_run_logs(
    workspace: PathBuf,
    run_id: String,
    lines: Option<u32>,
) -> Result<Vec<String>, String> {
    let lines_arg = lines.unwrap_or(100).to_string();
    let output = run_granary(&workspace, &["run", &run_id, "logs", "-n", &lines_arg]).await?;
    Ok(output.lines().map(String::from).collect())
}

// =============================================================================
// Runner Management
// =============================================================================

/// List all configured runners
pub async fn list_runners(workspace: PathBuf) -> Result<HashMap<String, RunnerConfig>, String> {
    let output = run_granary(&workspace, &["config", "runners", "--json"]).await?;
    parse_json_logged(&output, "config runners --json")
}

/// Add a new runner configuration
pub async fn add_runner(
    workspace: PathBuf,
    name: String,
    command: String,
    args: Option<Vec<String>>,
    on_event: Option<String>,
    concurrency: Option<u32>,
    env: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let mut cmd_args = vec!["config", "runners", "add", &name, "--command", &command];

    // Store string versions to keep them alive
    let args_strs: Vec<String>;
    if let Some(ref a) = args {
        args_strs = a.clone();
        for arg in &args_strs {
            cmd_args.push("-a");
            cmd_args.push(arg);
        }
    }

    let on_flag;
    if let Some(ref o) = on_event {
        on_flag = o.as_str();
        cmd_args.push("--on");
        cmd_args.push(on_flag);
    }

    let concurrency_str;
    if let Some(c) = concurrency {
        concurrency_str = c.to_string();
        cmd_args.push("--concurrency");
        cmd_args.push(&concurrency_str);
    }

    let env_strs: Vec<String>;
    if let Some(ref e) = env {
        env_strs = e.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        for env_str in &env_strs {
            cmd_args.push("-e");
            cmd_args.push(env_str);
        }
    }

    run_granary(&workspace, &cmd_args).await?;
    Ok(())
}

/// Update an existing runner configuration
pub async fn update_runner(
    workspace: PathBuf,
    name: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    on_event: Option<String>,
    concurrency: Option<u32>,
    env: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let mut cmd_args = vec!["config", "runners", "update", &name];

    let command_flag;
    if let Some(ref c) = command {
        command_flag = c.as_str();
        cmd_args.push("--command");
        cmd_args.push(command_flag);
    }

    let args_strs: Vec<String>;
    if let Some(ref a) = args {
        args_strs = a.clone();
        for arg in &args_strs {
            cmd_args.push("-a");
            cmd_args.push(arg);
        }
    }

    let on_flag;
    if let Some(ref o) = on_event {
        on_flag = o.as_str();
        cmd_args.push("--on");
        cmd_args.push(on_flag);
    }

    let concurrency_str;
    if let Some(c) = concurrency {
        concurrency_str = c.to_string();
        cmd_args.push("--concurrency");
        cmd_args.push(&concurrency_str);
    }

    let env_strs: Vec<String>;
    if let Some(ref e) = env {
        env_strs = e.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        for env_str in &env_strs {
            cmd_args.push("-e");
            cmd_args.push(env_str);
        }
    }

    run_granary(&workspace, &cmd_args).await?;
    Ok(())
}

/// Remove a runner configuration
pub async fn remove_runner(workspace: PathBuf, name: String) -> Result<(), String> {
    run_granary(&workspace, &["config", "runners", "rm", &name]).await?;
    Ok(())
}

// =============================================================================
// Action Management
// =============================================================================

/// List all configured actions
pub async fn list_actions(workspace: PathBuf) -> Result<HashMap<String, ActionConfig>, String> {
    let output = run_granary(&workspace, &["config", "actions", "--json"]).await?;
    parse_json_logged(&output, "config actions --json")
}

/// Add a new action configuration
pub async fn add_action(
    workspace: PathBuf,
    name: String,
    command: String,
    args: Option<Vec<String>>,
    on_event: Option<String>,
    concurrency: Option<u32>,
    env: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let mut cmd_args = vec!["config", "actions", "add", &name, "--command", &command];

    let args_strs: Vec<String>;
    if let Some(ref a) = args {
        args_strs = a.clone();
        for arg in &args_strs {
            cmd_args.push("-a");
            cmd_args.push(arg);
        }
    }

    let on_flag;
    if let Some(ref o) = on_event {
        on_flag = o.as_str();
        cmd_args.push("--on");
        cmd_args.push(on_flag);
    }

    let concurrency_str;
    if let Some(c) = concurrency {
        concurrency_str = c.to_string();
        cmd_args.push("--concurrency");
        cmd_args.push(&concurrency_str);
    }

    let env_strs: Vec<String>;
    if let Some(ref e) = env {
        env_strs = e.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        for env_str in &env_strs {
            cmd_args.push("-e");
            cmd_args.push(env_str);
        }
    }

    run_granary(&workspace, &cmd_args).await?;
    Ok(())
}

/// Update an existing action configuration
pub async fn update_action(
    workspace: PathBuf,
    name: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    on_event: Option<String>,
    concurrency: Option<u32>,
    env: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let mut cmd_args = vec!["config", "actions", "update", &name];

    let command_flag;
    if let Some(ref c) = command {
        command_flag = c.as_str();
        cmd_args.push("--command");
        cmd_args.push(command_flag);
    }

    let args_strs: Vec<String>;
    if let Some(ref a) = args {
        args_strs = a.clone();
        for arg in &args_strs {
            cmd_args.push("-a");
            cmd_args.push(arg);
        }
    }

    let on_flag;
    if let Some(ref o) = on_event {
        on_flag = o.as_str();
        cmd_args.push("--on");
        cmd_args.push(on_flag);
    }

    let concurrency_str;
    if let Some(c) = concurrency {
        concurrency_str = c.to_string();
        cmd_args.push("--concurrency");
        cmd_args.push(&concurrency_str);
    }

    let env_strs: Vec<String>;
    if let Some(ref e) = env {
        env_strs = e.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
        for env_str in &env_strs {
            cmd_args.push("-e");
            cmd_args.push(env_str);
        }
    }

    run_granary(&workspace, &cmd_args).await?;
    Ok(())
}

/// Remove an action configuration
pub async fn remove_action(workspace: PathBuf, name: String) -> Result<(), String> {
    run_granary(&workspace, &["config", "actions", "rm", &name]).await?;
    Ok(())
}

/// Start a worker using a configured action
pub async fn start_worker_from_action(
    workspace: PathBuf,
    action_name: String,
    detached: bool,
) -> Result<Worker, String> {
    let mut args = vec!["worker", "start", "--action", &action_name, "--json"];
    if detached {
        args.push("-d");
    }
    let output = run_granary(&workspace, &args).await?;
    parse_json_logged(
        &output,
        &format!("worker start --action {} --json", action_name),
    )
}

// =============================================================================
// Steering File Management
// =============================================================================

/// List all steering files
pub async fn list_steering(workspace: PathBuf) -> Result<Vec<SteeringFile>, String> {
    let output = run_granary(&workspace, &["steering", "list", "--json"]).await?;
    parse_json_logged(&output, "steering list --json")
}

/// Add a steering file
pub async fn add_steering(
    workspace: PathBuf,
    path: String,
    mode: Option<String>,
    project: Option<String>,
    task: Option<String>,
    for_session: bool,
) -> Result<(), String> {
    let mut args = vec!["steering", "add", &path];

    let mode_flag;
    if let Some(ref m) = mode {
        mode_flag = m.as_str();
        args.push("--mode");
        args.push(mode_flag);
    }

    let project_flag;
    if let Some(ref p) = project {
        project_flag = p.as_str();
        args.push("--project");
        args.push(project_flag);
    }

    let task_flag;
    if let Some(ref t) = task {
        task_flag = t.as_str();
        args.push("--task");
        args.push(task_flag);
    }

    if for_session {
        args.push("--for-session");
    }

    run_granary(&workspace, &args).await?;
    Ok(())
}

/// Remove a steering file
pub async fn remove_steering(workspace: PathBuf, path: String) -> Result<(), String> {
    run_granary(&workspace, &["steering", "rm", &path]).await?;
    Ok(())
}

// =============================================================================
// Config Key-Value Management
// =============================================================================

/// List all config key-value pairs
pub async fn list_config(workspace: PathBuf) -> Result<Vec<ConfigEntry>, String> {
    let output = run_granary(&workspace, &["config", "list", "--json"]).await?;
    parse_json_logged(&output, "config list --json")
}

/// Set a config value
pub async fn set_config(workspace: PathBuf, key: String, value: String) -> Result<(), String> {
    run_granary(&workspace, &["config", "set", &key, &value]).await?;
    Ok(())
}

/// Delete a config key
pub async fn delete_config(workspace: PathBuf, key: String) -> Result<(), String> {
    run_granary(&workspace, &["config", "delete", &key]).await?;
    Ok(())
}

// =============================================================================
// Run Management
// =============================================================================

/// Load runs with optional filters
///
/// # Arguments
/// - `workspace`: Path to the granary workspace
/// - `worker_id`: Optional worker ID to filter by
/// - `status`: Optional status to filter by
pub async fn load_runs(
    workspace: PathBuf,
    worker_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<Run>, String> {
    let mut args = vec!["runs", "--all"];

    let worker_flag;
    if let Some(ref w) = worker_id {
        worker_flag = w.as_str();
        args.push("--worker");
        args.push(worker_flag);
    }

    let status_flag;
    if let Some(ref s) = status {
        status_flag = s.as_str();
        args.push("--status");
        args.push(status_flag);
    }

    args.push("--json");

    let output = run_granary(&workspace, &args).await?;
    parse_json_logged(&output, "runs --json")
}

/// Get a specific run by ID
pub async fn get_run(workspace: PathBuf, run_id: String) -> Result<Run, String> {
    let output = run_granary(&workspace, &["run", &run_id, "status", "--json"]).await?;
    parse_json_logged(&output, &format!("run {} status --json", run_id))
}

/// Stop a running run
pub async fn stop_run(workspace: PathBuf, run_id: String) -> Result<(), String> {
    run_granary(&workspace, &["run", &run_id, "stop"]).await?;
    Ok(())
}

/// Pause a running run
pub async fn pause_run(workspace: PathBuf, run_id: String) -> Result<(), String> {
    run_granary(&workspace, &["run", &run_id, "pause"]).await?;
    Ok(())
}

/// Resume a paused run
pub async fn resume_run(workspace: PathBuf, run_id: String) -> Result<(), String> {
    run_granary(&workspace, &["run", &run_id, "resume"]).await?;
    Ok(())
}

// =============================================================================
// Task CRUD Operations
// =============================================================================

/// Create task with full options
#[allow(clippy::too_many_arguments)]
pub async fn create_task_full(
    workspace: PathBuf,
    project_id: String,
    title: String,
    description: Option<String>,
    priority: Option<TaskPriority>,
    status: Option<TaskStatus>,
    owner: Option<String>,
    due_at: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<String, String> {
    let mut args = vec!["project", &project_id, "tasks", "create", &title];
    let desc_owned;
    let priority_owned;
    let status_owned;
    let owner_owned;
    let due_owned;
    let tags_owned;

    if let Some(d) = &description {
        desc_owned = d.clone();
        args.extend(&["--description", &desc_owned]);
    }
    if let Some(p) = &priority {
        priority_owned = p.as_str().to_string();
        args.extend(&["--priority", &priority_owned]);
    }
    if let Some(s) = &status {
        status_owned = s.as_str().to_string();
        args.extend(&["--status", &status_owned]);
    }
    if let Some(o) = &owner {
        owner_owned = o.clone();
        args.extend(&["--owner", &owner_owned]);
    }
    if let Some(d) = &due_at {
        due_owned = d.clone();
        args.extend(&["--due", &due_owned]);
    }
    if let Some(t) = &tags {
        tags_owned = t.join(",");
        args.extend(&["--tags", &tags_owned]);
    }

    run_granary(&workspace, &args).await
}

/// Update existing task
#[allow(clippy::too_many_arguments)]
pub async fn update_task(
    workspace: PathBuf,
    task_id: String,
    title: Option<String>,
    description: Option<String>,
    priority: Option<TaskPriority>,
    status: Option<TaskStatus>,
    owner: Option<String>,
    due_at: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<(), String> {
    let mut args = vec!["task".to_string(), task_id, "update".to_string()];

    if let Some(t) = title {
        args.extend(["--title".to_string(), t]);
    }
    if let Some(d) = description {
        args.extend(["--description".to_string(), d]);
    }
    if let Some(p) = priority {
        args.extend(["--priority".to_string(), p.as_str().to_string()]);
    }
    if let Some(s) = status {
        args.extend(["--status".to_string(), s.as_str().to_string()]);
    }
    if let Some(o) = owner {
        args.extend(["--owner".to_string(), o]);
    }
    if let Some(d) = due_at {
        args.extend(["--due".to_string(), d]);
    }
    if let Some(t) = tags {
        args.extend(["--tags".to_string(), t.join(",")]);
    }

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_granary(&workspace, &args_refs).await?;
    Ok(())
}

/// Block a task with reason
pub async fn block_task(workspace: PathBuf, task_id: String, reason: String) -> Result<(), String> {
    run_granary(
        &workspace,
        &["task", &task_id, "block", "--reason", &reason],
    )
    .await?;
    Ok(())
}

/// Load task dependencies
pub async fn load_dependencies(
    workspace: PathBuf,
    task_id: String,
) -> Result<Vec<TaskDependency>, String> {
    let output = run_granary(&workspace, &["task", &task_id, "deps", "--json"]).await?;
    parse_json_logged(&output, &format!("task {} deps --json", task_id))
}

/// Add dependency to task
pub async fn add_dependency(
    workspace: PathBuf,
    task_id: String,
    depends_on: String,
) -> Result<(), String> {
    run_granary(&workspace, &["task", &task_id, "deps", "add", &depends_on]).await?;
    Ok(())
}

/// Remove dependency from task
pub async fn remove_dependency(
    workspace: PathBuf,
    task_id: String,
    depends_on: String,
) -> Result<(), String> {
    run_granary(
        &workspace,
        &["task", &task_id, "deps", "remove", &depends_on],
    )
    .await?;
    Ok(())
}

/// Load single task by ID
pub async fn load_task(workspace: PathBuf, task_id: String) -> Result<GranaryTask, String> {
    let output = run_granary(&workspace, &["task", &task_id, "--json"]).await?;
    parse_json_logged(&output, &format!("task {} --json", task_id))
}

// =============================================================================
// Worker Management
// =============================================================================

/// Load all workers
///
/// # Arguments
/// - `workspace`: Path to the granary workspace
/// - `include_all`: Include stopped/terminated workers
pub async fn load_workers(workspace: PathBuf, include_all: bool) -> Result<Vec<Worker>, String> {
    let mut args = vec!["workers", "--json"];
    if include_all {
        args.push("--all");
    }
    let output = run_granary(&workspace, &args).await?;
    parse_json_logged(&output, "workers --json")
}

/// Start a worker using a configured runner
///
/// # Arguments
/// - `workspace`: Path to the granary workspace
/// - `runner_name`: Name of the configured runner to use
/// - `detached`: Run worker in detached/background mode
pub async fn start_worker_from_runner(
    workspace: PathBuf,
    runner_name: String,
    detached: bool,
) -> Result<Worker, String> {
    let mut args = vec!["worker", "start", "--runner", &runner_name, "--json"];
    if detached {
        args.push("-d");
    }
    let output = run_granary(&workspace, &args).await?;
    parse_json_logged(
        &output,
        &format!("worker start --runner {} --json", runner_name),
    )
}

/// Start a worker with inline command configuration
///
/// # Arguments
/// - `workspace`: Path to the granary workspace
/// - `command`: Command to run
/// - `args`: Arguments to pass to the command
/// - `event_type`: Event type to trigger on (e.g., "task.ready")
/// - `concurrency`: Number of concurrent tasks
/// - `detached`: Run worker in detached/background mode
pub async fn start_worker_inline(
    workspace: PathBuf,
    command: String,
    args: Vec<String>,
    event_type: String,
    concurrency: u32,
    detached: bool,
) -> Result<Worker, String> {
    let concurrency_str = concurrency.to_string();
    let mut cmd_args = vec![
        "worker",
        "start",
        "--command",
        &command,
        "--on",
        &event_type,
        "--concurrency",
        &concurrency_str,
        "--json",
    ];

    // Add command arguments
    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    for arg in &args_refs {
        cmd_args.push("-a");
        cmd_args.push(arg);
    }

    if detached {
        cmd_args.push("-d");
    }

    let output = run_granary(&workspace, &cmd_args).await?;
    parse_json_logged(
        &output,
        &format!("worker start --command {} --json", command),
    )
}

/// Stop a running worker
///
/// # Arguments
/// - `workspace`: Path to the granary workspace
/// - `worker_id`: ID of the worker to stop
pub async fn stop_worker(workspace: PathBuf, worker_id: String) -> Result<(), String> {
    run_granary(&workspace, &["worker", &worker_id, "stop"]).await?;
    Ok(())
}

/// Get the status of a single worker
///
/// # Arguments
/// - `workspace`: Path to the granary workspace
/// - `worker_id`: ID of the worker to get status for
pub async fn get_worker_status(workspace: PathBuf, worker_id: String) -> Result<Worker, String> {
    let output = run_granary(&workspace, &["worker", &worker_id, "status", "--json"]).await?;
    parse_json_logged(&output, &format!("worker {} status --json", worker_id))
}

// =============================================================================
// Initiative Management
// =============================================================================

/// Load all active initiatives
pub async fn load_initiatives(workspace: PathBuf) -> Result<Vec<Initiative>, String> {
    let output = run_granary(&workspace, &["initiatives", "--json"]).await?;
    parse_json_logged(&output, "initiatives --json")
}

/// Load initiative summary (includes projects, blockers, next actions)
pub async fn load_initiative_summary(
    workspace: PathBuf,
    initiative_id: String,
) -> Result<InitiativeSummary, String> {
    let output = run_granary(
        &workspace,
        &["initiative", &initiative_id, "summary", "--json"],
    )
    .await?;
    parse_json_logged(
        &output,
        &format!("initiative {} summary --json", initiative_id),
    )
}

/// Archive an initiative
pub async fn archive_initiative(workspace: PathBuf, initiative_id: String) -> Result<(), String> {
    run_granary(&workspace, &["initiative", &initiative_id, "archive"]).await?;
    Ok(())
}

// =============================================================================
// Comment Management
// =============================================================================

/// Load comments for a task
pub async fn load_comments(workspace: PathBuf, task_id: String) -> Result<Vec<Comment>, String> {
    let output = run_granary(&workspace, &["task", &task_id, "comments", "--json"]).await?;
    parse_json_logged(&output, &format!("task {} comments --json", task_id))
}

/// Add a comment to a task
pub async fn add_comment(
    workspace: PathBuf,
    task_id: String,
    content: String,
) -> Result<(), String> {
    run_granary(
        &workspace,
        &["task", &task_id, "comments", "create", &content],
    )
    .await?;
    Ok(())
}

/// Prune stopped/errored workers
///
/// Removes all workers that are stopped or in error state from the database.
pub async fn prune_workers(workspace: PathBuf) -> Result<(), String> {
    run_granary(&workspace, &["worker", "prune"]).await?;
    Ok(())
}
