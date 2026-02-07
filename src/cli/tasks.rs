use granary_types::{CreateTask, Task, UpdateTask};

use crate::cli::args::{
    ArtifactAction, CliOutputFormat, CommentAction, DepsAction, SubtaskAction, TaskAction,
};
use crate::cli::comments::{CommentOutput, CommentsOutput};
use crate::cli::show::{ArtifactOutput, ArtifactsOutput};
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::db;
use crate::error::Result;
use crate::models::*;
use crate::output::{Output, json, prompt, table};
use crate::services::{self, Workspace};
use std::time::Duration;

// =============================================================================
// Output Types
// =============================================================================

/// Output for a list of tasks with their dependencies
pub struct TasksOutput {
    pub tasks: Vec<(Task, Vec<String>)>,
}

impl Output for TasksOutput {
    fn to_json(&self) -> String {
        json::format_tasks_with_deps(&self.tasks)
    }

    fn to_prompt(&self) -> String {
        let refs: Vec<(&Task, &[String])> =
            self.tasks.iter().map(|(t, d)| (t, d.as_slice())).collect();
        prompt::format_tasks_with_deps(&refs)
    }

    fn to_text(&self) -> String {
        table::format_tasks_with_deps(&self.tasks)
    }
}

/// Output for the "next task" command
pub struct NextTaskOutput {
    pub task: Option<Task>,
    pub reason: Option<String>,
}

impl Output for NextTaskOutput {
    fn to_json(&self) -> String {
        json::format_next_task(self.task.as_ref(), self.reason.as_deref())
    }

    fn to_prompt(&self) -> String {
        prompt::format_next_task(self.task.as_ref(), self.reason.as_deref())
    }

    fn to_text(&self) -> String {
        table::format_next_task(self.task.as_ref(), self.reason.as_deref())
    }
}

/// Output for a single task with dependencies
pub struct TaskOutput {
    pub task: Task,
    pub blocked_by: Vec<String>,
}

impl Output for TaskOutput {
    fn to_json(&self) -> String {
        json::format_task_with_deps(&self.task, self.blocked_by.clone())
    }

    fn to_prompt(&self) -> String {
        prompt::format_task_with_deps(&self.task, &self.blocked_by)
    }

    fn to_text(&self) -> String {
        table::format_task_with_deps(&self.task, &self.blocked_by)
    }
}

/// Output for task creation confirmation
pub struct TaskCreatedOutput {
    pub task: Task,
}

impl Output for TaskCreatedOutput {
    fn to_json(&self) -> String {
        json::format_task(&self.task)
    }

    fn to_prompt(&self) -> String {
        format!("Task created: {}", self.task.id)
    }

    fn to_text(&self) -> String {
        format!("Task created: {}", self.task.id)
    }
}

/// List tasks
pub async fn list_tasks(
    all: bool,
    status: Option<String>,
    priority: Option<String>,
    owner: Option<String>,
    cli_format: Option<CliOutputFormat>,
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
                cli_format,
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
        let output = fetch_and_format_tasks(all, status, priority, owner, cli_format).await?;
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
    cli_format: Option<CliOutputFormat>,
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

    let output = TasksOutput {
        tasks: tasks_with_deps,
    };
    Ok(output.format(cli_format))
}

/// Show or manage a task
pub async fn task(
    id: &str,
    action: Option<TaskAction>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    match action {
        None => {
            let (task, blocked_by) = services::get_task_with_deps(&pool, id).await?;
            let output = TaskOutput { task, blocked_by };
            println!("{}", output.format(cli_format));
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

            let output = TaskOutput {
                task,
                blocked_by: vec![],
            };
            println!("{}", output.format(cli_format));
        }

        Some(TaskAction::Ready) => {
            let task = services::ready_task(&pool, id).await?;
            let output = TaskOutput {
                task,
                blocked_by: vec![],
            };
            println!("{}", output.format(cli_format));
        }

        Some(TaskAction::Start { owner, lease }) => {
            let task = services::start_task(&pool, id, owner.clone()).await?;
            if let Some(minutes) = lease {
                let owner_name = owner.unwrap_or_else(|| "unknown".to_string());
                services::claim_task(&pool, id, &owner_name, Some(minutes)).await?;
            }
            let output = TaskOutput {
                task,
                blocked_by: vec![],
            };
            println!("{}", output.format(cli_format));
        }

        Some(TaskAction::Done { comment }) => {
            let task = services::complete_task(&pool, id, comment.as_deref()).await?;
            let output = TaskOutput {
                task,
                blocked_by: vec![],
            };
            println!("{}", output.format(cli_format));
        }

        Some(TaskAction::Block { reason }) => {
            let task = services::block_task(&pool, id, &reason).await?;
            let output = TaskOutput {
                task,
                blocked_by: vec![],
            };
            println!("{}", output.format(cli_format));
        }

        Some(TaskAction::Unblock) => {
            let task = services::unblock_task(&pool, id).await?;
            let output = TaskOutput {
                task,
                blocked_by: vec![],
            };
            println!("{}", output.format(cli_format));
        }

        Some(TaskAction::Claim { owner, lease }) => {
            let task = services::claim_task(&pool, id, &owner, lease).await?;
            let output = TaskOutput {
                task,
                blocked_by: vec![],
            };
            println!("{}", output.format(cli_format));
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
            handle_deps(id, action, &pool, cli_format).await?;
        }

        Some(TaskAction::Tasks { action }) => match action {
            None => {
                let subtasks = services::list_subtasks(&pool, id).await?;
                // Convert to tasks with empty deps for output
                let tasks_with_deps: Vec<(Task, Vec<String>)> =
                    subtasks.into_iter().map(|t| (t, vec![])).collect();
                let output = TasksOutput {
                    tasks: tasks_with_deps,
                };
                println!("{}", output.format(cli_format));
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

                let output = TaskCreatedOutput { task: subtask };
                println!("{}", output.format(cli_format));
            }
        },

        Some(TaskAction::Comments { action }) => match action {
            None => {
                let comments = db::comments::list_by_parent(&pool, id).await?;
                let output = CommentsOutput { comments };
                println!("{}", output.format(cli_format));
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
                let output = CommentOutput { comment };
                println!("{}", output.format(cli_format));
            }
        },

        Some(TaskAction::Artifacts { action }) => match action {
            None => {
                let artifacts = db::artifacts::list_by_parent(&pool, id).await?;
                let output = ArtifactsOutput { artifacts };
                println!("{}", output.format(cli_format));
            }
            Some(ArtifactAction::Add {
                artifact_type,
                path,
                description,
            }) => {
                let artifact =
                    create_artifact(&pool, id, &artifact_type, &path, description).await?;
                let output = ArtifactOutput { artifact };
                println!("{}", output.format(cli_format));
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
    _cli_format: Option<CliOutputFormat>,
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
pub async fn next_task(
    include_reason: bool,
    all: bool,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Get project scope from current session if any
    let project_ids = if let Some(session_id) = workspace.current_session_id() {
        let ids = services::get_scope_by_type(&pool, &session_id, ScopeItemType::Project).await?;
        if ids.is_empty() { None } else { Some(ids) }
    } else {
        None
    };

    if all {
        // Get all available tasks
        let tasks = services::get_all_next_tasks(&pool, project_ids.as_deref()).await?;
        // For all tasks, enrich with dependencies
        let tasks_with_deps = services::get_tasks_with_deps(&pool, tasks).await?;
        let output = TasksOutput {
            tasks: tasks_with_deps,
        };
        println!("{}", output.format(cli_format));
    } else {
        // Get single next task
        let task = services::get_next_task(&pool, project_ids.as_deref()).await?;

        let reason = if include_reason {
            Some(
                "Selected based on: priority, due date, creation time; all dependencies satisfied"
                    .to_string(),
            )
        } else {
            None
        };

        let output = NextTaskOutput { task, reason };
        println!("{}", output.format(cli_format));
    }

    Ok(())
}

/// Start a task (shortcut command)
pub async fn start_task(
    task_id: &str,
    owner: Option<String>,
    lease: Option<u32>,
    cli_format: Option<CliOutputFormat>,
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

    let output = TaskOutput {
        task,
        blocked_by: vec![],
    };
    println!("{}", output.format(cli_format));

    Ok(())
}

/// Focus on a task
pub async fn focus_task(task_id: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let session_id = workspace
        .current_session_id()
        .ok_or(crate::error::GranaryError::NoActiveSession)?;

    services::set_focus_task(&pool, &session_id, task_id).await?;

    let task = services::get_task(&pool, task_id).await?;
    println!("Focus set to: {}", task.title);
    let output = TaskOutput {
        task,
        blocked_by: vec![],
    };
    println!("{}", output.format(cli_format));

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
