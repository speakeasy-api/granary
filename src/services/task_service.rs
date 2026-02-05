use granary_types::{CreateTask, Task, TaskStatus, UpdateTask};
use sqlx::SqlitePool;

use crate::db::{self, counters};
use crate::error::{GranaryError, Result};
use crate::models::*;

/// Create a new task in a project
pub async fn create_task(pool: &SqlitePool, input: CreateTask) -> Result<Task> {
    // Verify project exists
    let _project = crate::services::get_project(pool, &input.project_id).await?;

    // Get next task number
    let scope = format!("project:{}:task", input.project_id);
    let task_number = counters::next(pool, &scope).await?;

    let id = generate_task_id(&input.project_id, task_number);
    let now = chrono::Utc::now().to_rfc3339();

    let tags = if input.tags.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&input.tags)?)
    };

    let task = Task {
        id: id.clone(),
        project_id: input.project_id,
        task_number,
        parent_task_id: input.parent_task_id,
        title: input.title,
        description: input.description,
        status: input.status.as_str().to_string(),
        priority: input.priority.as_str().to_string(),
        owner: input.owner,
        tags,
        blocked_reason: None,
        started_at: None,
        completed_at: None,
        due_at: input.due_at,
        claim_owner: None,
        claim_claimed_at: None,
        claim_lease_expires_at: None,
        pinned: 0,
        focus_weight: 0,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };

    db::tasks::create(pool, &task).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::TaskCreated,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({
                "title": task.title,
                "project_id": task.project_id,
            }),
        },
    )
    .await?;

    Ok(task)
}

/// Get a task by ID
pub async fn get_task(pool: &SqlitePool, id: &str) -> Result<Task> {
    db::tasks::get(pool, id)
        .await?
        .ok_or_else(|| GranaryError::TaskNotFound(id.to_string()))
}

/// List tasks in a project
pub async fn list_tasks_by_project(pool: &SqlitePool, project_id: &str) -> Result<Vec<Task>> {
    db::tasks::list_by_project(pool, project_id).await
}

/// List all tasks
pub async fn list_all_tasks(pool: &SqlitePool) -> Result<Vec<Task>> {
    db::tasks::list_all(pool).await
}

/// List tasks with filters
pub async fn list_tasks_filtered(
    pool: &SqlitePool,
    status: Option<&str>,
    priority: Option<&str>,
    owner: Option<&str>,
) -> Result<Vec<Task>> {
    db::tasks::list_filtered(pool, status, priority, owner).await
}

/// List subtasks of a task
pub async fn list_subtasks(pool: &SqlitePool, parent_task_id: &str) -> Result<Vec<Task>> {
    db::tasks::list_subtasks(pool, parent_task_id).await
}

/// Update a task
pub async fn update_task(pool: &SqlitePool, id: &str, updates: UpdateTask) -> Result<Task> {
    let mut task = get_task(pool, id).await?;
    let old_status = task.status.clone();

    if let Some(title) = updates.title {
        task.title = title;
    }
    if let Some(description) = updates.description {
        task.description = Some(description);
    }
    if let Some(status) = &updates.status {
        task.status = status.as_str().to_string();
    }
    if let Some(priority) = updates.priority {
        task.priority = priority.as_str().to_string();
    }
    if let Some(owner) = updates.owner {
        task.owner = Some(owner);
    }
    if let Some(tags) = updates.tags {
        task.tags = Some(serde_json::to_string(&tags)?);
    }
    if let Some(reason) = updates.blocked_reason {
        task.blocked_reason = Some(reason);
    }
    if let Some(due) = updates.due_at {
        task.due_at = Some(due);
    }
    if let Some(pinned) = updates.pinned {
        task.pinned = if pinned { 1 } else { 0 };
    }
    if let Some(weight) = updates.focus_weight {
        task.focus_weight = weight;
    }

    let updated = db::tasks::update(pool, &task).await?;
    if !updated {
        return Err(GranaryError::VersionMismatch {
            expected: task.version,
            found: task.version + 1,
        });
    }

    // Log event
    let event_type = if updates.status.is_some() && old_status != task.status {
        EventType::TaskStatusChanged
    } else {
        EventType::TaskUpdated
    };

    db::events::create(
        pool,
        &CreateEvent {
            event_type,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({
                "old_status": old_status,
                "new_status": task.status,
            }),
        },
    )
    .await?;

    get_task(pool, id).await
}

/// Mark a draft task as ready (transition Draft -> Todo)
pub async fn ready_task(pool: &SqlitePool, id: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    // Check if task is in Draft status
    if !task.status_enum().is_draft() {
        return Err(GranaryError::Conflict(format!(
            "Task {} is not in Draft status (current status: {}). Only draft tasks can be marked as ready.",
            id, task.status
        )));
    }

    task.status = TaskStatus::Todo.as_str().to_string();

    let updated = db::tasks::update(pool, &task).await?;
    if !updated {
        return Err(GranaryError::VersionMismatch {
            expected: task.version,
            found: task.version + 1,
        });
    }

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::TaskUpdated,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({
                "old_status": "draft",
                "new_status": "todo",
                "action": "ready",
            }),
        },
    )
    .await?;

    get_task(pool, id).await
}

/// Start a task (set status to in_progress)
pub async fn start_task(pool: &SqlitePool, id: &str, owner: Option<String>) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    // Check if task is in Draft status
    if task.status_enum().is_draft() {
        return Err(GranaryError::Conflict(format!(
            "Task {} is in Draft status. Use 'granary task {} ready' to mark it as ready before starting.",
            id, id
        )));
    }

    // Check if task is already terminal
    if task.status_enum().is_terminal() {
        return Err(GranaryError::Conflict(format!(
            "Task {} is already completed",
            id
        )));
    }

    // Check dependencies
    let unmet = db::dependencies::get_unmet(pool, id).await?;
    if !unmet.is_empty() {
        let unmet_ids: Vec<_> = unmet.iter().map(|t| t.id.clone()).collect();
        return Err(GranaryError::UnmetDependencies(unmet_ids.join(", ")));
    }

    task.status = TaskStatus::InProgress.as_str().to_string();
    if task.started_at.is_none() {
        task.started_at = Some(chrono::Utc::now().to_rfc3339());
    }
    if let Some(o) = owner {
        task.owner = Some(o);
    }

    db::tasks::update(pool, &task).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::TaskStarted,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: task.owner.clone(),
            session_id: None,
            payload: serde_json::json!({}),
        },
    )
    .await?;

    get_task(pool, id).await
}

/// Complete a task
pub async fn complete_task(pool: &SqlitePool, id: &str, comment: Option<&str>) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    task.status = TaskStatus::Done.as_str().to_string();
    task.completed_at = Some(chrono::Utc::now().to_rfc3339());
    task.blocked_reason = None;

    db::tasks::update(pool, &task).await?;

    // Add completion comment if provided
    if let Some(content) = comment {
        let scope = format!("task:{}:comment", id);
        let comment_number = counters::next(pool, &scope).await?;
        let comment_id = generate_comment_id(id, comment_number);
        let now = chrono::Utc::now().to_rfc3339();

        let comment = Comment {
            id: comment_id,
            parent_type: "task".to_string(),
            parent_id: id.to_string(),
            comment_number,
            kind: CommentKind::Progress.as_str().to_string(),
            content: content.to_string(),
            author: task.owner.clone(),
            meta: None,
            created_at: now.clone(),
            updated_at: now,
            version: 1,
        };
        db::comments::create(pool, &comment).await?;
    }

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::TaskCompleted,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: task.owner.clone(),
            session_id: None,
            payload: serde_json::json!({}),
        },
    )
    .await?;

    get_task(pool, id).await
}

/// Block a task
pub async fn block_task(pool: &SqlitePool, id: &str, reason: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    task.status = TaskStatus::Blocked.as_str().to_string();
    task.blocked_reason = Some(reason.to_string());

    db::tasks::update(pool, &task).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::TaskBlocked,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({
                "reason": reason,
            }),
        },
    )
    .await?;

    get_task(pool, id).await
}

/// Unblock a task
pub async fn unblock_task(pool: &SqlitePool, id: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    // Return to in_progress if it was started, otherwise todo
    task.status = if task.started_at.is_some() {
        TaskStatus::InProgress.as_str().to_string()
    } else {
        TaskStatus::Todo.as_str().to_string()
    };
    task.blocked_reason = None;

    db::tasks::update(pool, &task).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::TaskUnblocked,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({}),
        },
    )
    .await?;

    get_task(pool, id).await
}

/// Claim a task with a lease
pub async fn claim_task(
    pool: &SqlitePool,
    id: &str,
    owner: &str,
    lease_minutes: Option<u32>,
) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    // Check if already claimed by someone else
    if task.is_claimed() {
        let claim = task.claim_info().unwrap();
        if claim.owner != owner {
            return Err(GranaryError::ClaimConflict {
                owner: claim.owner,
                expires_at: claim.lease_expires_at.unwrap_or_default(),
            });
        }
    }

    let now = chrono::Utc::now();
    task.claim_owner = Some(owner.to_string());
    task.claim_claimed_at = Some(now.to_rfc3339());

    if let Some(minutes) = lease_minutes {
        let expires = now + chrono::Duration::minutes(minutes as i64);
        task.claim_lease_expires_at = Some(expires.to_rfc3339());
    }

    db::tasks::update(pool, &task).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::TaskClaimed,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: Some(owner.to_string()),
            session_id: None,
            payload: serde_json::json!({
                "lease_minutes": lease_minutes,
            }),
        },
    )
    .await?;

    get_task(pool, id).await
}

/// Extend the lease on a claimed task (heartbeat)
pub async fn heartbeat_task(pool: &SqlitePool, id: &str, lease_minutes: u32) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    if task.claim_owner.is_none() {
        return Err(GranaryError::Conflict(format!(
            "Task {} is not claimed",
            id
        )));
    }

    let expires = chrono::Utc::now() + chrono::Duration::minutes(lease_minutes as i64);
    task.claim_lease_expires_at = Some(expires.to_rfc3339());

    db::tasks::update(pool, &task).await?;

    get_task(pool, id).await
}

/// Release a claim on a task
pub async fn release_task(pool: &SqlitePool, id: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    let owner = task.claim_owner.clone();
    task.claim_owner = None;
    task.claim_claimed_at = None;
    task.claim_lease_expires_at = None;

    db::tasks::update(pool, &task).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::TaskReleased,
            entity_type: EntityType::Task,
            entity_id: task.id.clone(),
            actor: owner,
            session_id: None,
            payload: serde_json::json!({}),
        },
    )
    .await?;

    get_task(pool, id).await
}

/// Add a dependency to a task
pub async fn add_dependency(pool: &SqlitePool, task_id: &str, depends_on: &str) -> Result<()> {
    // Verify both tasks exist
    let _task = get_task(pool, task_id).await?;
    let _dep = get_task(pool, depends_on).await?;

    // Check for cycles
    if db::dependencies::would_create_cycle(pool, task_id, depends_on).await? {
        return Err(GranaryError::DependencyCycle(format!(
            "Adding dependency {} -> {} would create a cycle",
            task_id, depends_on
        )));
    }

    db::dependencies::add(pool, task_id, depends_on).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::DependencyAdded,
            entity_type: EntityType::Task,
            entity_id: task_id.to_string(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({
                "depends_on": depends_on,
            }),
        },
    )
    .await?;

    Ok(())
}

/// Remove a dependency from a task
pub async fn remove_dependency(pool: &SqlitePool, task_id: &str, depends_on: &str) -> Result<bool> {
    let removed = db::dependencies::remove(pool, task_id, depends_on).await?;

    if removed {
        // Log event
        db::events::create(
            pool,
            &CreateEvent {
                event_type: EventType::DependencyRemoved,
                entity_type: EntityType::Task,
                entity_id: task_id.to_string(),
                actor: None,
                session_id: None,
                payload: serde_json::json!({
                    "depends_on": depends_on,
                }),
            },
        )
        .await?;
    }

    Ok(removed)
}

/// List dependencies of a task
pub async fn list_dependencies(pool: &SqlitePool, task_id: &str) -> Result<Vec<Task>> {
    let deps = db::dependencies::list(pool, task_id).await?;
    let mut tasks = Vec::new();
    for dep in deps {
        if let Ok(task) = get_task(pool, &dep.depends_on_task_id).await {
            tasks.push(task);
        }
    }
    Ok(tasks)
}

/// Get the next actionable task
pub async fn get_next_task(
    pool: &SqlitePool,
    project_ids: Option<&[String]>,
) -> Result<Option<Task>> {
    db::tasks::get_next(pool, project_ids).await
}

/// Get all currently actionable tasks
pub async fn get_all_next_tasks(
    pool: &SqlitePool,
    project_ids: Option<&[String]>,
) -> Result<Vec<Task>> {
    db::tasks::get_all_next(pool, project_ids).await
}

/// Pin a task
pub async fn pin_task(pool: &SqlitePool, id: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;
    task.pinned = 1;
    db::tasks::update(pool, &task).await?;
    get_task(pool, id).await
}

/// Unpin a task
pub async fn unpin_task(pool: &SqlitePool, id: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;
    task.pinned = 0;
    db::tasks::update(pool, &task).await?;
    get_task(pool, id).await
}

/// Get unmet dependencies for a task (task IDs that this task is blocked by)
pub async fn get_unmet_dependency_ids(pool: &SqlitePool, task_id: &str) -> Result<Vec<String>> {
    let unmet = db::dependencies::get_unmet(pool, task_id).await?;
    Ok(unmet.iter().map(|t| t.id.clone()).collect())
}

/// Get a task with its unmet dependency information
pub async fn get_task_with_deps(pool: &SqlitePool, id: &str) -> Result<(Task, Vec<String>)> {
    let task = get_task(pool, id).await?;
    let blocked_by = get_unmet_dependency_ids(pool, id).await?;
    Ok((task, blocked_by))
}

/// Get multiple tasks with their unmet dependency information
pub async fn get_tasks_with_deps(
    pool: &SqlitePool,
    tasks: Vec<Task>,
) -> Result<Vec<(Task, Vec<String>)>> {
    let mut result = Vec::with_capacity(tasks.len());
    for task in tasks {
        let blocked_by = get_unmet_dependency_ids(pool, &task.id).await?;
        result.push((task, blocked_by));
    }
    Ok(result)
}
