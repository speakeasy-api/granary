use granary_types::{CreateTask, Project, Task, TaskStatus, UpdateTask};
use sqlx::SqlitePool;

use crate::db::{self, counters};
use crate::error::{GranaryError, Result};
use crate::models::*;

/// Read the workspace review mode from DB config.
/// Returns Some("task") or Some("project"), or None if disabled.
pub async fn get_review_mode(pool: &SqlitePool) -> Result<Option<String>> {
    Ok(db::config::get(pool, "workflow.review_mode").await?)
}

/// Create a new task in a project
pub async fn create_task(pool: &SqlitePool, input: CreateTask) -> Result<Task> {
    // Verify project exists (trigger handles auto-reactivation if completed)
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
        owner: input.owner.clone(),
        tags,
        worker_ids: None,
        run_ids: None,
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
        last_edited_by: input.owner,
    };

    db::tasks::create(pool, &task).await?;

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
    if let Some(worker_ids) = updates.worker_ids {
        task.worker_ids = Some(serde_json::to_string(&worker_ids)?);
    }
    if let Some(run_ids) = updates.run_ids {
        task.run_ids = Some(serde_json::to_string(&run_ids)?);
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

    // Set actor for trigger-based events
    task.last_edited_by = task.owner.clone();

    let updated = db::tasks::update(pool, &task).await?;
    if !updated {
        return Err(GranaryError::VersionMismatch {
            expected: task.version,
            found: task.version + 1,
        });
    }

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
    // Set actor for trigger-based events
    task.last_edited_by = task.owner.clone();

    let updated = db::tasks::update(pool, &task).await?;
    if !updated {
        return Err(GranaryError::VersionMismatch {
            expected: task.version,
            found: task.version + 1,
        });
    }

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

    // Check if task is currently under review
    if task.status_enum().is_in_review() {
        return Err(GranaryError::Conflict(format!(
            "Task {} is in review. Use 'granary review {} reject \"feedback\"' before restarting it.",
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

    // Set actor for trigger-based events
    task.last_edited_by = task.owner.clone();

    db::tasks::update(pool, &task).await?;

    get_task(pool, id).await
}

/// Complete a task
/// If review_mode is "task", transitions to in_review instead of done.
pub async fn complete_task(pool: &SqlitePool, id: &str, comment: Option<&str>) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    let review_mode = get_review_mode(pool).await?;
    if review_mode.as_deref() == Some("task") {
        task.status = TaskStatus::InReview.as_str().to_string();
    } else {
        task.status = TaskStatus::Done.as_str().to_string();
        task.completed_at = Some(chrono::Utc::now().to_rfc3339());
    }
    task.blocked_reason = None;
    // Set actor for trigger-based events
    task.last_edited_by = task.owner.clone();

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

    // Auto-complete is handled by trg_project_auto_complete trigger

    get_task(pool, id).await
}

/// Block a task
pub async fn block_task(pool: &SqlitePool, id: &str, reason: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    task.status = TaskStatus::Blocked.as_str().to_string();
    task.blocked_reason = Some(reason.to_string());
    // Set actor for trigger-based events (system action)
    task.last_edited_by = None;

    db::tasks::update(pool, &task).await?;

    get_task(pool, id).await
}

/// Unblock a task
pub async fn unblock_task(pool: &SqlitePool, id: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    task.status = TaskStatus::Todo.as_str().to_string();
    task.blocked_reason = None;
    // Clear stale claim so task.next trigger can fire
    task.claim_owner = None;
    task.claim_claimed_at = None;
    task.claim_lease_expires_at = None;
    // Set actor for trigger-based events (system action)
    task.last_edited_by = None;

    db::tasks::update(pool, &task).await?;

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

    // Set actor for trigger-based events
    task.last_edited_by = Some(owner.to_string());

    db::tasks::update(pool, &task).await?;

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
    task.status = TaskStatus::Todo.to_string();
    // Set actor for trigger-based events
    task.last_edited_by = owner.clone();

    db::tasks::update(pool, &task).await?;

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

    Ok(())
}

/// Remove a dependency from a task
pub async fn remove_dependency(pool: &SqlitePool, task_id: &str, depends_on: &str) -> Result<bool> {
    let removed = db::dependencies::remove(pool, task_id, depends_on).await?;

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

/// Approve a task review (in_review -> done)
pub async fn approve_task(pool: &SqlitePool, id: &str, comment: Option<&str>) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    if !task.status_enum().is_in_review() {
        return Err(GranaryError::Conflict(format!(
            "Task {} is not in review (current status: {})",
            id, task.status
        )));
    }

    task.status = TaskStatus::Done.as_str().to_string();
    task.completed_at = Some(chrono::Utc::now().to_rfc3339());
    task.last_edited_by = task.owner.clone();

    db::tasks::update(pool, &task).await?;

    create_review_comment(pool, "task", id, "approved", comment, task.owner.as_deref()).await?;

    get_task(pool, id).await
}

/// Reject a task review (in_review -> todo, clear claim)
pub async fn reject_task(pool: &SqlitePool, id: &str, comment: &str) -> Result<Task> {
    let mut task = get_task(pool, id).await?;

    if !task.status_enum().is_in_review() {
        return Err(GranaryError::Conflict(format!(
            "Task {} is not in review (current status: {})",
            id, task.status
        )));
    }

    task.status = TaskStatus::Todo.as_str().to_string();
    task.blocked_reason = None;
    task.claim_owner = None;
    task.claim_claimed_at = None;
    task.claim_lease_expires_at = None;
    task.last_edited_by = task.owner.clone();

    db::tasks::update(pool, &task).await?;

    create_review_comment(
        pool,
        "task",
        id,
        "rejected",
        Some(comment),
        task.owner.as_deref(),
    )
    .await?;

    get_task(pool, id).await
}

/// Approve a project review (in_review -> completed)
pub async fn approve_project(
    pool: &SqlitePool,
    id: &str,
    comment: Option<&str>,
) -> Result<Project> {
    let mut project = crate::services::get_project(pool, id).await?;

    if !project.status_enum().is_in_review() {
        return Err(GranaryError::Conflict(format!(
            "Project {} is not in review (current status: {})",
            id, project.status
        )));
    }

    project.status = ProjectStatus::Completed.as_str().to_string();
    project.last_edited_by = project.owner.clone();

    db::projects::update(pool, &project).await?;

    create_review_comment(
        pool,
        "project",
        id,
        "approved",
        comment,
        project.owner.as_deref(),
    )
    .await?;

    crate::services::get_project(pool, id).await
}

/// Reject a project review (in_review -> active, draft tasks -> todo)
/// Must be called in a single transaction to ensure correct ordering.
pub async fn reject_project(pool: &SqlitePool, id: &str, comment: &str) -> Result<Project> {
    let project = crate::services::get_project(pool, id).await?;

    if !project.status_enum().is_in_review() {
        return Err(GranaryError::Conflict(format!(
            "Project {} is not in review (current status: {})",
            id, project.status
        )));
    }

    let mut tx = pool.begin().await?;
    let now = chrono::Utc::now().to_rfc3339();

    // 1. Transition project in_review -> active
    let updated = sqlx::query(
        r#"
        UPDATE projects
        SET status = ?, updated_at = ?, version = version + 1, last_edited_by = ?
        WHERE id = ? AND version = ?
        "#,
    )
    .bind(ProjectStatus::Active.as_str())
    .bind(&now)
    .bind(&project.owner)
    .bind(id)
    .bind(project.version)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(GranaryError::VersionMismatch {
            expected: project.version,
            found: project.version + 1,
        });
    }

    // 2. Transition draft tasks -> todo (so task.next triggers fire while project is active)
    sqlx::query(
        r#"
        UPDATE tasks
        SET status = ?, updated_at = ?, version = version + 1, last_edited_by = ?
        WHERE project_id = ? AND status = ?
        "#,
    )
    .bind(TaskStatus::Todo.as_str())
    .bind(&now)
    .bind(&project.owner)
    .bind(id)
    .bind(TaskStatus::Draft.as_str())
    .execute(&mut *tx)
    .await?;

    // 3. Add review comment
    create_review_comment_in_tx(
        &mut tx,
        "project",
        id,
        "rejected",
        Some(comment),
        project.owner.as_deref(),
    )
    .await?;

    tx.commit().await?;

    crate::services::get_project(pool, id).await
}

/// Create a review comment with structured verdict metadata
async fn create_review_comment(
    pool: &SqlitePool,
    parent_type: &str,
    parent_id: &str,
    verdict: &str,
    content: Option<&str>,
    author: Option<&str>,
) -> Result<()> {
    let scope = format!("{}:{}:comment", parent_type, parent_id);
    let comment_number = counters::next(pool, &scope).await?;
    let comment_id = generate_comment_id(parent_id, comment_number);
    let now = chrono::Utc::now().to_rfc3339();

    let meta = serde_json::json!({ "verdict": verdict });

    let comment = Comment {
        id: comment_id,
        parent_type: parent_type.to_string(),
        parent_id: parent_id.to_string(),
        comment_number,
        kind: CommentKind::Review.as_str().to_string(),
        content: content.unwrap_or("").to_string(),
        author: author.map(|s| s.to_string()),
        meta: Some(meta.to_string()),
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };
    db::comments::create(pool, &comment).await?;

    Ok(())
}

/// Create a review comment within an existing SQL transaction.
async fn create_review_comment_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    parent_type: &str,
    parent_id: &str,
    verdict: &str,
    content: Option<&str>,
    author: Option<&str>,
) -> Result<()> {
    let scope = format!("{}:{}:comment", parent_type, parent_id);
    let comment_number = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO counters (scope, value) VALUES (?, 1)
        ON CONFLICT(scope) DO UPDATE SET value = value + 1
        RETURNING value
        "#,
    )
    .bind(&scope)
    .fetch_one(&mut **tx)
    .await?;

    let comment_id = generate_comment_id(parent_id, comment_number);
    let now = chrono::Utc::now().to_rfc3339();
    let meta = serde_json::json!({ "verdict": verdict });

    sqlx::query(
        r#"
        INSERT INTO comments (id, parent_type, parent_id, comment_number, kind, content,
            author, meta, created_at, updated_at, version)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&comment_id)
    .bind(parent_type)
    .bind(parent_id)
    .bind(comment_number)
    .bind(CommentKind::Review.as_str())
    .bind(content.unwrap_or(""))
    .bind(author)
    .bind(meta.to_string())
    .bind(&now)
    .bind(&now)
    .bind(1_i64)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
