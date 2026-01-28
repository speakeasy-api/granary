//! CLI handlers for initiative commands

use crate::cli::args::{InitiativeAction, InitiativesAction};
use crate::db;
use crate::error::Result;
use crate::models::initiative::{CreateInitiative, UpdateInitiative};
use crate::output::{Formatter, OutputFormat};
use crate::services::{self, Workspace};

/// Handle initiatives command (list or create)
pub async fn initiatives(
    action: Option<InitiativesAction>,
    include_archived: bool,
    format: OutputFormat,
) -> Result<()> {
    match action {
        None => list_initiatives(include_archived, format).await,
        Some(InitiativesAction::Create {
            name,
            description,
            owner,
            tags,
        }) => create_initiative(&name, description, owner, tags, format).await,
    }
}

/// List all initiatives
pub async fn list_initiatives(include_archived: bool, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let initiatives = services::list_initiatives(&pool, include_archived).await?;
    let formatter = Formatter::new(format);
    println!("{}", formatter.format_initiatives(&initiatives));

    Ok(())
}

/// Create a new initiative
pub async fn create_initiative(
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

    let initiative = services::create_initiative(
        &pool,
        CreateInitiative {
            name: name.to_string(),
            description,
            owner,
            tags,
        },
    )
    .await?;

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_initiative(&initiative));

    Ok(())
}

/// Show or manage an initiative
pub async fn initiative(
    id: &str,
    action: Option<InitiativeAction>,
    format: OutputFormat,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let formatter = Formatter::new(format);

    match action {
        None => {
            // Show initiative details
            let initiative = services::get_initiative_or_error(&pool, id).await?;
            println!("{}", formatter.format_initiative(&initiative));
        }

        Some(InitiativeAction::Update {
            name,
            description,
            owner,
            tags,
        }) => {
            let parsed_tags = tags.map(|t| parse_tags(&t));

            let initiative = services::update_initiative(
                &pool,
                id,
                UpdateInitiative {
                    name,
                    description,
                    owner,
                    tags: parsed_tags,
                    ..Default::default()
                },
            )
            .await?;

            println!("{}", formatter.format_initiative(&initiative));
        }

        Some(InitiativeAction::Archive) => {
            let initiative = services::archive_initiative(&pool, id).await?;
            println!("Archived initiative: {}", initiative.id);
        }

        Some(InitiativeAction::Projects) => {
            // List projects in initiative
            let projects = services::get_initiative_projects(&pool, id).await?;
            if projects.is_empty() {
                println!("No projects in initiative {}", id);
            } else {
                println!("{}", formatter.format_projects(&projects));
            }
        }

        Some(InitiativeAction::AddProject { project_id }) => {
            // Add project to initiative
            services::add_project_to_initiative(&pool, id, &project_id).await?;
            println!("Added project {} to initiative {}", project_id, id);
        }

        Some(InitiativeAction::RemoveProject { project_id }) => {
            // Remove project from initiative
            let removed = services::remove_project_from_initiative(&pool, id, &project_id).await?;
            if removed {
                println!("Removed project {} from initiative {}", project_id, id);
            } else {
                println!("Project {} was not in initiative {}", project_id, id);
            }
        }

        Some(InitiativeAction::Graph) => {
            // Show dependency graph between projects in this initiative
            print_initiative_dependency_graph(&pool, id).await?;
        }

        Some(InitiativeAction::Next { all }) => {
            // Get next actionable task(s) in initiative
            let tasks = services::get_next_tasks(&pool, id, all).await?;

            if tasks.is_empty() {
                println!("No actionable tasks in initiative {}", id);
            } else if all {
                let tasks_with_deps = services::get_tasks_with_deps(&pool, tasks).await?;
                println!("{}", formatter.format_tasks_with_deps(&tasks_with_deps));
            } else {
                let (task, blocked_by) = services::get_task_with_deps(&pool, &tasks[0].id).await?;
                println!("{}", formatter.format_task_with_deps(&task, blocked_by));
            }
        }

        Some(InitiativeAction::Summary) => {
            // Generate and display initiative summary
            let summary = services::generate_initiative_summary(&pool, id, 5).await?;
            println!("{}", formatter.format_initiative_summary(&summary));
        }
    }

    Ok(())
}

/// Print a dependency graph showing relationships between projects in an initiative
/// Outputs in Mermaid flowchart format
async fn print_initiative_dependency_graph(
    pool: &sqlx::SqlitePool,
    initiative_id: &str,
) -> Result<()> {
    // Verify initiative exists and get its info
    let initiative = services::get_initiative_or_error(pool, initiative_id).await?;

    // Get all projects in the initiative
    let projects = services::get_initiative_projects(pool, initiative_id).await?;

    if projects.is_empty() {
        println!("No projects in initiative {}", initiative_id);
        return Ok(());
    }

    // Get internal dependencies (between projects within this initiative)
    let dependencies =
        db::initiative_projects::list_internal_dependencies(pool, initiative_id).await?;

    // Get unmet dependencies for each project (to show status)
    let mut unmet_projects: std::collections::HashSet<String> = std::collections::HashSet::new();
    for project in &projects {
        let unmet = db::project_dependencies::get_unmet(pool, &project.id).await?;
        if !unmet.is_empty() {
            unmet_projects.insert(project.id.clone());
        }
    }

    // Output Mermaid flowchart
    println!("flowchart TD");
    println!("    %% Initiative: {} ({})", initiative.name, initiative_id);
    println!();

    // Define nodes with styling based on status
    for project in &projects {
        let node_id = sanitize_mermaid_id(&project.id);
        let label = escape_mermaid_label(&project.name);

        // Check project status and if it has unmet dependencies
        let has_unmet = unmet_projects.contains(&project.id);
        let is_archived = project.status == "archived";

        if is_archived {
            // Archived projects get a different style
            println!("    {}[\"{}\"]:::archived", node_id, label);
        } else if has_unmet {
            // Projects with unmet dependencies
            println!("    {}[\"{}\"]:::blocked", node_id, label);
        } else {
            // Normal active projects
            println!("    {}[\"{}\"]:::active", node_id, label);
        }
    }

    println!();

    // Define edges (dependencies)
    if dependencies.is_empty() {
        println!("    %% No dependencies between projects");
    } else {
        println!("    %% Dependencies");
        for dep in &dependencies {
            let from_id = sanitize_mermaid_id(&dep.project_id);
            let to_id = sanitize_mermaid_id(&dep.depends_on_project_id);
            // Arrow from dependent to dependency (A depends on B means A --> B)
            println!("    {} --> {}", from_id, to_id);
        }
    }
    Ok(())
}

/// Sanitize a string to be a valid Mermaid node ID
/// Mermaid IDs should be alphanumeric with underscores
fn sanitize_mermaid_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Escape a string for use as a Mermaid label
/// Wraps in quotes and escapes internal quotes
fn escape_mermaid_label(label: &str) -> String {
    label.replace('\"', "'").replace('\n', " ")
}

fn parse_tags(tags_str: &str) -> Vec<String> {
    tags_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
