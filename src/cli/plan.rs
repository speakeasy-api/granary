use granary_types::{CreateProject, Project, Task};
use serde::Serialize;

use crate::cli::args::CliOutputFormat;
use crate::db;
use crate::error::Result;
use crate::output::{Output, OutputType};
use crate::services::{self, Workspace};

/// Output for a new project plan with prior art
pub struct PlanOutput {
    pub project: Project,
    pub prior_art: Vec<ProjectWithProgress>,
}

/// Output for planning an existing project (sub-agent mode)
pub struct ExistingPlanOutput {
    pub project: Project,
    pub tasks: Vec<Task>,
}

impl Output for PlanOutput {
    fn output_type() -> OutputType {
        OutputType::Prompt // LLM-first commands default to Prompt
    }

    fn to_json(&self) -> String {
        let json_output = PlanJsonOutput {
            project_id: &self.project.id,
            project_name: &self.project.name,
            prior_art: self
                .prior_art
                .iter()
                .map(|p| PriorArtJson {
                    id: &p.project.id,
                    name: &p.project.name,
                    description: p.project.description.as_deref(),
                    done_count: p.done_count,
                    total_count: p.total_count,
                })
                .collect(),
        };
        serde_json::to_string_pretty(&json_output).unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        format_planning_guidance(&self.project, &self.prior_art)
    }

    fn to_text(&self) -> String {
        format!("Created project: {}", self.project.id)
    }
}

impl Output for ExistingPlanOutput {
    fn output_type() -> OutputType {
        OutputType::Prompt // LLM-first commands default to Prompt
    }

    fn to_json(&self) -> String {
        let json_output = ExistingPlanJsonOutput {
            project_id: &self.project.id,
            project_name: &self.project.name,
            description: self.project.description.as_deref(),
            tasks: self
                .tasks
                .iter()
                .map(|t| TaskSummaryJson {
                    id: &t.id,
                    title: &t.title,
                    status: &t.status,
                    priority: &t.priority,
                })
                .collect(),
        };
        serde_json::to_string_pretty(&json_output).unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        format_existing_project_guidance(&self.project, &self.tasks)
    }

    fn to_text(&self) -> String {
        format!("Project: {} ({})", self.project.name, self.project.id)
    }
}

#[derive(Serialize)]
struct PlanJsonOutput<'a> {
    project_id: &'a str,
    project_name: &'a str,
    prior_art: Vec<PriorArtJson<'a>>,
}

#[derive(Serialize)]
struct PriorArtJson<'a> {
    id: &'a str,
    name: &'a str,
    description: Option<&'a str>,
    done_count: usize,
    total_count: usize,
}

#[derive(Serialize)]
struct ExistingPlanJsonOutput<'a> {
    project_id: &'a str,
    project_name: &'a str,
    description: Option<&'a str>,
    tasks: Vec<TaskSummaryJson<'a>>,
}

#[derive(Serialize)]
struct TaskSummaryJson<'a> {
    id: &'a str,
    title: &'a str,
    status: &'a str,
    priority: &'a str,
}

/// Handle the plan command - creates a project and outputs guidance for task creation
/// Note: clap ensures exactly one of `name` or `existing_project` is provided
pub async fn plan(
    name: Option<&str>,
    existing_project: Option<String>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    if let Some(project_id) = existing_project {
        // Existing project mode - for sub-agents planning initiative projects
        let project = services::get_project(&pool, &project_id).await?;
        let tasks = services::list_tasks_by_project(&pool, &project_id).await?;
        let output = ExistingPlanOutput { project, tasks };
        println!("{}", output.format(cli_format));
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

        // Search for prior art - similar/related projects (exclude the just-created project)
        let prior_art = find_prior_art(&pool, name, &project.id).await?;

        // Output the guidance
        let output = PlanOutput { project, prior_art };
        println!("{}", output.format(cli_format));
    }

    Ok(())
}

/// Find prior art - projects with similar names or keywords, excluding the current project
async fn find_prior_art(
    pool: &sqlx::SqlitePool,
    query: &str,
    exclude_id: &str,
) -> Result<Vec<ProjectWithProgress>> {
    // Search for similar projects
    let search_results = db::search::search_projects(pool, query).await?;

    let mut prior_art = Vec::new();
    for project in search_results
        .into_iter()
        .filter(|p| p.id != exclude_id)
        .take(5)
    {
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

pub struct ProjectWithProgress {
    pub project: Project,
    pub done_count: usize,
    pub total_count: usize,
}

fn format_planning_guidance(project: &Project, prior_art: &[ProjectWithProgress]) -> String {
    let mut output = String::new();

    output.push_str(&format!("Project created: {}\n\n", project.id));

    // Prior Art section
    output.push_str("## Prior Art\n\n");
    if prior_art.is_empty() {
        output.push_str("No similar projects found.\n");
    } else {
        for p in prior_art {
            let status = if p.done_count == p.total_count && p.total_count > 0 {
                "(completed)".to_string()
            } else if p.total_count > 0 {
                format!("({}/{} tasks done)", p.done_count, p.total_count)
            } else {
                "(no tasks)".to_string()
            };
            output.push_str(&format!(
                "- {}: {} {}\n",
                p.project.id, p.project.name, status
            ));
        }
    }
    output.push_str("\nView details:\n");
    output.push_str("  granary show <project-id>\n\n");

    // Research section
    output.push_str("## Research\n\n");
    output.push_str("Before creating tasks, research the codebase:\n");
    output.push_str("- Find all files that need modification (exact paths, line numbers)\n");
    output.push_str("- Document existing patterns to follow\n");
    output.push_str("- Identify test patterns to replicate\n\n");

    // Create Tasks section
    output.push_str("## Create Tasks\n\n");
    output.push_str("Task descriptions are the ONLY context workers receive.\n\n");
    output.push_str(&format!(
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
  "
"#,
        project.id
    ));

    // Set Dependencies section
    output.push_str("## Set Dependencies\n\n");
    output.push_str("  granary task <task-id> deps add <other-task-id>\n\n");

    // Attach Steering Files section
    output.push_str("## Attach Steering Files\n\n");
    output.push_str(&format!(
        "  granary steering add <path> --project {}\n\n",
        project.id
    ));

    // Finish section
    output.push_str("## Finish\n\n");
    output.push_str(&format!("  granary project {} ready", project.id));

    output
}

/// Format guidance for planning an existing project (sub-agent mode for initiatives)
fn format_existing_project_guidance(project: &Project, tasks: &[Task]) -> String {
    let mut output = String::new();

    output.push_str(&format!("# Project: {}\n\n", project.name));
    output.push_str(&format!("ID: {}\n", project.id));

    // Show description if present
    if let Some(ref desc) = project.description {
        output.push_str("\n## Description\n\n");
        output.push_str(desc);
        output.push('\n');
    }

    // Show existing tasks if any (don't mention if none)
    if !tasks.is_empty() {
        output.push_str("\n## Existing Tasks\n\n");
        for task in tasks {
            let status_indicator = match task.status.as_str() {
                "done" => "[x]",
                "in_progress" => "[~]",
                "blocked" => "[!]",
                _ => "[ ]",
            };
            output.push_str(&format!(
                "- {} {} ({}) - {}\n",
                status_indicator, task.id, task.priority, task.title
            ));
        }
    }

    output.push_str("\n## Research\n\n");
    output.push_str("Before creating tasks, research the codebase:\n");
    output.push_str("- Find all files that need modification (exact paths, line numbers)\n");
    output.push_str("- Document existing patterns to follow\n");
    output.push_str("- Identify test patterns to replicate\n\n");

    // Create Tasks section
    output.push_str("## Create Tasks\n\n");
    output.push_str("Task descriptions are the ONLY context workers receive.\n\n");
    output.push_str(&format!(
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
  "
"#,
        project.id
    ));

    // Set Dependencies section
    output.push_str("## Set Dependencies\n\n");
    output.push_str("  granary task <task-id> deps add <other-task-id>\n\n");

    // Attach Steering Files section
    output.push_str("## Attach Steering Files\n\n");
    output.push_str(&format!(
        "  granary steering add <path> --project {}\n\n",
        project.id
    ));

    // Finish section
    output.push_str("## Finish\n\n");
    output.push_str(&format!("  granary project {} ready", project.id));

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::{create_pool, run_migrations};
    use granary_types::CreateProject;
    use tempfile::tempdir;

    async fn setup_test_db() -> (sqlx::SqlitePool, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();
        (pool, temp_dir)
    }

    #[tokio::test]
    async fn test_find_prior_art_excludes_current_project() {
        let (pool, _temp) = setup_test_db().await;

        // Create three projects with similar names
        let project_a = services::create_project(
            &pool,
            CreateProject {
                name: "auth feature".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let project_b = services::create_project(
            &pool,
            CreateProject {
                name: "auth refactor".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let project_c = services::create_project(
            &pool,
            CreateProject {
                name: "auth migration".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Search for "auth" excluding project_b — should return a and c only
        let results = find_prior_art(&pool, "auth", &project_b.id).await.unwrap();

        let result_ids: Vec<&str> = results.iter().map(|r| r.project.id.as_str()).collect();
        assert!(
            !result_ids.contains(&project_b.id.as_str()),
            "prior art should not contain the excluded project"
        );
        assert!(result_ids.contains(&project_a.id.as_str()));
        assert!(result_ids.contains(&project_c.id.as_str()));
    }

    #[tokio::test]
    async fn test_find_prior_art_returns_empty_when_only_match_is_excluded() {
        let (pool, _temp) = setup_test_db().await;

        let project = services::create_project(
            &pool,
            CreateProject {
                name: "uniquexyzname".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // The only match for this query is the excluded project itself
        let results = find_prior_art(&pool, "uniquexyzname", &project.id)
            .await
            .unwrap();

        assert!(
            results.is_empty(),
            "prior art should be empty when the only match is excluded"
        );
    }

    #[tokio::test]
    async fn test_find_prior_art_with_special_characters() {
        let (pool, _temp) = setup_test_db().await;

        let project = services::create_project(
            &pool,
            CreateProject {
                name: "TypeScript SDK battery tests".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Query contains `-` which is an FTS5 NOT operator when unescaped
        let results = find_prior_art(
            &pool,
            "Fix TypeScript v2 build failures - 11 root causes from SDK battery tests",
            "nonexistent-id",
        )
        .await
        .unwrap();

        let result_ids: Vec<&str> = results.iter().map(|r| r.project.id.as_str()).collect();
        assert!(
            result_ids.contains(&project.id.as_str()),
            "search with special characters should still find matching projects"
        );
    }
}
