use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::error::{GranaryError, Result};
use crate::models::*;
use crate::services;

/// A batch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum BatchOp {
    #[serde(rename = "project.create")]
    ProjectCreate {
        name: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        owner: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
    },
    #[serde(rename = "project.update")]
    ProjectUpdate {
        id: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        owner: Option<String>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        tags: Option<Vec<String>>,
    },
    #[serde(rename = "project.archive")]
    ProjectArchive { id: String },

    #[serde(rename = "task.create")]
    TaskCreate {
        project_id: String,
        title: String,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        priority: Option<String>,
        #[serde(default)]
        owner: Option<String>,
        #[serde(default)]
        parent_task_id: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
    },
    #[serde(rename = "task.update")]
    TaskUpdate {
        id: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        priority: Option<String>,
        #[serde(default)]
        owner: Option<String>,
        #[serde(default)]
        tags: Option<Vec<String>>,
    },
    #[serde(rename = "task.start")]
    TaskStart {
        id: String,
        #[serde(default)]
        owner: Option<String>,
    },
    #[serde(rename = "task.done")]
    TaskDone {
        id: String,
        #[serde(default)]
        comment: Option<String>,
    },
    #[serde(rename = "task.block")]
    TaskBlock { id: String, reason: String },
    #[serde(rename = "task.unblock")]
    TaskUnblock { id: String },

    #[serde(rename = "dependency.add")]
    DependencyAdd { task_id: String, depends_on: String },
    #[serde(rename = "dependency.remove")]
    DependencyRemove { task_id: String, depends_on: String },

    #[serde(rename = "comment.create")]
    CommentCreate {
        parent: String,
        content: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        author: Option<String>,
    },
    #[serde(rename = "comment.update")]
    CommentUpdate {
        id: String,
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        kind: Option<String>,
    },

    #[serde(rename = "session.scope.add")]
    SessionScopeAdd {
        session_id: String,
        item_type: String,
        item_id: String,
    },
    #[serde(rename = "session.scope.remove")]
    SessionScopeRemove {
        session_id: String,
        item_type: String,
        item_id: String,
    },
    #[serde(rename = "session.focus")]
    SessionFocus { session_id: String, task_id: String },
}

/// Result of a batch operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResult {
    pub index: usize,
    pub op: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Batch request with multiple operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchRequest {
    pub ops: Vec<BatchOp>,
}

/// Apply a batch of operations
pub async fn apply_batch(pool: &SqlitePool, request: &BatchRequest) -> Result<Vec<BatchResult>> {
    let mut results = Vec::new();

    for (index, op) in request.ops.iter().enumerate() {
        let result = apply_single_op(pool, op).await;
        let (success, id, error) = match result {
            Ok(id) => (true, id, None),
            Err(e) => (false, None, Some(e.to_string())),
        };

        results.push(BatchResult {
            index,
            op: op_name(op),
            success,
            id,
            error,
        });
    }

    Ok(results)
}

/// Apply a single operation
async fn apply_single_op(pool: &SqlitePool, op: &BatchOp) -> Result<Option<String>> {
    match op {
        BatchOp::ProjectCreate {
            name,
            description,
            owner,
            tags,
        } => {
            let project = services::create_project(
                pool,
                CreateProject {
                    name: name.clone(),
                    description: description.clone(),
                    owner: owner.clone(),
                    tags: tags.clone(),
                    ..Default::default()
                },
            )
            .await?;
            Ok(Some(project.id))
        }

        BatchOp::ProjectUpdate {
            id,
            name,
            description,
            owner,
            status,
            tags,
        } => {
            let status = status.as_ref().and_then(|s| s.parse().ok());

            services::update_project(
                pool,
                id,
                UpdateProject {
                    name: name.clone(),
                    description: description.clone(),
                    owner: owner.clone(),
                    status,
                    tags: tags.clone(),
                    ..Default::default()
                },
            )
            .await?;
            Ok(Some(id.clone()))
        }

        BatchOp::ProjectArchive { id } => {
            services::archive_project(pool, id).await?;
            Ok(Some(id.clone()))
        }

        BatchOp::TaskCreate {
            project_id,
            title,
            description,
            priority,
            owner,
            parent_task_id,
            tags,
        } => {
            let priority = priority
                .as_ref()
                .and_then(|p| p.parse().ok())
                .unwrap_or_default();

            let task = services::create_task(
                pool,
                CreateTask {
                    project_id: project_id.clone(),
                    title: title.clone(),
                    description: description.clone(),
                    priority,
                    owner: owner.clone(),
                    parent_task_id: parent_task_id.clone(),
                    tags: tags.clone(),
                    ..Default::default()
                },
            )
            .await?;
            Ok(Some(task.id))
        }

        BatchOp::TaskUpdate {
            id,
            title,
            description,
            status,
            priority,
            owner,
            tags,
        } => {
            let status = status.as_ref().and_then(|s| s.parse().ok());
            let priority = priority.as_ref().and_then(|p| p.parse().ok());

            services::update_task(
                pool,
                id,
                UpdateTask {
                    title: title.clone(),
                    description: description.clone(),
                    status,
                    priority,
                    owner: owner.clone(),
                    tags: tags.clone(),
                    ..Default::default()
                },
            )
            .await?;
            Ok(Some(id.clone()))
        }

        BatchOp::TaskStart { id, owner } => {
            services::start_task(pool, id, owner.clone()).await?;
            Ok(Some(id.clone()))
        }

        BatchOp::TaskDone { id, comment } => {
            services::complete_task(pool, id, comment.as_deref()).await?;
            Ok(Some(id.clone()))
        }

        BatchOp::TaskBlock { id, reason } => {
            services::block_task(pool, id, reason).await?;
            Ok(Some(id.clone()))
        }

        BatchOp::TaskUnblock { id } => {
            services::unblock_task(pool, id).await?;
            Ok(Some(id.clone()))
        }

        BatchOp::DependencyAdd {
            task_id,
            depends_on,
        } => {
            services::add_dependency(pool, task_id, depends_on).await?;
            Ok(None)
        }

        BatchOp::DependencyRemove {
            task_id,
            depends_on,
        } => {
            services::remove_dependency(pool, task_id, depends_on).await?;
            Ok(None)
        }

        BatchOp::CommentCreate {
            parent,
            content,
            kind,
            author,
        } => {
            let comment_kind = kind
                .as_ref()
                .and_then(|k| k.parse().ok())
                .unwrap_or_default();

            // Determine parent type from ID format
            let parent_type = if parent.contains("-task-") {
                ParentType::Task
            } else if parent.contains("-comment-") {
                ParentType::Comment
            } else {
                ParentType::Project
            };

            let comment = create_comment(
                pool,
                CreateComment {
                    parent_type,
                    parent_id: parent.clone(),
                    kind: comment_kind,
                    content: content.clone(),
                    author: author.clone(),
                    ..Default::default()
                },
            )
            .await?;
            Ok(Some(comment.id))
        }

        BatchOp::CommentUpdate { id, content, kind } => {
            let kind = kind.as_ref().and_then(|k| k.parse().ok());
            update_comment(
                pool,
                id,
                UpdateComment {
                    content: content.clone(),
                    kind,
                    ..Default::default()
                },
            )
            .await?;
            Ok(Some(id.clone()))
        }

        BatchOp::SessionScopeAdd {
            session_id,
            item_type,
            item_id,
        } => {
            let item_type: ScopeItemType = item_type.parse().map_err(|_| {
                GranaryError::InvalidArgument(format!("Invalid item type: {}", item_type))
            })?;
            services::add_to_scope(pool, session_id, item_type, item_id).await?;
            Ok(None)
        }

        BatchOp::SessionScopeRemove {
            session_id,
            item_type,
            item_id,
        } => {
            let item_type: ScopeItemType = item_type.parse().map_err(|_| {
                GranaryError::InvalidArgument(format!("Invalid item type: {}", item_type))
            })?;
            services::remove_from_scope(pool, session_id, item_type, item_id).await?;
            Ok(None)
        }

        BatchOp::SessionFocus {
            session_id,
            task_id,
        } => {
            services::set_focus_task(pool, session_id, task_id).await?;
            Ok(None)
        }
    }
}

fn op_name(op: &BatchOp) -> String {
    match op {
        BatchOp::ProjectCreate { .. } => "project.create".to_string(),
        BatchOp::ProjectUpdate { .. } => "project.update".to_string(),
        BatchOp::ProjectArchive { .. } => "project.archive".to_string(),
        BatchOp::TaskCreate { .. } => "task.create".to_string(),
        BatchOp::TaskUpdate { .. } => "task.update".to_string(),
        BatchOp::TaskStart { .. } => "task.start".to_string(),
        BatchOp::TaskDone { .. } => "task.done".to_string(),
        BatchOp::TaskBlock { .. } => "task.block".to_string(),
        BatchOp::TaskUnblock { .. } => "task.unblock".to_string(),
        BatchOp::DependencyAdd { .. } => "dependency.add".to_string(),
        BatchOp::DependencyRemove { .. } => "dependency.remove".to_string(),
        BatchOp::CommentCreate { .. } => "comment.create".to_string(),
        BatchOp::CommentUpdate { .. } => "comment.update".to_string(),
        BatchOp::SessionScopeAdd { .. } => "session.scope.add".to_string(),
        BatchOp::SessionScopeRemove { .. } => "session.scope.remove".to_string(),
        BatchOp::SessionFocus { .. } => "session.focus".to_string(),
    }
}

/// Create a comment (used by batch operations)
async fn create_comment(pool: &SqlitePool, input: CreateComment) -> Result<Comment> {
    let scope = format!("{}:{}:comment", input.parent_type.as_str(), input.parent_id);
    let comment_number = crate::db::counters::next(pool, &scope).await?;
    let id = generate_comment_id(&input.parent_id, comment_number);
    let now = chrono::Utc::now().to_rfc3339();

    let meta = input.meta.map(|m| serde_json::to_string(&m)).transpose()?;

    let comment = Comment {
        id: id.clone(),
        parent_type: input.parent_type.as_str().to_string(),
        parent_id: input.parent_id,
        comment_number,
        kind: input.kind.as_str().to_string(),
        content: input.content,
        author: input.author,
        meta,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };

    crate::db::comments::create(pool, &comment).await?;

    // Log event
    crate::db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::CommentCreated,
            entity_type: EntityType::Comment,
            entity_id: comment.id.clone(),
            actor: comment.author.clone(),
            session_id: None,
            payload: serde_json::json!({
                "kind": comment.kind,
                "parent_id": comment.parent_id,
            }),
        },
    )
    .await?;

    Ok(comment)
}

/// Update a comment
async fn update_comment(pool: &SqlitePool, id: &str, updates: UpdateComment) -> Result<Comment> {
    let mut comment = crate::db::comments::get(pool, id)
        .await?
        .ok_or_else(|| GranaryError::CommentNotFound(id.to_string()))?;

    if let Some(content) = updates.content {
        comment.content = content;
    }
    if let Some(kind) = updates.kind {
        comment.kind = kind.as_str().to_string();
    }
    if let Some(meta) = updates.meta {
        comment.meta = Some(serde_json::to_string(&meta)?);
    }

    let updated = crate::db::comments::update(pool, &comment).await?;
    if !updated {
        return Err(GranaryError::VersionMismatch {
            expected: comment.version,
            found: comment.version + 1,
        });
    }

    crate::db::comments::get(pool, id)
        .await?
        .ok_or_else(|| GranaryError::CommentNotFound(id.to_string()))
}
