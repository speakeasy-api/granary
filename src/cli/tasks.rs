use granary_types::{CreateTask, UpdateTask};

use crate::cli::args::{ArtifactAction, CommentAction, DepsAction, SubtaskAction, TaskAction};
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::db;
use crate::error::Result;
use crate::models::*;
use crate::output::{Formatter, OutputFormat};
use crate::services::{self, Workspace};
use std::time::Duration;

/// List tasks
pub async fn list_tasks(
    all: bool,
    status: Option<String>,
    priority: Option<String>,
    owner: Option<String>,
    format: OutputFormat,
    watch: bool,
    interval: u64,
) -> Result<()> {
    if watch {
        let interval_duration = Duration::from_secs(interval);
        watch_loop(interval_duration, || async {
            let output = fetch_and_format_tasks(
                all,
                status.clone(),
                priority.clone(),
                owner.clone(),
                format,
            )
            .await?;
            Ok(format!(
                "{}\n\n{}",
                watch_status_line(interval_duration),
                output
            ))
        })
        .await?;
    } else {
        let output = fetch_and_format_tasks(all, status, priority, owner, format).await?;
        println!("{}", output);
    }

    Ok(())
}

/// Fetch tasks and format them for display
async fn fetch_and_format_tasks(
    all: bool,
    status: Option<String>,
    priority: Option<String>,
    owner: Option<String>,
    format: OutputFormat,
) -> Result<String> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let tasks = if all || status.is_some() || priority.is_some() || owner.is_some() {
        services::list_tasks_filtered(
            &pool,
            status.as_deref(),
            priority.as_deref(),
            owner.as_deref(),
        )
        .await?
    } else {
        // Default: show tasks in current session scope, or all if no session
        if let Some(session_id) = workspace.current_session_id() {
            let project_ids =
                services::get_scope_by_type(&pool, &session_id, ScopeItemType::Project).await?;
            if project_ids.is_empty() {
                services::list_all_tasks(&pool).await?
            } else {
                let mut all_tasks = Vec::new();
                for project_id in project_ids {
                    let tasks = services::list_tasks_by_project(&pool, &project_id).await?;
                    all_tasks.extend(tasks);
                }
                all_tasks
            }
        } else {
            services::list_all_tasks(&pool).await?
        }
    };

    // Enrich tasks with dependency information
    let tasks_with_deps = services::get_tasks_with_deps(&pool, tasks).await?;

    let formatter = Formatter::new(format);
    Ok(formatter.format_tasks_with_deps(&tasks_with_deps))
}

/// Show or manage a task
pub async fn task(id: &str, action: Option<TaskAction>, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;
    let formatter = Formatter::new(format);

    match action {
        None => {
            let (task, blocked_by) = services::get_task_with_deps(&pool, id).await?;
            println!("{}", formatter.format_task_with_deps(&task, blocked_by));
        }

        Some(TaskAction::Update {
            title,
            description,
            status,
            priority,
            owner,
            tags,
            due,
        }) => {
            let status = status.as_ref().and_then(|s| s.parse().ok());
            let priority = priority.as_ref().and_then(|p| p.parse().ok());
            let tags = tags.map(|t| t.split(',').map(|s| s.trim().to_string()).collect());

            let task = services::update_task(
                &pool,
                id,
                UpdateTask {
                    title,
                    description,
                    status,
                    priority,
                    owner,
                    tags,
                    due_at: due,
                    ..Default::default()
                },
            )
            .await?;

            println!("{}", formatter.format_task(&task));
        }

        Some(TaskAction::Ready) => {
            let task = services::ready_task(&pool, id).await?;
            println!("{}", formatter.format_task(&task));
        }

        Some(TaskAction::Start { owner, lease }) => {
            let task = services::start_task(&pool, id, owner.clone()).await?;
            if let Some(minutes) = lease {
                let owner_name = owner.unwrap_or_else(|| "unknown".to_string());
                services::claim_task(&pool, id, &owner_name, Some(minutes)).await?;
            }
            println!("{}", formatter.format_task(&task));
        }

        Some(TaskAction::Done { comment }) => {
            let task = services::complete_task(&pool, id, comment.as_deref()).await?;
            println!("{}", formatter.format_task(&task));
        }

        Some(TaskAction::Block { reason }) => {
            let task = services::block_task(&pool, id, &reason).await?;
            println!("{}", formatter.format_task(&task));
        }

        Some(TaskAction::Unblock) => {
            let task = services::unblock_task(&pool, id).await?;
            println!("{}", formatter.format_task(&task));
        }

        Some(TaskAction::Claim { owner, lease }) => {
            let task = services::claim_task(&pool, id, &owner, lease).await?;
            println!("{}", formatter.format_task(&task));
        }

        Some(TaskAction::Heartbeat { lease }) => {
            let task = services::heartbeat_task(&pool, id, lease).await?;
            println!("Lease extended for task {}", task.id);
        }

        Some(TaskAction::Release) => {
            let task = services::release_task(&pool, id).await?;
            println!("Released claim on task {}", task.id);
        }

        Some(TaskAction::Deps { action }) => {
            handle_deps(id, action, &pool, format).await?;
        }

        Some(TaskAction::Tasks { action }) => match action {
            None => {
                let subtasks = services::list_subtasks(&pool, id).await?;
                println!("{}", formatter.format_tasks(&subtasks));
            }
            Some(SubtaskAction::Create {
                title,
                description,
                priority,
                owner,
            }) => {
                let task = services::get_task(&pool, id).await?;
                let priority = priority.parse().unwrap_or_default();

                let subtask = services::create_task(
                    &pool,
                    CreateTask {
                        project_id: task.project_id,
                        parent_task_id: Some(id.to_string()),
                        title,
                        description,
                        priority,
                        owner,
                        ..Default::default()
                    },
                )
                .await?;

                println!("{}", formatter.format_task_created(&subtask));
            }
        },

        Some(TaskAction::Comments { action }) => match action {
            None => {
                let comments = db::comments::list_by_parent(&pool, id).await?;
                println!("{}", formatter.format_comments(&comments));
            }
            Some(CommentAction::Create {
                content_positional,
                content_flag,
                kind,
                author,
            }) => {
                let content = content_positional
                    .or(content_flag)
                    .ok_or_else(|| crate::error::GranaryError::InvalidArgument(
                        "content is required (provide as positional argument or with --content flag)".to_string()
                    ))?;
                let comment = create_comment(&pool, id, &content, &kind, author).await?;
                println!("{}", formatter.format_comment(&comment));
            }
        },

        Some(TaskAction::Artifacts { action }) => match action {
            None => {
                let artifacts = db::artifacts::list_by_parent(&pool, id).await?;
                println!("{}", formatter.format_artifacts(&artifacts));
            }
            Some(ArtifactAction::Add {
                artifact_type,
                path,
                description,
            }) => {
                let artifact =
                    create_artifact(&pool, id, &artifact_type, &path, description).await?;
                println!("{}", formatter.format_artifact(&artifact));
            }
            Some(ArtifactAction::Rm { artifact_id }) => {
                db::artifacts::delete(&pool, &artifact_id).await?;
                println!("Removed artifact {}", artifact_id);
            }
        },
    }

    Ok(())
}

async fn handle_deps(
    task_id: &str,
    action: DepsAction,
    pool: &sqlx::SqlitePool,
    _format: OutputFormat,
) -> Result<()> {
    match action {
        DepsAction::Add { task_ids } => {
            for dep_id in task_ids {
                services::add_dependency(pool, task_id, &dep_id).await?;
                println!("Added dependency: {} -> {}", task_id, dep_id);
            }
        }

        DepsAction::Rm { task_id: dep_id } => {
            let removed = services::remove_dependency(pool, task_id, &dep_id).await?;
            if removed {
                println!("Removed dependency: {} -> {}", task_id, dep_id);
            } else {
                println!("Dependency not found");
            }
        }

        DepsAction::Graph => {
            let deps = services::list_dependencies(pool, task_id).await?;
            if deps.is_empty() {
                println!("No dependencies for task {}", task_id);
            } else {
                println!("Dependencies for {}:", task_id);
                for dep in deps {
                    let status = if dep.status == "done" { "[x]" } else { "[ ]" };
                    println!("  {} {} ({})", status, dep.title, dep.id);
                }
            }
        }
    }

    Ok(())
}

/// Get next actionable task
pub async fn next_task(include_reason: bool, all: bool, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Get project scope from current session if any
    let project_ids = if let Some(session_id) = workspace.current_session_id() {
        let ids = services::get_scope_by_type(&pool, &session_id, ScopeItemType::Project).await?;
        if ids.is_empty() { None } else { Some(ids) }
    } else {
        None
    };

    let formatter = Formatter::new(format);

    if all {
        // Get all available tasks
        let tasks = services::get_all_next_tasks(&pool, project_ids.as_deref()).await?;
        println!("{}", formatter.format_tasks(&tasks));
    } else {
        // Get single next task
        let task = services::get_next_task(&pool, project_ids.as_deref()).await?;

        let reason = if include_reason {
            Some("Selected based on: priority, due date, creation time; all dependencies satisfied")
        } else {
            None
        };

        println!("{}", formatter.format_next_task(task.as_ref(), reason));
    }

    Ok(())
}

/// Start a task (shortcut command)
pub async fn start_task(
    task_id: &str,
    owner: Option<String>,
    lease: Option<u32>,
    format: OutputFormat,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let task = services::start_task(&pool, task_id, owner.clone()).await?;

    if let Some(minutes) = lease {
        let owner_name = owner.unwrap_or_else(|| "unknown".to_string());
        services::claim_task(&pool, task_id, &owner_name, Some(minutes)).await?;
    }

    // Set as focus task in current session if any
    if let Some(session_id) = workspace.current_session_id() {
        services::set_focus_task(&pool, &session_id, task_id).await?;
    }

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_task(&task));

    Ok(())
}

/// Focus on a task
pub async fn focus_task(task_id: &str, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let session_id = workspace
        .current_session_id()
        .ok_or(crate::error::GranaryError::NoActiveSession)?;

    services::set_focus_task(&pool, &session_id, task_id).await?;

    let task = services::get_task(&pool, task_id).await?;
    let formatter = Formatter::new(format);
    println!("Focus set to: {}", task.title);
    println!("{}", formatter.format_task(&task));

    Ok(())
}

/// Pin a task
pub async fn pin_task(task_id: &str) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    services::pin_task(&pool, task_id).await?;
    println!("Pinned task {}", task_id);

    // Also add to session scope if there's an active session
    if let Some(session_id) = workspace.current_session_id() {
        services::add_to_scope(&pool, &session_id, ScopeItemType::Task, task_id).await?;
    }

    Ok(())
}

/// Unpin a task
pub async fn unpin_task(task_id: &str) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    services::unpin_task(&pool, task_id).await?;
    println!("Unpinned task {}", task_id);

    Ok(())
}

async fn create_comment(
    pool: &sqlx::SqlitePool,
    parent_id: &str,
    content: &str,
    kind: &str,
    author: Option<String>,
) -> Result<Comment> {
    let scope = format!("task:{}:comment", parent_id);
    let comment_number = db::counters::next(pool, &scope).await?;
    let id = generate_comment_id(parent_id, comment_number);
    let now = chrono::Utc::now().to_rfc3339();

    let comment_kind: CommentKind = kind.parse().unwrap_or_default();

    let comment = Comment {
        id: id.clone(),
        parent_type: "task".to_string(),
        parent_id: parent_id.to_string(),
        comment_number,
        kind: comment_kind.as_str().to_string(),
        content: content.to_string(),
        author,
        meta: None,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };

    db::comments::create(pool, &comment).await?;

    // Log event
    db::events::create(
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

async fn create_artifact(
    pool: &sqlx::SqlitePool,
    parent_id: &str,
    artifact_type: &str,
    path: &str,
    description: Option<String>,
) -> Result<Artifact> {
    let scope = format!("task:{}:artifact", parent_id);
    let artifact_number = db::counters::next(pool, &scope).await?;
    let id = generate_artifact_id(parent_id, artifact_number);
    let now = chrono::Utc::now().to_rfc3339();

    let art_type: ArtifactType = artifact_type.parse().unwrap_or_default();

    let artifact = Artifact {
        id: id.clone(),
        parent_type: "task".to_string(),
        parent_id: parent_id.to_string(),
        artifact_number,
        artifact_type: art_type.as_str().to_string(),
        path_or_url: path.to_string(),
        description,
        meta: None,
        created_at: now,
    };

    db::artifacts::create(pool, &artifact).await?;

    Ok(artifact)
}
