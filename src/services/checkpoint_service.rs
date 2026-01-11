use sqlx::SqlitePool;

use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::*;
use crate::output::json::{CheckpointDiff, DiffChange};
use crate::services::{get_scope, get_session};

/// Create a checkpoint for a session
pub async fn create_checkpoint(
    pool: &SqlitePool,
    session_id: &str,
    name: &str,
) -> Result<Checkpoint> {
    let session = get_session(pool, session_id).await?;

    // Check for duplicate name
    if db::checkpoints::get_by_name(pool, session_id, name)
        .await?
        .is_some()
    {
        return Err(GranaryError::Conflict(format!(
            "Checkpoint '{}' already exists in session {}",
            name, session_id
        )));
    }

    // Build snapshot
    let scope = get_scope(pool, session_id).await?;
    let scope_items: Vec<ScopeItem> = scope
        .iter()
        .map(|s| ScopeItem {
            item_type: s.item_type.clone(),
            item_id: s.item_id.clone(),
        })
        .collect();

    // Get task snapshots for tasks in scope
    let mut task_snapshots = Vec::new();
    for item in &scope_items {
        if item.item_type != "task" {
            continue;
        }
        if let Ok(task) = crate::services::get_task(pool, &item.item_id).await {
            task_snapshots.push(TaskSnapshot {
                id: task.id,
                status: task.status,
                priority: task.priority,
                owner: task.owner,
                blocked_reason: task.blocked_reason,
                pinned: task.pinned != 0,
                focus_weight: task.focus_weight,
            });
        }
    }

    let snapshot = SessionSnapshot {
        session: SessionSnapshotData {
            id: session.id.clone(),
            name: session.name.clone(),
            owner: session.owner.clone(),
            mode: session.mode.clone(),
            focus_task_id: session.focus_task_id.clone(),
        },
        scope: scope_items,
        tasks: task_snapshots,
        variables: session.variables_map(),
    };

    let id = generate_checkpoint_id();
    let now = chrono::Utc::now().to_rfc3339();

    let checkpoint = Checkpoint {
        id: id.clone(),
        session_id: session_id.to_string(),
        name: name.to_string(),
        snapshot: serde_json::to_string(&snapshot)?,
        created_at: now,
    };

    db::checkpoints::create(pool, &checkpoint).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::CheckpointCreated,
            entity_type: EntityType::Checkpoint,
            entity_id: id,
            actor: None,
            session_id: Some(session_id.to_string()),
            payload: serde_json::json!({
                "name": name,
            }),
        },
    )
    .await?;

    Ok(checkpoint)
}

/// Get a checkpoint by ID
pub async fn get_checkpoint(pool: &SqlitePool, id: &str) -> Result<Checkpoint> {
    db::checkpoints::get(pool, id)
        .await?
        .ok_or_else(|| GranaryError::CheckpointNotFound(id.to_string()))
}

/// Get a checkpoint by name within a session
pub async fn get_checkpoint_by_name(
    pool: &SqlitePool,
    session_id: &str,
    name: &str,
) -> Result<Checkpoint> {
    db::checkpoints::get_by_name(pool, session_id, name)
        .await?
        .ok_or_else(|| GranaryError::CheckpointNotFound(name.to_string()))
}

/// List checkpoints for a session
pub async fn list_checkpoints(pool: &SqlitePool, session_id: &str) -> Result<Vec<Checkpoint>> {
    db::checkpoints::list_by_session(pool, session_id).await
}

/// Diff two checkpoints
pub async fn diff_checkpoints(
    pool: &SqlitePool,
    session_id: &str,
    from: &str,
    to: &str,
) -> Result<CheckpointDiff> {
    // Handle "now" as current state
    let from_snapshot = if from == "now" {
        get_current_snapshot(pool, session_id).await?
    } else {
        let checkpoint = get_checkpoint_by_name(pool, session_id, from).await?;
        serde_json::from_str(&checkpoint.snapshot)?
    };

    let to_snapshot = if to == "now" {
        get_current_snapshot(pool, session_id).await?
    } else {
        let checkpoint = get_checkpoint_by_name(pool, session_id, to).await?;
        serde_json::from_str(&checkpoint.snapshot)?
    };

    let mut changes = Vec::new();

    // Compare session properties
    if from_snapshot.session.mode != to_snapshot.session.mode {
        changes.push(DiffChange {
            entity_type: "session".to_string(),
            entity_id: session_id.to_string(),
            field: "mode".to_string(),
            old_value: from_snapshot.session.mode.map(|m| serde_json::json!(m)),
            new_value: to_snapshot.session.mode.map(|m| serde_json::json!(m)),
        });
    }

    if from_snapshot.session.focus_task_id != to_snapshot.session.focus_task_id {
        changes.push(DiffChange {
            entity_type: "session".to_string(),
            entity_id: session_id.to_string(),
            field: "focus_task_id".to_string(),
            old_value: from_snapshot
                .session
                .focus_task_id
                .map(|f| serde_json::json!(f)),
            new_value: to_snapshot
                .session
                .focus_task_id
                .map(|f| serde_json::json!(f)),
        });
    }

    // Compare tasks
    let from_tasks: std::collections::HashMap<_, _> = from_snapshot
        .tasks
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();
    let to_tasks: std::collections::HashMap<_, _> = to_snapshot
        .tasks
        .into_iter()
        .map(|t| (t.id.clone(), t))
        .collect();

    // Check for changed tasks
    for (id, to_task) in &to_tasks {
        if let Some(from_task) = from_tasks.get(id) {
            if from_task.status != to_task.status {
                changes.push(DiffChange {
                    entity_type: "task".to_string(),
                    entity_id: id.clone(),
                    field: "status".to_string(),
                    old_value: Some(serde_json::json!(from_task.status)),
                    new_value: Some(serde_json::json!(to_task.status)),
                });
            }
            if from_task.priority != to_task.priority {
                changes.push(DiffChange {
                    entity_type: "task".to_string(),
                    entity_id: id.clone(),
                    field: "priority".to_string(),
                    old_value: Some(serde_json::json!(from_task.priority)),
                    new_value: Some(serde_json::json!(to_task.priority)),
                });
            }
            if from_task.owner != to_task.owner {
                changes.push(DiffChange {
                    entity_type: "task".to_string(),
                    entity_id: id.clone(),
                    field: "owner".to_string(),
                    old_value: from_task.owner.clone().map(|o| serde_json::json!(o)),
                    new_value: to_task.owner.clone().map(|o| serde_json::json!(o)),
                });
            }
        } else {
            // Task added
            changes.push(DiffChange {
                entity_type: "task".to_string(),
                entity_id: id.clone(),
                field: "added".to_string(),
                old_value: None,
                new_value: Some(serde_json::json!(to_task)),
            });
        }
    }

    // Check for removed tasks
    for id in from_tasks.keys() {
        if !to_tasks.contains_key(id) {
            changes.push(DiffChange {
                entity_type: "task".to_string(),
                entity_id: id.clone(),
                field: "removed".to_string(),
                old_value: Some(serde_json::json!(from_tasks.get(id))),
                new_value: None,
            });
        }
    }

    Ok(CheckpointDiff {
        from: from.to_string(),
        to: to.to_string(),
        changes,
    })
}

/// Restore session state from a checkpoint
pub async fn restore_checkpoint(
    pool: &SqlitePool,
    session_id: &str,
    checkpoint_name: &str,
) -> Result<()> {
    let checkpoint = get_checkpoint_by_name(pool, session_id, checkpoint_name).await?;
    let snapshot: SessionSnapshot = serde_json::from_str(&checkpoint.snapshot)?;

    // Update session
    let mut session = get_session(pool, session_id).await?;
    session.mode = snapshot.session.mode;
    session.focus_task_id = snapshot.session.focus_task_id;
    if !snapshot.variables.is_empty() {
        session.variables = Some(serde_json::to_string(&snapshot.variables)?);
    }
    db::sessions::update(pool, &session).await?;

    // Restore task states
    for task_snapshot in &snapshot.tasks {
        if let Ok(mut task) = crate::services::get_task(pool, &task_snapshot.id).await {
            task.status = task_snapshot.status.clone();
            task.priority = task_snapshot.priority.clone();
            task.owner = task_snapshot.owner.clone();
            task.blocked_reason = task_snapshot.blocked_reason.clone();
            task.pinned = if task_snapshot.pinned { 1 } else { 0 };
            task.focus_weight = task_snapshot.focus_weight;
            db::tasks::update(pool, &task).await?;
        }
    }

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::CheckpointRestored,
            entity_type: EntityType::Checkpoint,
            entity_id: checkpoint.id,
            actor: None,
            session_id: Some(session_id.to_string()),
            payload: serde_json::json!({
                "checkpoint_name": checkpoint_name,
            }),
        },
    )
    .await?;

    Ok(())
}

/// Get current state as a snapshot (for "now" in diffs)
async fn get_current_snapshot(pool: &SqlitePool, session_id: &str) -> Result<SessionSnapshot> {
    let session = get_session(pool, session_id).await?;
    let scope = get_scope(pool, session_id).await?;

    let scope_items: Vec<ScopeItem> = scope
        .iter()
        .map(|s| ScopeItem {
            item_type: s.item_type.clone(),
            item_id: s.item_id.clone(),
        })
        .collect();

    let mut task_snapshots = Vec::new();
    for item in &scope_items {
        if item.item_type != "task" {
            continue;
        }
        if let Ok(task) = crate::services::get_task(pool, &item.item_id).await {
            task_snapshots.push(TaskSnapshot {
                id: task.id,
                status: task.status,
                priority: task.priority,
                owner: task.owner,
                blocked_reason: task.blocked_reason,
                pinned: task.pinned != 0,
                focus_weight: task.focus_weight,
            });
        }
    }

    let variables = session.variables_map();
    Ok(SessionSnapshot {
        session: SessionSnapshotData {
            id: session.id.clone(),
            name: session.name.clone(),
            owner: session.owner.clone(),
            mode: session.mode.clone(),
            focus_task_id: session.focus_task_id.clone(),
        },
        scope: scope_items,
        tasks: task_snapshots,
        variables,
    })
}
