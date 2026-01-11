use crate::cli::args::{ProjectAction, ProjectTasksAction, ProjectsAction};
use crate::error::Result;
use crate::models::*;
use crate::output::{Formatter, OutputFormat};
use crate::services::{self, Workspace};

/// Handle projects command (list or create)
pub async fn projects(
    action: Option<ProjectsAction>,
    include_archived: bool,
    format: OutputFormat,
) -> Result<()> {
    match action {
        None => list_projects(include_archived, format).await,
        Some(ProjectsAction::Create {
            name,
            description,
            owner,
            tags,
        }) => create_project(&name, description, owner, tags, format).await,
    }
}

/// List all projects
pub async fn list_projects(include_archived: bool, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let projects = services::list_projects(&pool, include_archived).await?;
    let formatter = Formatter::new(format);
    println!("{}", formatter.format_projects(&projects));

    Ok(())
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
                    // List tasks
                    let tasks = services::list_tasks_by_project(&pool, id).await?;
                    println!("{}", formatter.format_tasks(&tasks));
                }
                Some(ProjectTasksAction::Create {
                    title,
                    description,
                    priority,
                    owner,
                    dependencies,
                    tags,
                    due,
                }) => {
                    let priority = priority.parse().unwrap_or_default();
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

                    println!("{}", formatter.format_task(&task));
                }
            }
        }
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
