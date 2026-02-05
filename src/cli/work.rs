//! Work command - claims a task and provides full context for execution
//!
//! This implements the "Work" workflow from the LLM-first CLI redesign.
//! Agents use this command to claim a task, get complete context,
//! and then signal completion or blockers.

use granary_types::{Project, Task};

use crate::cli::args::WorkCommand;
use crate::db;
use crate::error::{GranaryError, Result};
use crate::output::json::SteeringInfo;
use crate::services::{self, Workspace};

/// Handle work commands
pub async fn work(command: WorkCommand) -> Result<()> {
    match command {
        WorkCommand::Start { task_id, owner } => {
            work_start(&task_id, owner).await?;
        }
        WorkCommand::Done { task_id, summary } => {
            work_done(&task_id, &summary).await?;
        }
        WorkCommand::Block { task_id, reason } => {
            work_block(&task_id, &reason).await?;
        }
        WorkCommand::Release { task_id } => {
            work_release(&task_id).await?;
        }
    }

    Ok(())
}

/// Start working on a task - claims it and outputs full context
async fn work_start(task_id: &str, owner: Option<String>) -> Result<()> {
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
        eprintln!(
            "Task is in draft status. Use 'granary task {} ready' first. Exiting.",
            task_id
        );
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

    // 9. Output the context in markdown format
    output_work_context(&task, &project, &steering);

    Ok(())
}

/// Mark task as done
async fn work_done(task_id: &str, summary: &str) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Get the task first (fail fast if not found)
    services::get_task(&pool, task_id).await.inspect_err(|_e| {
        eprintln!("Task not found. Exiting.");
    })?;

    // Complete the task with a comment
    services::complete_task(&pool, task_id, Some(summary)).await?;

    println!("Done.");
    Ok(())
}

/// Block task with reason
async fn work_block(task_id: &str, reason: &str) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Get the task first (fail fast if not found)
    services::get_task(&pool, task_id).await.inspect_err(|_e| {
        eprintln!("Task not found. Exiting.");
    })?;

    // Block the task
    services::block_task(&pool, task_id, reason).await?;

    println!("Blocked.");
    Ok(())
}

/// Release task (give up claim)
async fn work_release(task_id: &str) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Get the task first (fail fast if not found)
    services::get_task(&pool, task_id).await.inspect_err(|_e| {
        eprintln!("Task not found. Exiting.");
    })?;

    // Release the task claim
    services::release_task(&pool, task_id).await?;

    println!("Released.");
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

/// Output work context in markdown format
fn output_work_context(task: &Task, project: &Project, steering: &[SteeringInfo]) {
    // Header with task ID and title
    println!("## {}: {}", task.id, task.title);
    println!();

    // Metadata
    println!("Project: {}", project.id);
    println!("Priority: {}", task.priority);
    println!();

    // Goal/Description
    if let Some(ref description) = task.description {
        // Try to extract Goal if it's structured in the description
        if description.contains("**Goal:**") {
            // The description has structured content, output it as-is
            println!("{}", description);
        } else {
            println!("**Goal:** {}", description);
        }
        println!();
    }

    // Extract files to modify from description if present
    // (This is a heuristic - if the description contains file paths)
    if let Some(ref description) = task.description {
        if description.contains("**Files to") {
            // Files are already in the description, don't duplicate
        } else {
            // Try to extract file paths from description
            let files: Vec<&str> = description
                .lines()
                .filter(|line| {
                    line.contains(".rs")
                        || line.contains(".ts")
                        || line.contains(".js")
                        || line.contains(".py")
                        || line.contains("src/")
                        || line.contains("lib/")
                })
                .collect();

            if !files.is_empty() {
                println!("**Files to modify:**");
                for file in files {
                    println!("- {}", file.trim_start_matches("- ").trim());
                }
                println!();
            }
        }
    }

    // Steering files
    if !steering.is_empty() {
        println!("## Steering");
        println!();

        for sf in steering {
            // Include scope indicator in the tag
            let scope_attr = sf
                .scope
                .as_ref()
                .map(|s| format!(" scope=\"{}\"", s))
                .unwrap_or_default();

            if let Some(ref content) = sf.content {
                println!("<steering_file path=\"{}\"{}>", sf.path, scope_attr);
                println!("{}", content);
                println!("</steering_file>");
                println!();
            } else {
                println!("<steering_file path=\"{}\"{}>", sf.path, scope_attr);
                println!("(reference to external document)");
                println!("</steering_file>");
                println!();
            }
        }
    }

    // Instructions for completion
    println!("## When Done");
    println!("```bash");
    println!("granary work done {} \"summary of changes\"", task.id);
    println!("```");
    println!();

    println!("## If Blocked");
    println!("```bash");
    println!("granary work block {} \"reason for blocking\"", task.id);
    println!("```");
}
