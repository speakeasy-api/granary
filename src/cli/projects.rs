use granary_types::{CreateProject, CreateTask, UpdateProject};

use crate::cli::args::{
    ProjectAction, ProjectDepsAction, ProjectSteerAction, ProjectTasksAction, ProjectsAction,
};
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::db;
use crate::error::Result;
use crate::output::{Formatter, OutputFormat};
use crate::services::{self, Workspace};
use std::time::Duration;

/// Handle projects command (list or create)
pub async fn projects(
    action: Option<ProjectsAction>,
    include_archived: bool,
    format: OutputFormat,
    watch: bool,
    interval: u64,
) -> Result<()> {
    match action {
        None => {
            // List projects - support watch mode
            if watch {
                let interval_duration = Duration::from_secs(interval);
                watch_loop(interval_duration, || async {
                    let output = fetch_and_format_projects(include_archived, format)
                        .await
                        .map_err(|e| anyhow::anyhow!("{}", e))?;
                    Ok(format!(
                        "{}\n\n{}",
                        watch_status_line(interval_duration),
                        output
                    ))
                })
                .await?;
                Ok(())
            } else {
                let output = fetch_and_format_projects(include_archived, format).await?;
                println!("{}", output);
                Ok(())
            }
        }
        Some(ProjectsAction::Create {
            name,
            description,
            owner,
            tags,
        }) => create_project(&name, description, owner, tags, format).await,
    }
}

/// Fetch and format all projects as a string
async fn fetch_and_format_projects(include_archived: bool, format: OutputFormat) -> Result<String> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let projects = services::list_projects(&pool, include_archived).await?;
    let formatter = Formatter::new(format);
    Ok(formatter.format_projects(&projects))
}

/// Show or manage a project
pub async fn project(id: &str, action: Option<ProjectAction>, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Check if this is a create command
    if id == "create" {
        return Err(crate::error::GranaryError::InvalidArgument(
            "Use 'granary projects create <name>' to create a project".to_string(),
        ));
    }

    let formatter = Formatter::new(format);

    match action {
        None => {
            // Show project details
            let project = services::get_project(&pool, id).await?;
            println!("{}", formatter.format_project(&project));
        }

        Some(ProjectAction::Update {
            name,
            description,
            owner,
            tags,
        }) => {
            let parsed_tags = tags.map(|t| parse_tags(&t));

            let project = services::update_project(
                &pool,
                id,
                UpdateProject {
                    name,
                    description,
                    owner,
                    tags: parsed_tags,
                    ..Default::default()
                },
            )
            .await?;

            println!("{}", formatter.format_project(&project));
        }

        Some(ProjectAction::Archive) => {
            let project = services::archive_project(&pool, id).await?;
            println!("Archived project: {}", project.id);
        }

        Some(ProjectAction::Tasks { action }) => {
            match action {
                None => {
                    // List tasks with dependency info
                    let tasks = services::list_tasks_by_project(&pool, id).await?;
                    let tasks_with_deps = services::get_tasks_with_deps(&pool, tasks).await?;
                    println!("{}", formatter.format_tasks_with_deps(&tasks_with_deps));
                }
                Some(ProjectTasksAction::Create {
                    title,
                    description,
                    priority,
                    status,
                    owner,
                    dependencies,
                    tags,
                    due,
                }) => {
                    let priority = priority.parse().unwrap_or_default();
                    let status = status.parse().unwrap_or_default();
                    let tags = tags
                        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();

                    let task = services::create_task(
                        &pool,
                        CreateTask {
                            project_id: id.to_string(),
                            title,
                            description,
                            priority,
                            status,
                            owner,
                            tags,
                            due_at: due,
                            ..Default::default()
                        },
                    )
                    .await?;

                    // Add dependencies if specified
                    if let Some(deps) = dependencies {
                        for dep_id in deps.split(',').map(|s| s.trim()) {
                            services::add_dependency(&pool, &task.id, dep_id).await?;
                        }
                    }

                    println!("{}", formatter.format_task_created(&task));
                }
            }
        }

        Some(ProjectAction::Deps { action }) => {
            handle_deps_action(&pool, id, action, format).await?;
        }

        Some(ProjectAction::Summary) => {
            let project = services::get_project(&pool, id).await?;
            let tasks = services::list_tasks_by_project(&pool, id).await?;

            let done_count = tasks.iter().filter(|t| t.status == "done").count();
            let total = tasks.len();

            println!("## {} ({})", project.name, project.id);
            println!();
            if let Some(desc) = &project.description {
                println!("{}", desc);
                println!();
            }
            println!("Progress: {}/{} tasks done", done_count, total);
            println!();
            println!("Tasks:");
            for task in &tasks {
                let checkbox = if task.status == "done" { "[x]" } else { "[ ]" };
                println!("  {} {} ({})", checkbox, task.title, task.id);
            }
        }

        Some(ProjectAction::Ready) => {
            // Validate project has tasks
            let tasks = services::list_tasks_by_project(&pool, id).await?;

            if tasks.is_empty() {
                println!("Project has no tasks. Create tasks before marking ready.");
                return Ok(());
            }

            // Update all draft tasks to todo
            let updated_count = db::tasks::set_draft_tasks_to_todo(&pool, id).await?;

            // Get project dependencies
            let deps = db::project_dependencies::list(&pool, id).await?;

            // Get steering files for this project
            let steering_files = db::steering::list_by_scope(&pool, "project", id).await?;

            println!("Project ready: {}", id);
            println!();
            println!("Tasks: {}", tasks.len());
            println!("Dependencies configured: {}", deps.len());
            println!("Steering files: {}", steering_files.len());

            if updated_count > 0 {
                println!();
                println!("Activated {} draft task(s) (draft -> todo).", updated_count);
            }
        }

        Some(ProjectAction::Steer { action }) => {
            handle_steer_action(&pool, id, action).await?;
        }
    }

    Ok(())
}

/// Handle project dependency actions
async fn handle_deps_action(
    pool: &sqlx::SqlitePool,
    project_id: &str,
    action: ProjectDepsAction,
    format: OutputFormat,
) -> Result<()> {
    let formatter = Formatter::new(format);

    match action {
        ProjectDepsAction::Add { depends_on_id } => {
            // Verify both projects exist
            services::get_project(pool, project_id).await?;
            services::get_project(pool, &depends_on_id).await?;

            db::project_dependencies::add(pool, project_id, &depends_on_id).await?;
            println!(
                "Added dependency: {} depends on {}",
                project_id, depends_on_id
            );
        }

        ProjectDepsAction::Rm { depends_on_id } => {
            let removed =
                db::project_dependencies::remove(pool, project_id, &depends_on_id).await?;
            if removed {
                println!(
                    "Removed dependency: {} no longer depends on {}",
                    project_id, depends_on_id
                );
            } else {
                println!(
                    "No dependency found between {} and {}",
                    project_id, depends_on_id
                );
            }
        }

        ProjectDepsAction::List => {
            let deps = db::project_dependencies::list(pool, project_id).await?;
            if deps.is_empty() {
                println!("No dependencies for project {}", project_id);
            } else {
                println!("Dependencies for {}:", project_id);
                println!("{}", formatter.format_projects(&deps));
            }
        }

        ProjectDepsAction::Graph => {
            print_dependency_graph(pool, project_id).await?;
        }
    }

    Ok(())
}

/// Print a dependency graph showing both what this project depends on and what depends on it
async fn print_dependency_graph(pool: &sqlx::SqlitePool, project_id: &str) -> Result<()> {
    let project = services::get_project(pool, project_id).await?;
    let deps = db::project_dependencies::list(pool, project_id).await?;
    let dependents = db::project_dependencies::list_dependents(pool, project_id).await?;
    let unmet = db::project_dependencies::get_unmet(pool, project_id).await?;

    println!("Dependency graph for: {} ({})", project.name, project_id);
    println!();

    if !deps.is_empty() {
        println!("This project depends on:");
        for dep in &deps {
            let status_marker = if unmet.iter().any(|u| u.id == dep.id) {
                " [UNMET - has incomplete tasks]"
            } else {
                " [OK]"
            };
            println!("  -> {} ({}){}", dep.name, dep.id, status_marker);
        }
        println!();
    } else {
        println!("This project has no dependencies.");
        println!();
    }

    if !dependents.is_empty() {
        println!("Projects that depend on this:");
        for dep in &dependents {
            println!("  <- {} ({})", dep.name, dep.id);
        }
        println!();
    } else {
        println!("No projects depend on this project.");
        println!();
    }

    Ok(())
}

/// Create a new project (called from main when id == "create")
pub async fn create_project(
    name: &str,
    description: Option<String>,
    owner: Option<String>,
    tags: Option<String>,
    format: OutputFormat,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let tags = tags
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let project = services::create_project(
        &pool,
        CreateProject {
            name: name.to_string(),
            description,
            owner,
            tags,
            ..Default::default()
        },
    )
    .await?;

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_project(&project));

    Ok(())
}

fn parse_tags(tags_str: &str) -> Vec<String> {
    tags_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Handle project steering actions
async fn handle_steer_action(
    pool: &sqlx::SqlitePool,
    project_id: &str,
    action: ProjectSteerAction,
) -> Result<()> {
    match action {
        ProjectSteerAction::Add { path, mode } => {
            db::steering::add(pool, &path, &mode, Some("project"), Some(project_id)).await?;
            println!("Added steering file: {} [project: {}]", path, project_id);
        }

        ProjectSteerAction::Rm { path } => {
            let removed =
                db::steering::remove(pool, &path, Some("project"), Some(project_id)).await?;
            if removed {
                println!("Removed steering file: {} [project: {}]", path, project_id);
            } else {
                println!("Steering file not found: {}", path);
            }
        }

        ProjectSteerAction::List => {
            let files = db::steering::list_by_scope(pool, "project", project_id).await?;
            if files.is_empty() {
                println!("No steering files for project {}", project_id);
            } else {
                println!("Steering files for {}:", project_id);
                for file in files {
                    println!("  {} [{}]", file.path, file.mode);
                }
            }
        }
    }

    Ok(())
}
