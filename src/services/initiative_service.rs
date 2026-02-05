//! Service layer for initiative business logic.
//!
//! Services provide the business logic layer between CLI and database.
//! All CRUD operations delegate to the db layer, with additional validation
//! and error handling.

use granary_types::Project;
use sqlx::SqlitePool;

use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::{
    CreateInitiative, Initiative, InitiativeBlockerInfo, InitiativeInfo, InitiativeStatusSummary,
    InitiativeSummary, NextAction, ProjectSummary, UpdateInitiative,
};
use crate::services;

/// Create a new initiative
pub async fn create_initiative(pool: &SqlitePool, input: CreateInitiative) -> Result<Initiative> {
    db::initiatives::create(pool, &input).await
}

/// Get an initiative by ID, returning None if not found
pub async fn get_initiative(pool: &SqlitePool, id: &str) -> Result<Option<Initiative>> {
    db::initiatives::get(pool, id).await
}

/// Get an initiative by ID, returning an error if not found
pub async fn get_initiative_or_error(pool: &SqlitePool, id: &str) -> Result<Initiative> {
    db::initiatives::get(pool, id)
        .await?
        .ok_or_else(|| GranaryError::InitiativeNotFound(id.to_string()))
}

/// List all initiatives
pub async fn list_initiatives(
    pool: &SqlitePool,
    include_archived: bool,
) -> Result<Vec<Initiative>> {
    db::initiatives::list(pool, include_archived).await
}

/// Update an initiative
///
/// Fetches the current version for optimistic locking before updating.
pub async fn update_initiative(
    pool: &SqlitePool,
    id: &str,
    updates: UpdateInitiative,
) -> Result<Initiative> {
    // Get current initiative to obtain version for optimistic locking
    let initiative = get_initiative_or_error(pool, id).await?;

    // Perform update with optimistic locking
    db::initiatives::update(pool, id, &updates, initiative.version)
        .await?
        .ok_or_else(|| GranaryError::InitiativeNotFound(id.to_string()))
}

/// Archive an initiative
pub async fn archive_initiative(pool: &SqlitePool, id: &str) -> Result<Initiative> {
    // Verify initiative exists
    let initiative = get_initiative_or_error(pool, id).await?;

    // Check if already archived
    if initiative.status == "archived" {
        return Err(GranaryError::Conflict(format!(
            "Initiative {} is already archived",
            id
        )));
    }

    db::initiatives::archive(pool, id).await?;

    // Refetch to return updated initiative
    get_initiative_or_error(pool, id).await
}

/// Delete an initiative (hard delete)
pub async fn delete_initiative(pool: &SqlitePool, id: &str) -> Result<bool> {
    // Verify initiative exists first
    let _ = get_initiative_or_error(pool, id).await?;

    db::initiatives::delete(pool, id).await
}

// === Initiative-Project relationship operations ===

/// Add a project to an initiative
///
/// Verifies both the initiative and project exist before adding.
/// This operation is idempotent - adding the same project twice does nothing.
pub async fn add_project_to_initiative(
    pool: &SqlitePool,
    initiative_id: &str,
    project_id: &str,
) -> Result<()> {
    // Verify both exist
    let _ = get_initiative_or_error(pool, initiative_id).await?;
    let _ = services::get_project(pool, project_id).await?;

    db::initiative_projects::add(pool, initiative_id, project_id).await
}

/// Remove a project from an initiative
pub async fn remove_project_from_initiative(
    pool: &SqlitePool,
    initiative_id: &str,
    project_id: &str,
) -> Result<bool> {
    db::initiative_projects::remove(pool, initiative_id, project_id).await
}

/// Get all projects in an initiative
pub async fn get_initiative_projects(
    pool: &SqlitePool,
    initiative_id: &str,
) -> Result<Vec<Project>> {
    // Verify initiative exists
    let _ = get_initiative_or_error(pool, initiative_id).await?;

    db::initiative_projects::list_projects(pool, initiative_id).await
}

/// Get all initiatives that contain a project
pub async fn get_project_initiatives(
    pool: &SqlitePool,
    project_id: &str,
) -> Result<Vec<Initiative>> {
    // Verify project exists
    let _ = services::get_project(pool, project_id).await?;

    db::initiative_projects::list_initiatives(pool, project_id).await
}

// === Next task operations ===

/// Get the next actionable task(s) across an initiative.
///
/// This respects both project-to-project dependencies and task-to-task dependencies.
/// A task is actionable only if:
/// 1. Its project has no unmet project dependencies (all dependency projects have all tasks done)
/// 2. The task itself has no unmet task dependencies (all dependency tasks are done)
/// 3. The task is not blocked (status != blocked, no blocked_reason)
/// 4. The task is todo or in_progress
///
/// If `all` is false, returns only the single next task (or empty if none).
/// If `all` is true, returns all actionable tasks sorted by priority, due_at, created_at, id.
pub async fn get_next_tasks(
    pool: &SqlitePool,
    initiative_id: &str,
    all: bool,
) -> Result<Vec<granary_types::Task>> {
    // Verify initiative exists
    let _ = get_initiative_or_error(pool, initiative_id).await?;

    db::initiative_tasks::get_next(pool, initiative_id, all).await
}

// === Initiative Summary ===

/// Generate a high-level summary of an initiative.
///
/// This aggregates statistics across all projects in the initiative:
/// - Overall progress percentage
/// - Project completion and blocking status
/// - Blockers with context
/// - Next actionable tasks
///
/// The summary is optimized for low token usage in orchestration scenarios.
pub async fn generate_initiative_summary(
    pool: &SqlitePool,
    initiative_id: &str,
    max_next_actions: usize,
) -> Result<InitiativeSummary> {
    let initiative = get_initiative_or_error(pool, initiative_id).await?;
    let projects = get_initiative_projects(pool, initiative_id).await?;

    let mut total_tasks = 0;
    let mut tasks_done = 0;
    let mut tasks_in_progress = 0;
    let mut tasks_blocked = 0;
    let mut tasks_todo = 0;
    let mut project_summaries = Vec::new();
    let mut blockers = Vec::new();

    for proj in &projects {
        let tasks = db::tasks::list_by_project(pool, &proj.id).await?;
        let proj_total = tasks.len();
        let proj_done = tasks.iter().filter(|t| t.status == "done").count();
        let proj_blocked_tasks: Vec<_> = tasks
            .iter()
            .filter(|t| t.status == "blocked" || t.blocked_reason.is_some())
            .collect();

        total_tasks += proj_total;
        tasks_done += proj_done;
        tasks_in_progress += tasks.iter().filter(|t| t.status == "in_progress").count();
        tasks_blocked += proj_blocked_tasks.len();
        tasks_todo += tasks.iter().filter(|t| t.status == "todo").count();

        // Check project dependencies
        let unmet_deps = db::project_dependencies::get_unmet(pool, &proj.id).await?;
        let blocked_by: Vec<String> = unmet_deps.iter().map(|p| p.id.clone()).collect();

        project_summaries.push(ProjectSummary {
            id: proj.id.clone(),
            name: proj.name.clone(),
            task_count: proj_total,
            done_count: proj_done,
            blocked: !blocked_by.is_empty() || !proj_blocked_tasks.is_empty(),
            blocked_by: blocked_by.clone(),
        });

        // Collect blockers - project dependencies
        for dep_id in &blocked_by {
            if let Some(dep_proj) = projects.iter().find(|p| &p.id == dep_id) {
                blockers.push(InitiativeBlockerInfo {
                    project_id: proj.id.clone(),
                    project_name: proj.name.clone(),
                    blocker_type: "project_dependency".to_string(),
                    description: format!("Waiting for {} to complete", dep_proj.name),
                });
            }
        }

        // Collect blockers - blocked tasks
        for blocked_task in proj_blocked_tasks {
            blockers.push(InitiativeBlockerInfo {
                project_id: proj.id.clone(),
                project_name: proj.name.clone(),
                blocker_type: "task_blocked".to_string(),
                description: blocked_task
                    .blocked_reason
                    .clone()
                    .unwrap_or_else(|| format!("Task '{}' is blocked", blocked_task.title)),
            });
        }
    }

    // Get next actions
    let next_tasks = get_next_tasks(pool, initiative_id, true).await?;
    let next_actions: Vec<NextAction> = next_tasks
        .into_iter()
        .take(max_next_actions)
        .map(|t| {
            let proj_name = project_summaries
                .iter()
                .find(|p| p.id == t.project_id)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            NextAction {
                task_id: t.id,
                task_title: t.title,
                project_id: t.project_id,
                project_name: proj_name,
                priority: t.priority,
            }
        })
        .collect();

    let completed_projects = project_summaries
        .iter()
        .filter(|p| p.done_count == p.task_count && p.task_count > 0)
        .count();
    let blocked_projects = project_summaries.iter().filter(|p| p.blocked).count();

    let percent_complete = if total_tasks > 0 {
        (tasks_done as f32 / total_tasks as f32) * 100.0
    } else {
        0.0
    };

    Ok(InitiativeSummary {
        initiative: InitiativeInfo {
            id: initiative.id,
            name: initiative.name,
            description: initiative.description,
        },
        status: InitiativeStatusSummary {
            total_projects: projects.len(),
            completed_projects,
            blocked_projects,
            total_tasks,
            tasks_done,
            tasks_in_progress,
            tasks_blocked,
            tasks_todo,
            percent_complete,
        },
        projects: project_summaries,
        blockers,
        next_actions,
    })
}
