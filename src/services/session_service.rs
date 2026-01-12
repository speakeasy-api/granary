use sqlx::SqlitePool;

use crate::db::{self, counters};
use crate::error::{GranaryError, Result};
use crate::models::*;
use crate::services::Workspace;

/// Create a new session
pub async fn create_session(pool: &SqlitePool, input: CreateSession) -> Result<Session> {
    let id = generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();

    let session = Session {
        id: id.clone(),
        name: input.name,
        owner: input.owner,
        mode: Some(input.mode.as_str().to_string()),
        focus_task_id: None,
        variables: None,
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
    };

    db::sessions::create(pool, &session).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::SessionStarted,
            entity_type: EntityType::Session,
            entity_id: session.id.clone(),
            actor: session.owner.clone(),
            session_id: Some(session.id.clone()),
            payload: serde_json::json!({
                "name": session.name,
                "mode": session.mode,
            }),
        },
    )
    .await?;

    Ok(session)
}

/// Get a session by ID
pub async fn get_session(pool: &SqlitePool, id: &str) -> Result<Session> {
    db::sessions::get(pool, id)
        .await?
        .ok_or_else(|| GranaryError::SessionNotFound(id.to_string()))
}

/// List sessions
pub async fn list_sessions(pool: &SqlitePool, include_closed: bool) -> Result<Vec<Session>> {
    db::sessions::list(pool, include_closed).await
}

/// Update a session
pub async fn update_session(
    pool: &SqlitePool,
    id: &str,
    updates: UpdateSession,
) -> Result<Session> {
    let mut session = get_session(pool, id).await?;

    if let Some(name) = updates.name {
        session.name = Some(name);
    }
    if let Some(owner) = updates.owner {
        session.owner = Some(owner);
    }
    if let Some(mode) = updates.mode {
        session.mode = Some(mode.as_str().to_string());
    }
    if let Some(focus) = updates.focus_task_id {
        session.focus_task_id = Some(focus);
    }
    if let Some(vars) = updates.variables {
        session.variables = Some(serde_json::to_string(&vars)?);
    }

    db::sessions::update(pool, &session).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::SessionUpdated,
            entity_type: EntityType::Session,
            entity_id: session.id.clone(),
            actor: session.owner.clone(),
            session_id: Some(session.id.clone()),
            payload: serde_json::json!({}),
        },
    )
    .await?;

    get_session(pool, id).await
}

/// Close a session
pub async fn close_session(
    pool: &SqlitePool,
    id: &str,
    summary: Option<&str>,
    workspace: &Workspace,
) -> Result<Session> {
    let session = get_session(pool, id).await?;

    if session.is_closed() {
        return Err(GranaryError::Conflict(format!(
            "Session {} is already closed",
            id
        )));
    }

    db::sessions::close(pool, id).await?;

    // Clean up session-attached steering files
    let deleted_steering = db::steering::delete_by_session(pool, id).await?;
    if deleted_steering > 0 {
        // Steering cleanup is silent - no event logged as it's automatic cleanup
    }

    // Add summary as a comment if provided
    if let Some(content) = summary {
        let scope = format!("session:{}:comment", id);
        let comment_number = counters::next(pool, &scope).await?;
        let comment_id = generate_comment_id(id, comment_number);
        let now = chrono::Utc::now().to_rfc3339();

        let comment = Comment {
            id: comment_id,
            parent_type: "session".to_string(),
            parent_id: id.to_string(),
            comment_number,
            kind: CommentKind::Handoff.as_str().to_string(),
            content: content.to_string(),
            author: session.owner.clone(),
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
            event_type: EventType::SessionClosed,
            entity_type: EntityType::Session,
            entity_id: id.to_string(),
            actor: session.owner.clone(),
            session_id: Some(id.to_string()),
            payload: serde_json::json!({
                "summary": summary,
            }),
        },
    )
    .await?;

    // Clear current session if it's this one
    if workspace.current_session_id() == Some(id.to_string()) {
        workspace.clear_current_session()?;
    }

    get_session(pool, id).await
}

/// Add an item to the session scope
pub async fn add_to_scope(
    pool: &SqlitePool,
    session_id: &str,
    item_type: ScopeItemType,
    item_id: &str,
) -> Result<()> {
    // Verify session exists
    let _session = get_session(pool, session_id).await?;

    db::sessions::add_scope(pool, session_id, item_type.as_str(), item_id).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::SessionScopeAdded,
            entity_type: EntityType::Session,
            entity_id: session_id.to_string(),
            actor: None,
            session_id: Some(session_id.to_string()),
            payload: serde_json::json!({
                "item_type": item_type.as_str(),
                "item_id": item_id,
            }),
        },
    )
    .await?;

    Ok(())
}

/// Remove an item from the session scope
pub async fn remove_from_scope(
    pool: &SqlitePool,
    session_id: &str,
    item_type: ScopeItemType,
    item_id: &str,
) -> Result<bool> {
    let removed = db::sessions::remove_scope(pool, session_id, item_type.as_str(), item_id).await?;

    if removed {
        // Log event
        db::events::create(
            pool,
            &CreateEvent {
                event_type: EventType::SessionScopeRemoved,
                entity_type: EntityType::Session,
                entity_id: session_id.to_string(),
                actor: None,
                session_id: Some(session_id.to_string()),
                payload: serde_json::json!({
                    "item_type": item_type.as_str(),
                    "item_id": item_id,
                }),
            },
        )
        .await?;
    }

    Ok(removed)
}

/// Get the session scope
pub async fn get_scope(pool: &SqlitePool, session_id: &str) -> Result<Vec<SessionScope>> {
    db::sessions::get_scope(pool, session_id).await
}

/// Get scope items of a specific type
pub async fn get_scope_by_type(
    pool: &SqlitePool,
    session_id: &str,
    item_type: ScopeItemType,
) -> Result<Vec<String>> {
    let scope = db::sessions::get_scope_by_type(pool, session_id, item_type.as_str()).await?;
    Ok(scope.into_iter().map(|s| s.item_id).collect())
}

/// Set the focus task for a session
pub async fn set_focus_task(pool: &SqlitePool, session_id: &str, task_id: &str) -> Result<Session> {
    // Verify task exists
    let _task = crate::services::get_task(pool, task_id).await?;

    let mut session = get_session(pool, session_id).await?;
    session.focus_task_id = Some(task_id.to_string());
    db::sessions::update(pool, &session).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::SessionFocusChanged,
            entity_type: EntityType::Session,
            entity_id: session_id.to_string(),
            actor: None,
            session_id: Some(session_id.to_string()),
            payload: serde_json::json!({
                "focus_task_id": task_id,
            }),
        },
    )
    .await?;

    get_session(pool, session_id).await
}

/// Clear the focus task for a session
pub async fn clear_focus_task(pool: &SqlitePool, session_id: &str) -> Result<Session> {
    let mut session = get_session(pool, session_id).await?;
    session.focus_task_id = None;
    db::sessions::update(pool, &session).await?;
    get_session(pool, session_id).await
}

/// Get session environment variables for shell export
pub fn get_session_env(session_id: &str, workspace_path: &str) -> String {
    let mut output = String::new();
    output.push_str(&format!("export GRANARY_SESSION={}\n", session_id));
    output.push_str(&format!("export GRANARY_HOME={}\n", workspace_path));
    output
}

/// Get the current session for a workspace
pub async fn get_current_session(
    pool: &SqlitePool,
    workspace: &Workspace,
) -> Result<Option<Session>> {
    match workspace.current_session_id() {
        Some(id) => {
            let session = db::sessions::get(pool, &id).await?;
            Ok(session)
        }
        None => Ok(None),
    }
}

/// Set the current session for a workspace
pub fn set_current_session(workspace: &Workspace, session_id: &str) -> Result<()> {
    workspace.set_current_session(session_id)
}
