use granary_types::Task;
use sqlx::SqlitePool;

use crate::db;
use crate::error::Result;
use crate::models::*;
use crate::output::json::{
    BlockerInfo, ContextOutput, HandoffOutput, PriorityCounts, SessionSummary, StateSummary,
    StatusCounts, SteeringInfo, SummaryOutput,
};
use crate::services::{Workspace, get_current_session, get_scope_by_type, get_task};

/// Generate a summary for the current session or workspace
pub async fn generate_summary(
    pool: &SqlitePool,
    workspace: &Workspace,
    token_budget: Option<usize>,
) -> Result<SummaryOutput> {
    let current_session = get_current_session(pool, workspace).await?;

    // Get tasks based on session scope or all tasks
    let tasks = if let Some(ref session) = current_session {
        let project_ids = get_scope_by_type(pool, &session.id, ScopeItemType::Project).await?;
        if project_ids.is_empty() {
            db::tasks::list_all(pool).await?
        } else {
            let mut all_tasks = Vec::new();
            for project_id in project_ids {
                let project_tasks = db::tasks::list_by_project(pool, &project_id).await?;
                all_tasks.extend(project_tasks);
            }
            all_tasks
        }
    } else {
        db::tasks::list_all(pool).await?
    };

    // Calculate state summary
    let mut by_status = StatusCounts::default();
    let mut by_priority = PriorityCounts::default();

    for task in &tasks {
        match task.status.as_str() {
            "todo" => by_status.todo += 1,
            "in_progress" => by_status.in_progress += 1,
            "done" => by_status.done += 1,
            "blocked" => by_status.blocked += 1,
            _ => {}
        }
        match task.priority.as_str() {
            "P0" => by_priority.p0 += 1,
            "P1" => by_priority.p1 += 1,
            "P2" => by_priority.p2 += 1,
            "P3" => by_priority.p3 += 1,
            "P4" => by_priority.p4 += 1,
            _ => {}
        }
    }

    let state = StateSummary {
        total_tasks: tasks.len(),
        by_status,
        by_priority,
    };

    // Get focus task
    let focus_task = if let Some(ref session) = current_session {
        if let Some(ref focus_id) = session.focus_task_id {
            get_task(pool, focus_id).await.ok()
        } else {
            None
        }
    } else {
        None
    };

    // Get blockers
    let blockers: Vec<Task> = tasks
        .iter()
        .filter(|t| t.blocked_reason.is_some() || t.status == "blocked")
        .cloned()
        .collect();

    // Get next actionable tasks (limit based on token budget)
    let max_actions = token_budget.map(|b| b / 100).unwrap_or(5).max(3);
    let next_actions: Vec<Task> = tasks
        .iter()
        .filter(|t| (t.status == "todo" || t.status == "in_progress") && t.blocked_reason.is_none())
        .take(max_actions)
        .cloned()
        .collect();

    // Get recent decisions
    let recent_decisions = db::comments::list_by_kind(pool, "decision").await?;
    let recent_decisions: Vec<Comment> = recent_decisions.into_iter().take(5).collect();

    // Get recent artifacts (across all tasks in scope)
    let mut recent_artifacts = Vec::new();
    for task in tasks.iter().take(10) {
        let artifacts = db::artifacts::list_by_parent(pool, &task.id).await?;
        recent_artifacts.extend(artifacts);
    }
    recent_artifacts.truncate(5);

    let session_summary = current_session.map(|s| SessionSummary {
        id: s.id,
        name: s.name,
        mode: s.mode,
        owner: s.owner,
        focus_task_id: s.focus_task_id,
    });

    Ok(SummaryOutput {
        session: session_summary,
        state,
        focus_task,
        blockers,
        next_actions,
        recent_decisions,
        recent_artifacts,
    })
}

/// Generate a context pack for LLM consumption
pub async fn generate_context(
    pool: &SqlitePool,
    workspace: &Workspace,
    include: Option<Vec<String>>,
    max_items: Option<usize>,
) -> Result<ContextOutput> {
    let current_session = get_current_session(pool, workspace).await?;
    let max = max_items.unwrap_or(50);

    // Determine what to include
    let include_set: std::collections::HashSet<&str> = include
        .as_ref()
        .map(|v| v.iter().map(|s| s.as_str()).collect())
        .unwrap_or_else(|| {
            [
                "projects",
                "tasks",
                "comments",
                "decisions",
                "blockers",
                "artifacts",
            ]
            .into_iter()
            .collect()
        });

    // Get projects
    let projects = if include_set.contains("projects") {
        if let Some(ref session) = current_session {
            let project_ids = get_scope_by_type(pool, &session.id, ScopeItemType::Project).await?;
            let mut projects = Vec::new();
            for id in project_ids.iter().take(max) {
                if let Ok(p) = crate::services::get_project(pool, id).await {
                    projects.push(p);
                }
            }
            projects
        } else {
            db::projects::list(pool, false)
                .await?
                .into_iter()
                .take(max)
                .collect()
        }
    } else {
        Vec::new()
    };

    // Get tasks
    let tasks = if include_set.contains("tasks") {
        if let Some(ref session) = current_session {
            let project_ids = get_scope_by_type(pool, &session.id, ScopeItemType::Project).await?;
            let task_ids = get_scope_by_type(pool, &session.id, ScopeItemType::Task).await?;

            let mut all_tasks = Vec::new();

            // Add explicitly pinned tasks
            for id in task_ids.iter().take(max) {
                if let Ok(t) = get_task(pool, id).await {
                    all_tasks.push(t);
                }
            }

            // Add tasks from pinned projects
            for project_id in project_ids {
                let project_tasks = db::tasks::list_by_project(pool, &project_id).await?;
                for task in project_tasks {
                    if !all_tasks.iter().any(|t| t.id == task.id) {
                        all_tasks.push(task);
                    }
                    if all_tasks.len() >= max {
                        break;
                    }
                }
            }

            all_tasks.truncate(max);
            all_tasks
        } else {
            db::tasks::list_all(pool)
                .await?
                .into_iter()
                .take(max)
                .collect()
        }
    } else {
        Vec::new()
    };

    // Get comments
    let comments = if include_set.contains("comments") {
        let mut all_comments = Vec::new();
        for task in tasks.iter().take(10) {
            let task_comments = db::comments::list_by_parent(pool, &task.id).await?;
            all_comments.extend(task_comments);
        }
        all_comments.truncate(max);
        all_comments
    } else {
        Vec::new()
    };

    // Get decisions
    let decisions = if include_set.contains("decisions") {
        db::comments::list_by_kind(pool, "decision")
            .await?
            .into_iter()
            .take(max)
            .collect()
    } else {
        Vec::new()
    };

    // Get blockers
    let blockers: Vec<BlockerInfo> = if include_set.contains("blockers") {
        let mut blocker_info = Vec::new();
        for task in &tasks {
            if task.blocked_reason.is_some() || task.status == "blocked" {
                let unmet_deps = db::dependencies::get_unmet(pool, &task.id).await?;
                blocker_info.push(BlockerInfo {
                    task_id: task.id.clone(),
                    task_title: task.title.clone(),
                    reason: task.blocked_reason.clone(),
                    unmet_deps: unmet_deps.into_iter().map(|t| t.id).collect(),
                });
            }
        }
        blocker_info.truncate(max);
        blocker_info
    } else {
        Vec::new()
    };

    // Get artifacts
    let artifacts = if include_set.contains("artifacts") {
        let mut all_artifacts = Vec::new();
        for task in tasks.iter().take(10) {
            let task_artifacts = db::artifacts::list_by_parent(pool, &task.id).await?;
            all_artifacts.extend(task_artifacts);
        }
        all_artifacts.truncate(max);
        all_artifacts
    } else {
        Vec::new()
    };

    // Get steering files based on scope:
    // - Global steering (always included)
    // - Project-attached steering (for projects in session scope)
    // - Session-attached steering (for current session)
    let project_ids_for_steering = if let Some(ref session) = current_session {
        get_scope_by_type(pool, &session.id, ScopeItemType::Project).await?
    } else {
        Vec::new()
    };
    let session_id_for_steering = current_session.as_ref().map(|s| s.id.clone());

    let steering = fetch_steering_for_context(
        pool,
        workspace,
        &project_ids_for_steering,
        session_id_for_steering.as_deref(),
    )
    .await?;

    let session_summary = current_session.map(|s| SessionSummary {
        id: s.id,
        name: s.name,
        mode: s.mode,
        owner: s.owner,
        focus_task_id: s.focus_task_id,
    });

    Ok(ContextOutput {
        session: session_summary,
        projects,
        tasks,
        comments,
        decisions,
        blockers,
        artifacts,
        steering,
    })
}

/// Generate a handoff document for agent delegation
pub async fn generate_handoff(
    pool: &SqlitePool,
    to: &str,
    task_ids: &[String],
    constraints: Option<&str>,
    acceptance_criteria: Option<&str>,
    output_schema: Option<serde_json::Value>,
) -> Result<HandoffOutput> {
    let workspace = Workspace::find()?;
    let mut tasks = Vec::new();
    let mut context = Vec::new();

    for id in task_ids {
        let task = get_task(pool, id).await?;

        // Get task comments for context
        let comments = db::comments::list_by_parent(pool, id).await?;
        context.extend(comments);

        tasks.push(task);
    }

    // Sort context by created_at
    context.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    context.truncate(20);

    // Get steering files for handoff:
    // - Global steering (always included)
    // - Task-attached steering (for the specific tasks being handed off)
    let steering = fetch_steering_for_handoff(pool, &workspace, task_ids).await?;

    Ok(HandoffOutput {
        to: to.to_string(),
        tasks,
        context,
        constraints: constraints.map(|s| s.to_string()),
        acceptance_criteria: acceptance_criteria.map(|s| s.to_string()),
        output_schema,
        steering,
    })
}

/// Fetch steering files for context generation
/// Includes: global, project-attached (for projects in scope), session-attached (for current session)
async fn fetch_steering_for_context(
    pool: &SqlitePool,
    workspace: &Workspace,
    project_ids: &[String],
    session_id: Option<&str>,
) -> Result<Vec<SteeringInfo>> {
    let mut all_files = Vec::new();

    // 1. Get global (unscoped) steering files
    let global_files = db::steering::list_global(pool).await?;
    all_files.extend(global_files);

    // 2. Get project-attached steering files
    if !project_ids.is_empty() {
        let project_files = db::steering::list_by_scope_ids(pool, "project", project_ids).await?;
        all_files.extend(project_files);
    }

    // 3. Get session-attached steering files
    if let Some(sid) = session_id {
        let session_files = db::steering::list_by_scope(pool, "session", sid).await?;
        all_files.extend(session_files);
    }

    // Convert to SteeringInfo with content
    convert_to_steering_info(workspace, all_files)
}

/// Fetch steering files for handoff generation
/// Includes: global, task-attached (for the specific tasks being handed off)
async fn fetch_steering_for_handoff(
    pool: &SqlitePool,
    workspace: &Workspace,
    task_ids: &[String],
) -> Result<Vec<SteeringInfo>> {
    let mut all_files = Vec::new();

    // 1. Get global (unscoped) steering files
    let global_files = db::steering::list_global(pool).await?;
    all_files.extend(global_files);

    // 2. Get task-attached steering files
    if !task_ids.is_empty() {
        let task_files = db::steering::list_by_scope_ids(pool, "task", task_ids).await?;
        all_files.extend(task_files);
    }

    // Convert to SteeringInfo with content
    convert_to_steering_info(workspace, all_files)
}

/// Convert SteeringFile records to SteeringInfo with file contents
fn convert_to_steering_info(
    workspace: &Workspace,
    files: Vec<db::steering::SteeringFile>,
) -> Result<Vec<SteeringInfo>> {
    let mut result = Vec::new();

    for file in files {
        // Resolve file path
        let resolved_path = if file.path.starts_with('/') || file.path.starts_with("http") {
            file.path.clone()
        } else {
            // Resolve relative paths against workspace root
            workspace.root.join(&file.path).display().to_string()
        };

        // Try to read file content
        let content = if !resolved_path.starts_with("http") {
            std::fs::read_to_string(&resolved_path).ok()
        } else {
            None
        };

        // Determine scope from scope_type
        let scope = match file.scope_type.as_deref() {
            None => Some("global".to_string()),
            Some(st) => Some(st.to_string()),
        };

        result.push(SteeringInfo {
            path: file.path,
            mode: file.mode,
            content,
            scope,
        });
    }

    Ok(result)
}
