//! CLI handlers for initiative commands

use crate::cli::args::{CliOutputFormat, InitiativeAction};
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::db;
use crate::error::Result;
use crate::models::{CreateInitiative, Initiative, InitiativeSummary, UpdateInitiative};
use crate::output::{Output, json, prompt, table};
use crate::services::{self, Workspace};
use granary_types::{Project, Task};
use std::time::Duration;

// =============================================================================
// Output Types
// =============================================================================

/// Output for a list of initiatives
pub struct InitiativesOutput {
    pub initiatives: Vec<Initiative>,
}

impl Output for InitiativesOutput {
    fn to_json(&self) -> String {
        json::format_initiatives(&self.initiatives)
    }

    fn to_prompt(&self) -> String {
        prompt::format_initiatives(&self.initiatives)
    }

    fn to_text(&self) -> String {
        table::format_initiatives(&self.initiatives)
    }
}

/// Output for a single initiative
pub struct InitiativeOutput {
    pub initiative: Initiative,
}

impl Output for InitiativeOutput {
    fn to_json(&self) -> String {
        json::format_initiative(&self.initiative)
    }

    fn to_prompt(&self) -> String {
        prompt::format_initiative(&self.initiative)
    }

    fn to_text(&self) -> String {
        table::format_initiative(&self.initiative)
    }
}

/// Output for initiative summary
pub struct InitiativeSummaryOutput {
    pub summary: InitiativeSummary,
}

impl Output for InitiativeSummaryOutput {
    fn to_json(&self) -> String {
        json::format_initiative_summary(&self.summary)
    }

    fn to_prompt(&self) -> String {
        prompt::format_initiative_summary(&self.summary)
    }

    fn to_text(&self) -> String {
        table::format_initiative_summary(&self.summary)
    }
}

/// Output for projects in an initiative (reuses projects output)
pub struct InitiativeProjectsOutput {
    pub projects: Vec<Project>,
}

impl Output for InitiativeProjectsOutput {
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

/// Output for tasks with dependencies
pub struct InitiativeTasksOutput {
    pub tasks: Vec<(Task, Vec<String>)>,
}

impl Output for InitiativeTasksOutput {
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

/// Output for a single task with dependencies
pub struct InitiativeTaskOutput {
    pub task: Task,
    pub blocked_by: Vec<String>,
}

impl Output for InitiativeTaskOutput {
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

/// Handle initiative command (unified: list, create, or manage specific initiative)
pub async fn initiative(
    id: Option<String>,
    action: Option<InitiativeAction>,
    include_archived: bool,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    match (id, action) {
        // No ID, no action → list initiatives
        (None, None) => {
            if watch {
                let interval_duration = Duration::from_secs(interval);
                watch_loop(interval_duration, || async {
                    let output = fetch_and_format_initiatives(include_archived, cli_format).await?;
                    Ok(format!(
                        "{}\n{}",
                        watch_status_line(interval_duration),
                        output
                    ))
                })
                .await
            } else {
                let output = fetch_and_format_initiatives(include_archived, cli_format).await?;
                println!("{}", output);
                Ok(())
            }
        }

        // No ID, Create action → create initiative
        (
            None,
            Some(InitiativeAction::Create {
                name,
                description,
                owner,
                tags,
            }),
        ) => create_initiative(&name, description, owner, tags, cli_format).await,

        // No ID, other action → error (need an ID for non-create actions)
        (None, Some(_)) => Err(crate::error::GranaryError::InvalidArgument(
            "Initiative ID is required for this action".into(),
        )),

        // ID provided, no action → show initiative details
        (Some(id), None) => {
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
            let initiative = services::get_initiative_or_error(&pool, &id).await?;
            let output = InitiativeOutput { initiative };
            println!("{}", output.format(cli_format));
            Ok(())
        }

        // ID provided, Create action → error (create doesn't take an ID)
        (Some(_), Some(InitiativeAction::Create { .. })) => {
            Err(crate::error::GranaryError::InvalidArgument(
                "Cannot specify an initiative ID with the create action".into(),
            ))
        }

        // ID provided, other action → manage specific initiative
        (Some(id), Some(action)) => {
            let workspace = Workspace::find()?;
            let pool = workspace.pool().await?;
            handle_initiative_action(&pool, &id, action, cli_format).await
        }
    }
}

/// Handle a specific initiative action (requires an ID)
async fn handle_initiative_action(
    pool: &sqlx::SqlitePool,
    id: &str,
    action: InitiativeAction,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    match action {
        InitiativeAction::Create { .. } => {
            unreachable!("Create is handled before this function is called")
        }

        InitiativeAction::Update {
            name,
            description,
            owner,
            tags,
        } => {
            let parsed_tags = tags.map(|t| parse_tags(&t));

            let initiative = services::update_initiative(
                pool,
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

            let output = InitiativeOutput { initiative };
            println!("{}", output.format(cli_format));
        }

        InitiativeAction::Archive => {
            let initiative = services::archive_initiative(pool, id).await?;
            println!("Archived initiative: {}", initiative.id);
        }

        InitiativeAction::Projects => {
            let projects = services::get_initiative_projects(pool, id).await?;
            if projects.is_empty() {
                println!("No projects in initiative {}", id);
            } else {
                let output = InitiativeProjectsOutput { projects };
                println!("{}", output.format(cli_format));
            }
        }

        InitiativeAction::AddProject { project_id } => {
            services::add_project_to_initiative(pool, id, &project_id).await?;
            println!("Added project {} to initiative {}", project_id, id);
        }

        InitiativeAction::RemoveProject { project_id } => {
            let removed = services::remove_project_from_initiative(pool, id, &project_id).await?;
            if removed {
                println!("Removed project {} from initiative {}", project_id, id);
            } else {
                println!("Project {} was not in initiative {}", project_id, id);
            }
        }

        InitiativeAction::Graph => {
            print_initiative_dependency_graph(pool, id).await?;
        }

        InitiativeAction::Next { all } => {
            let tasks = services::get_next_tasks(pool, id, all).await?;

            if tasks.is_empty() {
                println!("No actionable tasks in initiative {}", id);
            } else if all {
                let tasks_with_deps = services::get_tasks_with_deps(pool, tasks).await?;
                let output = InitiativeTasksOutput {
                    tasks: tasks_with_deps,
                };
                println!("{}", output.format(cli_format));
            } else {
                let (task, blocked_by) = services::get_task_with_deps(pool, &tasks[0].id).await?;
                let output = InitiativeTaskOutput { task, blocked_by };
                println!("{}", output.format(cli_format));
            }
        }

        InitiativeAction::Summary => {
            let summary = services::generate_initiative_summary(pool, id, 5).await?;
            let output = InitiativeSummaryOutput { summary };
            println!("{}", output.format(cli_format));
        }
    }

    Ok(())
}

/// Fetch and format all initiatives as a string
async fn fetch_and_format_initiatives(
    include_archived: bool,
    cli_format: Option<CliOutputFormat>,
) -> Result<String> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let initiatives = services::list_initiatives(&pool, include_archived).await?;
    let output = InitiativesOutput { initiatives };
    Ok(output.format(cli_format))
}

/// Create a new initiative
async fn create_initiative(
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

    let output = InitiativeOutput { initiative };
    println!("{}", output.format(cli_format));

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
