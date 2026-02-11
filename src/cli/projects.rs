use granary_types::{CreateProject, CreateTask, Project, Task, UpdateProject};

use crate::cli::args::{
    CliOutputFormat, ProjectAction, ProjectDepsAction, ProjectSteerAction, ProjectTasksAction,
};
use crate::cli::tasks::TaskCreatedOutput;
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::db;
use crate::error::Result;
use crate::output::{Output, json, prompt, table};
use crate::services::{self, Workspace};
use std::time::Duration;

// =============================================================================
// Output Types
// =============================================================================

/// Output for a list of projects
pub struct ProjectsOutput {
    pub projects: Vec<Project>,
}

impl Output for ProjectsOutput {
    fn to_json(&self) -> String {
        json::format_projects(&self.projects)
    }

    fn to_prompt(&self) -> String {
        prompt::format_projects(&self.projects)
    }

    fn to_text(&self) -> String {
        table::format_projects(&self.projects)
    }
}

/// Output for a single project
pub struct ProjectOutput {
    pub project: Project,
}

impl Output for ProjectOutput {
    fn to_json(&self) -> String {
        json::format_project(&self.project)
    }

    fn to_prompt(&self) -> String {
        prompt::format_project(&self.project)
    }

    fn to_text(&self) -> String {
        table::format_project(&self.project)
    }
}

/// Output for a list of tasks with dependencies (used in project tasks listing)
pub struct ProjectTasksOutput {
    pub tasks: Vec<(Task, Vec<String>)>,
}

impl Output for ProjectTasksOutput {
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

/// Handle project list command
pub async fn list(
    include_archived: bool,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    if watch {
        let interval_duration = Duration::from_secs(interval);
        watch_loop(interval_duration, || async {
            let output = fetch_and_format_projects(include_archived, cli_format)
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
        let output = fetch_and_format_projects(include_archived, cli_format).await?;
        println!("{}", output);
        Ok(())
    }
}

/// Handle project action without an ID (e.g., `granary project create "name"`)
pub async fn project_action_without_id(
    action: ProjectAction,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    match action {
        ProjectAction::Create {
            name,
            name_flag,
            description,
            owner,
            tags,
        } => {
            let resolved_name = name.or(name_flag).ok_or_else(|| {
                crate::error::GranaryError::InvalidArgument(
                    "Project name is required. Usage: granary project create <name>".to_string(),
                )
            })?;
            create_project(&resolved_name, description, owner, tags, cli_format).await
        }
        _ => Err(crate::error::GranaryError::InvalidArgument(
            "Project ID is required for this action".to_string(),
        )),
    }
}

/// Fetch and format all projects as a string
async fn fetch_and_format_projects(
    include_archived: bool,
    cli_format: Option<CliOutputFormat>,
) -> Result<String> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let projects = services::list_projects(&pool, include_archived).await?;
    let output = ProjectsOutput { projects };
    Ok(output.format(cli_format))
}

/// Show or manage a project
pub async fn project(
    id: &str,
    action: Option<ProjectAction>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    match action {
        None => {
            // Show project details
            let project = services::get_project(&pool, id).await?;
            let output = ProjectOutput { project };
            println!("{}", output.format(cli_format));
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

            let output = ProjectOutput { project };
            println!("{}", output.format(cli_format));
        }

        Some(ProjectAction::Done { complete_tasks }) => {
            let project = services::complete_project(&pool, id, complete_tasks).await?;
            if complete_tasks {
                println!("Completed project (and all tasks): {}", project.id);
            } else {
                println!("Completed project: {}", project.id);
            }
        }

        Some(ProjectAction::Archive) => {
            let project = services::archive_project(&pool, id).await?;
            println!("Archived project: {}", project.id);
        }

        Some(ProjectAction::Unarchive) => {
            let project = services::unarchive_project(&pool, id).await?;
            println!("Unarchived project: {}", project.id);
        }

        Some(ProjectAction::Tasks { action }) => {
            match action {
                None => {
                    // List tasks with dependency info
                    let tasks = services::list_tasks_by_project(&pool, id).await?;
                    let tasks_with_deps = services::get_tasks_with_deps(&pool, tasks).await?;
                    let output = ProjectTasksOutput {
                        tasks: tasks_with_deps,
                    };
                    println!("{}", output.format(cli_format));
                }
                Some(ProjectTasksAction::Create {
                    title_positional,
                    title_flag,
                    description,
                    priority,
                    status,
                    owner,
                    dependencies,
                    tags,
                    due,
                }) => {
                    let title = title_positional.or(title_flag).ok_or_else(|| {
                        crate::error::GranaryError::InvalidArgument(
                            "Task title is required. Usage: granary project <id> tasks create <title>".to_string(),
                        )
                    })?;
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

                    let output = TaskCreatedOutput { task };
                    println!("{}", output.format(cli_format));
                }
            }
        }

        Some(ProjectAction::Deps { action }) => {
            handle_deps_action(&pool, id, action, cli_format).await?;
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

        Some(ProjectAction::Create { .. }) => {
            return Err(crate::error::GranaryError::InvalidArgument(
                "Cannot create a project with an ID. Use: granary project create <name>"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

/// Handle project dependency actions
async fn handle_deps_action(
    pool: &sqlx::SqlitePool,
    project_id: &str,
    action: ProjectDepsAction,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
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
                let output = ProjectsOutput { projects: deps };
                println!("{}", output.format(cli_format));
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
    cli_format: Option<CliOutputFormat>,
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

    let output = ProjectOutput { project };
    println!("{}", output.format(cli_format));

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
