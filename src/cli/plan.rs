use granary_types::{CreateProject, Project, Task};

use crate::db;
use crate::error::Result;
use crate::services::{self, Workspace};

/// Handle the plan command - creates a project and outputs guidance for task creation
/// Note: clap ensures exactly one of `name` or `existing_project` is provided
pub async fn plan(name: Option<&str>, existing_project: Option<String>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    if let Some(project_id) = existing_project {
        // Existing project mode - for sub-agents planning initiative projects
        let project = services::get_project(&pool, &project_id).await?;
        let tasks = services::list_tasks_by_project(&pool, &project_id).await?;
        print_existing_project_guidance(&project, &tasks);
    } else if let Some(name) = name {
        // New project mode - creates project and guides task creation
        let project = services::create_project(
            &pool,
            CreateProject {
                name: name.to_string(),
                description: None,
                owner: None,
                tags: vec![],
                ..Default::default()
            },
        )
        .await?;

        // Search for prior art - similar/related projects
        let prior_art = find_prior_art(&pool, name).await?;

        // Output the guidance
        print_planning_guidance(&project, &prior_art);
    }

    Ok(())
}

/// Find prior art - projects with similar names or keywords
async fn find_prior_art(pool: &sqlx::SqlitePool, query: &str) -> Result<Vec<ProjectWithProgress>> {
    // Search for similar projects
    let search_results = db::search::search_projects(pool, query).await?;

    let mut prior_art = Vec::new();
    for project in search_results.into_iter().take(5) {
        // Get task counts for each project
        let tasks = services::list_tasks_by_project(pool, &project.id).await?;
        let done_count = tasks.iter().filter(|t| t.status == "done").count();
        let total_count = tasks.len();

        prior_art.push(ProjectWithProgress {
            project,
            done_count,
            total_count,
        });
    }

    Ok(prior_art)
}

struct ProjectWithProgress {
    project: Project,
    done_count: usize,
    total_count: usize,
}

fn print_planning_guidance(project: &Project, prior_art: &[ProjectWithProgress]) {
    println!("Project created: {}", project.id);
    println!();

    // Prior Art section
    println!("## Prior Art");
    println!();
    if prior_art.is_empty() {
        println!("No similar projects found.");
    } else {
        for p in prior_art {
            let status = if p.done_count == p.total_count && p.total_count > 0 {
                "(completed)".to_string()
            } else if p.total_count > 0 {
                format!("({}/{} tasks done)", p.done_count, p.total_count)
            } else {
                "(no tasks)".to_string()
            };
            println!("- {}: {} {}", p.project.id, p.project.name, status);
        }
    }
    println!();
    println!("View details:");
    println!("  granary show <project-id>");
    println!();

    // Research section
    println!("## Research");
    println!();
    println!("Before creating tasks, research the codebase:");
    println!("- Find all files that need modification (exact paths, line numbers)");
    println!("- Document existing patterns to follow");
    println!("- Identify test patterns to replicate");
    println!();

    // Create Tasks section
    println!("## Create Tasks");
    println!();
    println!("Task descriptions are the ONLY context workers receive.");
    println!();
    println!(
        r#"  granary project {} tasks create "Task title" --priority P1 --description "
  **Goal:** What this accomplishes

  **Files to modify:**
  - path/to/file.rs:10-20 (what to change)

  **Pattern:**
  \`\`\`rust
  // code example from existing similar code
  \`\`\`

  **Acceptance criteria:**
  - [ ] Criterion 1
  ""#,
        project.id
    );
    println!();

    // Set Dependencies section
    println!("## Set Dependencies");
    println!();
    println!("  granary task <task-id> deps add <other-task-id>");
    println!();

    // Attach Steering Files section
    println!("## Attach Steering Files");
    println!();
    println!("  granary steering add <path> --project {}", project.id);
    println!();

    // Finish section
    println!("## Finish");
    println!();
    println!("  granary project {} ready", project.id);
}

/// Print guidance for planning an existing project (sub-agent mode for initiatives)
fn print_existing_project_guidance(project: &Project, tasks: &[Task]) {
    println!("# Project: {}", project.name);
    println!();
    println!("ID: {}", project.id);

    // Show description if present
    if let Some(ref desc) = project.description {
        println!();
        println!("## Description");
        println!();
        println!("{}", desc);
    }

    // Show existing tasks if any (don't mention if none)
    if !tasks.is_empty() {
        println!();
        println!("## Existing Tasks");
        println!();
        for task in tasks {
            let status_indicator = match task.status.as_str() {
                "done" => "[x]",
                "in_progress" => "[~]",
                "blocked" => "[!]",
                _ => "[ ]",
            };
            println!(
                "- {} {} ({}) - {}",
                status_indicator, task.id, task.priority, task.title
            );
        }
    }

    println!();
    println!("## Research");
    println!();
    println!("Before creating tasks, research the codebase:");
    println!("- Find all files that need modification (exact paths, line numbers)");
    println!("- Document existing patterns to follow");
    println!("- Identify test patterns to replicate");
    println!();

    // Create Tasks section
    println!("## Create Tasks");
    println!();
    println!("Task descriptions are the ONLY context workers receive.");
    println!();
    println!(
        r#"  granary project {} tasks create "Task title" --priority P1 --description "
  **Goal:** What this accomplishes

  **Files to modify:**
  - path/to/file.rs:10-20 (what to change)

  **Pattern:**
  \`\`\`rust
  // code example from existing similar code
  \`\`\`

  **Acceptance criteria:**
  - [ ] Criterion 1
  ""#,
        project.id
    );
    println!();

    // Set Dependencies section
    println!("## Set Dependencies");
    println!();
    println!("  granary task <task-id> deps add <other-task-id>");
    println!();

    // Attach Steering Files section
    println!("## Attach Steering Files");
    println!();
    println!("  granary steering add <path> --project {}", project.id);
    println!();

    // Finish section
    println!("## Finish");
    println!();
    println!("  granary project {} ready", project.id);
}
