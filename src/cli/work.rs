//! Work command - claims a task and provides full context for execution
//!
//! This implements the "Work" workflow from the LLM-first CLI redesign.
//! Agents use this command to claim a task, get complete context,
//! and then signal completion or blockers.

use granary_types::{Project, Task};
use serde::Serialize;

use crate::cli::args::{CliOutputFormat, WorkCommand};
use crate::db;
use crate::error::{GranaryError, Result};
use crate::output::json::SteeringInfo;
use crate::output::{Output, OutputType};
use crate::services::{self, Workspace};

/// Output for work start - full context for executing a task
pub struct WorkOutput {
    pub task: Task,
    pub project: Project,
    pub steering: Vec<SteeringInfo>,
}

impl Output for WorkOutput {
    fn output_type() -> OutputType {
        OutputType::Prompt // LLM-first command
    }

    fn to_json(&self) -> String {
        let json_output = WorkJsonOutput {
            task: &self.task,
            project: WorkProjectJson {
                id: &self.project.id,
                name: &self.project.name,
            },
            steering: &self.steering,
        };
        serde_json::to_string_pretty(&json_output).unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        format_work_context(&self.task, &self.project, &self.steering)
    }

    fn to_text(&self) -> String {
        format!("Working on: {} - {}", self.task.id, self.task.title)
    }
}

#[derive(Serialize)]
struct WorkJsonOutput<'a> {
    task: &'a Task,
    project: WorkProjectJson<'a>,
    steering: &'a [SteeringInfo],
}

#[derive(Serialize)]
struct WorkProjectJson<'a> {
    id: &'a str,
    name: &'a str,
}

/// Output for work done - simple status message
pub struct WorkDoneOutput;

impl Output for WorkDoneOutput {
    fn output_type() -> OutputType {
        OutputType::Text // Simple status
    }

    fn to_json(&self) -> String {
        r#"{"status": "done"}"#.to_string()
    }

    fn to_prompt(&self) -> String {
        "Done.".to_string()
    }

    fn to_text(&self) -> String {
        "Done.".to_string()
    }
}

/// Output for work block - simple status message
pub struct WorkBlockOutput;

impl Output for WorkBlockOutput {
    fn output_type() -> OutputType {
        OutputType::Text // Simple status
    }

    fn to_json(&self) -> String {
        r#"{"status": "blocked"}"#.to_string()
    }

    fn to_prompt(&self) -> String {
        "Blocked.".to_string()
    }

    fn to_text(&self) -> String {
        "Blocked.".to_string()
    }
}

/// Output for work release - simple status message
pub struct WorkReleaseOutput;

impl Output for WorkReleaseOutput {
    fn output_type() -> OutputType {
        OutputType::Text // Simple status
    }

    fn to_json(&self) -> String {
        r#"{"status": "released"}"#.to_string()
    }

    fn to_prompt(&self) -> String {
        "Released.".to_string()
    }

    fn to_text(&self) -> String {
        "Released.".to_string()
    }
}

/// Handle work commands
pub async fn work(command: WorkCommand, cli_format: Option<CliOutputFormat>) -> Result<()> {
    match command {
        WorkCommand::Start { task_id, owner } => {
            work_start(&task_id, owner, cli_format).await?;
        }
        WorkCommand::Done {
            task_id,
            summary_positional,
            summary_flag,
        } => {
            let summary = summary_positional.or(summary_flag).ok_or_else(|| {
                GranaryError::InvalidArgument(
                    "Summary is required. Usage: granary work done <task-id> <summary>".to_string(),
                )
            })?;
            work_done(&task_id, &summary, cli_format).await?;
        }
        WorkCommand::Block {
            task_id,
            reason_positional,
            reason_flag,
        } => {
            let reason = reason_positional.or(reason_flag).ok_or_else(|| {
                GranaryError::InvalidArgument(
                    "Reason is required. Usage: granary work block <task-id> <reason>".to_string(),
                )
            })?;
            work_block(&task_id, &reason, cli_format).await?;
        }
        WorkCommand::Release { task_id } => {
            work_release(&task_id, cli_format).await?;
        }
    }

    Ok(())
}

/// Start working on a task - claims it and outputs full context
async fn work_start(
    task_id: &str,
    owner: Option<String>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // 1. Get the task (fail fast if not found)
    let task = services::get_task(&pool, task_id).await.inspect_err(|_e| {
        eprintln!("Task not found. Exiting.");
    })?;

    // 2. Check if task is blocked by dependencies
    let unmet_deps = db::dependencies::get_unmet(&pool, task_id).await?;
    if !unmet_deps.is_empty() {
        eprintln!("Task blocked by dependencies. Exiting.");
        return Err(GranaryError::UnmetDependencies(
            unmet_deps
                .iter()
                .map(|t| t.id.clone())
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }

    // 3. Check if task is already claimed by someone else
    if task.is_claimed()
        && let Some(claim) = task.claim_info()
        && owner.as_deref() != Some(&claim.owner)
    {
        eprintln!("Task claimed by another worker. Exiting.");
        return Err(GranaryError::ClaimConflict {
            owner: claim.owner.clone(),
            expires_at: claim.lease_expires_at.clone().unwrap_or_default(),
        });
    }

    // 4. Check if task is in blocked status
    if task.status == "blocked" {
        eprintln!("Task is blocked. Exiting.");
        return Err(GranaryError::TaskBlocked(
            task.blocked_reason
                .clone()
                .unwrap_or_else(|| "No reason provided".to_string()),
        ));
    }

    // 5. Check if task is in draft status
    if task.status == "draft" {
        return Err(GranaryError::Conflict(format!(
            "Task {} is in draft status",
            task_id
        )));
    }

    // 6. Start the task (claims it)
    let owner_name = owner.clone().unwrap_or_else(|| "agent".to_string());
    services::start_task(&pool, task_id, Some(owner_name.clone())).await?;
    services::claim_task(&pool, task_id, &owner_name, Some(30)).await?;

    // 7. Get the project
    let project = services::get_project(&pool, &task.project_id).await?;

    // 8. Fetch steering files for this task
    let steering = fetch_steering_for_work(&pool, &workspace, task_id, &task.project_id).await?;

    // 9. Output the context
    let output = WorkOutput {
        task,
        project,
        steering,
    };
    println!("{}", output.format(cli_format));

    Ok(())
}

/// Mark task as done
async fn work_done(
    task_id: &str,
    summary: &str,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Get the task first (fail fast if not found)
    services::get_task(&pool, task_id).await.inspect_err(|_e| {
        eprintln!("Task not found. Exiting.");
    })?;

    // Complete the task with a comment
    services::complete_task(&pool, task_id, Some(summary)).await?;

    let output = WorkDoneOutput;
    println!("{}", output.format(cli_format));
    Ok(())
}

/// Block task with reason
async fn work_block(
    task_id: &str,
    reason: &str,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Get the task first (fail fast if not found)
    services::get_task(&pool, task_id).await.inspect_err(|_e| {
        eprintln!("Task not found. Exiting.");
    })?;

    // Block the task
    services::block_task(&pool, task_id, reason).await?;

    let output = WorkBlockOutput;
    println!("{}", output.format(cli_format));
    Ok(())
}

/// Release task (give up claim)
async fn work_release(task_id: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Get the task first (fail fast if not found)
    services::get_task(&pool, task_id).await.inspect_err(|_e| {
        eprintln!("Task not found. Exiting.");
    })?;

    // Release the task claim
    services::release_task(&pool, task_id).await?;

    let output = WorkReleaseOutput;
    println!("{}", output.format(cli_format));
    Ok(())
}

/// Fetch steering files for work context
/// Includes: global, task-attached, project-attached
async fn fetch_steering_for_work(
    pool: &sqlx::SqlitePool,
    workspace: &Workspace,
    task_id: &str,
    project_id: &str,
) -> Result<Vec<SteeringInfo>> {
    let mut result = Vec::new();

    // 1. Get global (unscoped) steering files
    let global_files = db::steering::list_global(pool).await?;
    for file in global_files {
        if let Some(info) = resolve_steering_file(workspace, file, "global") {
            result.push(info);
        }
    }

    // 2. Get project-attached steering files
    let project_files = db::steering::list_by_scope(pool, "project", project_id).await?;
    for file in project_files {
        if let Some(info) = resolve_steering_file(workspace, file, "project") {
            result.push(info);
        }
    }

    // 3. Get task-attached steering files
    let task_files = db::steering::list_by_scope(pool, "task", task_id).await?;
    for file in task_files {
        if let Some(info) = resolve_steering_file(workspace, file, "task") {
            result.push(info);
        }
    }

    Ok(result)
}

/// Resolve a steering file path and read its contents
fn resolve_steering_file(
    workspace: &Workspace,
    file: db::steering::SteeringFile,
    scope: &str,
) -> Option<SteeringInfo> {
    // Resolve file path
    let resolved_path = if file.path.starts_with('/') || file.path.starts_with("http") {
        file.path.clone()
    } else {
        // Resolve relative paths against workspace root
        workspace.root.join(&file.path).display().to_string()
    };

    // Try to read file content (silently skip missing files)
    let content = if !resolved_path.starts_with("http") {
        std::fs::read_to_string(&resolved_path).ok()
    } else {
        None
    };

    Some(SteeringInfo {
        path: file.path,
        mode: file.mode,
        content,
        scope: Some(scope.to_string()),
    })
}

/// Format work context in markdown format
fn format_work_context(task: &Task, project: &Project, steering: &[SteeringInfo]) -> String {
    let mut output = String::new();

    // Header with task ID and title
    output.push_str(&format!("## {}: {}\n\n", task.id, task.title));

    // Metadata
    output.push_str(&format!("Project: {}\n", project.id));
    output.push_str(&format!("Priority: {}\n\n", task.priority));

    // Goal/Description
    if let Some(ref description) = task.description {
        // Try to extract Goal if it's structured in the description
        if description.contains("**Goal:**") {
            // The description has structured content, output it as-is
            output.push_str(description);
        } else {
            output.push_str(&format!("**Goal:** {}", description));
        }
        output.push_str("\n\n");
    }

    // Steering files
    if !steering.is_empty() {
        output.push_str("## Steering\n\n");

        for sf in steering {
            // Include scope indicator in the tag
            let scope_attr = sf
                .scope
                .as_ref()
                .map(|s| format!(" scope=\"{}\"", s))
                .unwrap_or_default();

            if let Some(ref content) = sf.content {
                output.push_str(&format!(
                    "<steering_file path=\"{}\"{}>\n",
                    sf.path, scope_attr
                ));
                output.push_str(content);
                output.push_str("\n</steering_file>\n\n");
            } else {
                output.push_str(&format!(
                    "<steering_file path=\"{}\"{}>\n",
                    sf.path, scope_attr
                ));
                output.push_str("(reference to external document)\n");
                output.push_str("</steering_file>\n\n");
            }
        }
    }

    // Instructions for completion
    output.push_str("CRITICAL:\n- when done\n");
    output.push_str("```bash\n");
    output.push_str(&format!(
        "granary work done {} \"summary of changes\"\n",
        task.id
    ));
    output.push_str("```\n\n");

    output.push_str("- if blocked\n");
    output.push_str("```bash\n");
    output.push_str(&format!(
        "granary work block {} \"reason for blocking\"\n",
        task.id
    ));
    output.push_str("```");

    output
}
