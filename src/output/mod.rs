pub mod json;
pub mod prompt;
pub mod table;

use granary_types::{Initiative, InitiativeSummary, Project, Task};

use crate::models::run::Run;
use crate::models::*;
use granary_types::worker;

/// Output format enum
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
    Md,
    Prompt,
}

impl std::str::FromStr for OutputFormat {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "table" => Ok(OutputFormat::Table),
            "json" => Ok(OutputFormat::Json),
            "yaml" => Ok(OutputFormat::Yaml),
            "md" | "markdown" => Ok(OutputFormat::Md),
            "prompt" => Ok(OutputFormat::Prompt),
            _ => Err(()),
        }
    }
}

/// Format output based on the selected format
pub struct Formatter {
    pub format: OutputFormat,
}

impl Formatter {
    pub fn new(format: OutputFormat) -> Self {
        Self { format }
    }

    pub fn format_project(&self, project: &Project) -> String {
        match self.format {
            OutputFormat::Json => json::format_project(project),
            OutputFormat::Yaml => yaml_format_project(project),
            OutputFormat::Md => md_format_project(project),
            OutputFormat::Prompt => prompt::format_project(project),
            OutputFormat::Table => table::format_project(project),
        }
    }

    pub fn format_projects(&self, projects: &[Project]) -> String {
        match self.format {
            OutputFormat::Json => json::format_projects(projects),
            OutputFormat::Yaml => yaml_format_projects(projects),
            OutputFormat::Md => md_format_projects(projects),
            OutputFormat::Prompt => prompt::format_projects(projects),
            OutputFormat::Table => table::format_projects(projects),
        }
    }

    pub fn format_task(&self, task: &Task) -> String {
        match self.format {
            OutputFormat::Json => json::format_task(task),
            OutputFormat::Yaml => yaml_format_task(task),
            OutputFormat::Md => md_format_task(task),
            OutputFormat::Prompt => prompt::format_task(task),
            OutputFormat::Table => table::format_task(task),
        }
    }

    pub fn format_task_with_deps(&self, task: &Task, blocked_by: Vec<String>) -> String {
        match self.format {
            OutputFormat::Json => json::format_task_with_deps(task, blocked_by),
            OutputFormat::Yaml => yaml_format_task_with_deps(task, &blocked_by),
            OutputFormat::Md => md_format_task_with_deps(task, &blocked_by),
            OutputFormat::Prompt => prompt::format_task_with_deps(task, &blocked_by),
            OutputFormat::Table => table::format_task_with_deps(task, &blocked_by),
        }
    }

    pub fn format_tasks(&self, tasks: &[Task]) -> String {
        match self.format {
            OutputFormat::Json => json::format_tasks(tasks),
            OutputFormat::Yaml => yaml_format_tasks(tasks),
            OutputFormat::Md => md_format_tasks(tasks),
            OutputFormat::Prompt => prompt::format_tasks(tasks),
            OutputFormat::Table => table::format_tasks(tasks),
        }
    }

    pub fn format_tasks_with_deps(&self, tasks_with_deps: &[(Task, Vec<String>)]) -> String {
        match self.format {
            OutputFormat::Json => json::format_tasks_with_deps(tasks_with_deps),
            OutputFormat::Yaml => yaml_format_tasks_with_deps(tasks_with_deps),
            OutputFormat::Md => md_format_tasks_with_deps(tasks_with_deps),
            OutputFormat::Prompt => {
                let refs: Vec<(&Task, &[String])> = tasks_with_deps
                    .iter()
                    .map(|(t, d)| (t, d.as_slice()))
                    .collect();
                prompt::format_tasks_with_deps(&refs)
            }
            OutputFormat::Table => table::format_tasks_with_deps(tasks_with_deps),
        }
    }

    pub fn format_comment(&self, comment: &Comment) -> String {
        match self.format {
            OutputFormat::Json => json::format_comment(comment),
            OutputFormat::Yaml => yaml_format_comment(comment),
            OutputFormat::Md => md_format_comment(comment),
            OutputFormat::Prompt => prompt::format_comment(comment),
            OutputFormat::Table => table::format_comment(comment),
        }
    }

    pub fn format_comments(&self, comments: &[Comment]) -> String {
        match self.format {
            OutputFormat::Json => json::format_comments(comments),
            OutputFormat::Yaml => yaml_format_comments(comments),
            OutputFormat::Md => md_format_comments(comments),
            OutputFormat::Prompt => prompt::format_comments(comments),
            OutputFormat::Table => table::format_comments(comments),
        }
    }

    pub fn format_session(&self, session: &Session) -> String {
        match self.format {
            OutputFormat::Json => json::format_session(session),
            OutputFormat::Yaml => yaml_format_session(session),
            OutputFormat::Md => md_format_session(session),
            OutputFormat::Prompt => prompt::format_session(session),
            OutputFormat::Table => table::format_session(session),
        }
    }

    pub fn format_sessions(&self, sessions: &[Session]) -> String {
        match self.format {
            OutputFormat::Json => json::format_sessions(sessions),
            OutputFormat::Yaml => yaml_format_sessions(sessions),
            OutputFormat::Md => md_format_sessions(sessions),
            OutputFormat::Prompt => prompt::format_sessions(sessions),
            OutputFormat::Table => table::format_sessions(sessions),
        }
    }

    pub fn format_checkpoint(&self, checkpoint: &Checkpoint) -> String {
        match self.format {
            OutputFormat::Json => json::format_checkpoint(checkpoint),
            OutputFormat::Yaml => yaml_format_checkpoint(checkpoint),
            OutputFormat::Md => md_format_checkpoint(checkpoint),
            OutputFormat::Prompt => prompt::format_checkpoint(checkpoint),
            OutputFormat::Table => table::format_checkpoint(checkpoint),
        }
    }

    pub fn format_checkpoints(&self, checkpoints: &[Checkpoint]) -> String {
        match self.format {
            OutputFormat::Json => json::format_checkpoints(checkpoints),
            OutputFormat::Yaml => yaml_format_checkpoints(checkpoints),
            OutputFormat::Md => md_format_checkpoints(checkpoints),
            OutputFormat::Prompt => prompt::format_checkpoints(checkpoints),
            OutputFormat::Table => table::format_checkpoints(checkpoints),
        }
    }

    pub fn format_artifact(&self, artifact: &Artifact) -> String {
        match self.format {
            OutputFormat::Json => json::format_artifact(artifact),
            OutputFormat::Yaml => yaml_format_artifact(artifact),
            _ => table::format_artifact(artifact),
        }
    }

    pub fn format_artifacts(&self, artifacts: &[Artifact]) -> String {
        match self.format {
            OutputFormat::Json => json::format_artifacts(artifacts),
            OutputFormat::Yaml => yaml_format_artifacts(artifacts),
            _ => table::format_artifacts(artifacts),
        }
    }

    pub fn format_next_task(&self, task: Option<&Task>, reason: Option<&str>) -> String {
        match self.format {
            OutputFormat::Json => json::format_next_task(task, reason),
            OutputFormat::Prompt => prompt::format_next_task(task, reason),
            _ => table::format_next_task(task, reason),
        }
    }

    pub fn format_search_results(&self, results: &[SearchResult]) -> String {
        match self.format {
            OutputFormat::Json => json::format_search_results(results),
            OutputFormat::Yaml => yaml_format_search_results(results),
            OutputFormat::Md => md_format_search_results(results),
            OutputFormat::Prompt => prompt::format_search_results(results),
            OutputFormat::Table => table::format_search_results(results),
        }
    }

    pub fn format_initiative(&self, initiative: &Initiative) -> String {
        match self.format {
            OutputFormat::Json => json::format_initiative(initiative),
            OutputFormat::Yaml => yaml_format_initiative(initiative),
            OutputFormat::Md => md_format_initiative(initiative),
            OutputFormat::Prompt => prompt::format_initiative(initiative),
            OutputFormat::Table => table::format_initiative(initiative),
        }
    }

    pub fn format_initiatives(&self, initiatives: &[Initiative]) -> String {
        match self.format {
            OutputFormat::Json => json::format_initiatives(initiatives),
            OutputFormat::Yaml => yaml_format_initiatives(initiatives),
            OutputFormat::Md => md_format_initiatives(initiatives),
            OutputFormat::Prompt => prompt::format_initiatives(initiatives),
            OutputFormat::Table => table::format_initiatives(initiatives),
        }
    }

    pub fn format_initiative_summary(&self, summary: &InitiativeSummary) -> String {
        match self.format {
            OutputFormat::Json => json::format_initiative_summary(summary),
            OutputFormat::Yaml => yaml_format_initiative_summary(summary),
            OutputFormat::Md => md_format_initiative_summary(summary),
            OutputFormat::Prompt => prompt::format_initiative_summary(summary),
            OutputFormat::Table => table::format_initiative_summary(summary),
        }
    }

    pub fn format_worker(&self, worker: &worker::Worker) -> String {
        match self.format {
            OutputFormat::Json => json::format_worker(worker),
            OutputFormat::Yaml => yaml_format_worker(worker),
            _ => table::format_worker(worker),
        }
    }

    pub fn format_workers(&self, workers: &[worker::Worker]) -> String {
        match self.format {
            OutputFormat::Json => json::format_workers(workers),
            OutputFormat::Yaml => yaml_format_workers(workers),
            _ => table::format_workers(workers),
        }
    }

    pub fn format_run(&self, run: &Run) -> String {
        match self.format {
            OutputFormat::Json => json::format_run(run),
            OutputFormat::Yaml => yaml_format_run(run),
            _ => table::format_run(run),
        }
    }

    pub fn format_runs(&self, runs: &[Run]) -> String {
        match self.format {
            OutputFormat::Json => json::format_runs(runs),
            OutputFormat::Yaml => yaml_format_runs(runs),
            _ => table::format_runs(runs),
        }
    }

    /// Format task creation confirmation
    /// For table/text formats: single line "Task created: <task-id>"
    /// For JSON: full task object for scripting compatibility
    pub fn format_task_created(&self, task: &Task) -> String {
        match self.format {
            OutputFormat::Json => json::format_task(task),
            OutputFormat::Yaml => yaml_format_task(task),
            _ => format!("Task created: {}", task.id),
        }
    }
}

// YAML formatters (using serde_yaml)
fn yaml_format_project(project: &Project) -> String {
    serde_yaml::to_string(project).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_projects(projects: &[Project]) -> String {
    serde_yaml::to_string(projects).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_task(task: &Task) -> String {
    let output = json::TaskOutput::from_task(task.clone());
    serde_yaml::to_string(&output).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_task_with_deps(task: &Task, blocked_by: &[String]) -> String {
    let output = json::TaskOutput::new(task.clone(), blocked_by.to_vec());
    serde_yaml::to_string(&output).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_tasks(tasks: &[Task]) -> String {
    let outputs: Vec<json::TaskOutput> = tasks
        .iter()
        .map(|t| json::TaskOutput::from_task(t.clone()))
        .collect();
    serde_yaml::to_string(&outputs).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_tasks_with_deps(tasks_with_deps: &[(Task, Vec<String>)]) -> String {
    let outputs: Vec<json::TaskOutput> = tasks_with_deps
        .iter()
        .map(|(t, deps)| json::TaskOutput::new(t.clone(), deps.clone()))
        .collect();
    serde_yaml::to_string(&outputs).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_comment(comment: &Comment) -> String {
    serde_yaml::to_string(comment).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_comments(comments: &[Comment]) -> String {
    serde_yaml::to_string(comments).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_session(session: &Session) -> String {
    serde_yaml::to_string(session).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_sessions(sessions: &[Session]) -> String {
    serde_yaml::to_string(sessions).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_checkpoint(checkpoint: &Checkpoint) -> String {
    serde_yaml::to_string(checkpoint).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_checkpoints(checkpoints: &[Checkpoint]) -> String {
    serde_yaml::to_string(checkpoints).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_artifact(artifact: &Artifact) -> String {
    serde_yaml::to_string(artifact).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_artifacts(artifacts: &[Artifact]) -> String {
    serde_yaml::to_string(artifacts).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

// Markdown formatters
fn md_format_project(project: &Project) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", project.name));
    md.push_str(&format!("**ID:** `{}`\n", project.id));
    md.push_str(&format!("**Status:** {}\n", project.status));
    if let Some(owner) = &project.owner {
        md.push_str(&format!("**Owner:** {}\n", owner));
    }
    if let Some(desc) = &project.description {
        md.push_str(&format!("\n{}\n", desc));
    }
    let tags = project.tags_vec();
    if !tags.is_empty() {
        md.push_str(&format!("\n**Tags:** {}\n", tags.join(", ")));
    }
    md
}

fn md_format_projects(projects: &[Project]) -> String {
    let mut md = String::from("# Projects\n\n");
    for project in projects {
        md.push_str(&format!(
            "- **{}** (`{}`) - {}\n",
            project.name, project.id, project.status
        ));
    }
    md
}

fn md_format_task(task: &Task) -> String {
    md_format_task_with_deps(task, &[])
}

fn md_format_task_with_deps(task: &Task, blocked_by: &[String]) -> String {
    let mut md = String::new();
    let checkbox = if task.status == "done" { "[x]" } else { "[ ]" };
    md.push_str(&format!("## {} {}\n\n", checkbox, task.title));
    md.push_str(&format!("**ID:** `{}`\n", task.id));
    md.push_str(&format!(
        "**Status:** {} | **Priority:** {}\n",
        task.status, task.priority
    ));
    if let Some(owner) = &task.owner {
        md.push_str(&format!("**Owner:** {}\n", owner));
    }
    if let Some(desc) = &task.description {
        md.push_str(&format!("\n{}\n", desc));
    }
    if let Some(reason) = &task.blocked_reason {
        md.push_str(&format!("\n**Blocked:** {}\n", reason));
    }
    if !blocked_by.is_empty() {
        md.push_str(&format!("\n**Blocked by:** {}\n", blocked_by.join(", ")));
    }
    md
}

fn md_format_tasks(tasks: &[Task]) -> String {
    let tasks_with_deps: Vec<(&Task, &[String])> = tasks.iter().map(|t| (t, &[][..])).collect();
    md_format_tasks_internal(&tasks_with_deps)
}

fn md_format_tasks_with_deps(tasks_with_deps: &[(Task, Vec<String>)]) -> String {
    let refs: Vec<(&Task, &[String])> = tasks_with_deps
        .iter()
        .map(|(t, d)| (t, d.as_slice()))
        .collect();
    md_format_tasks_internal(&refs)
}

fn md_format_tasks_internal(tasks_with_deps: &[(&Task, &[String])]) -> String {
    let mut md = String::from("# Tasks\n\n");
    for (task, blocked_by) in tasks_with_deps {
        let checkbox = if task.status == "done" { "[x]" } else { "[ ]" };
        let blocked = if task.blocked_reason.is_some() || !blocked_by.is_empty() {
            " (blocked)"
        } else {
            ""
        };
        md.push_str(&format!(
            "- {} **{}** `{}` [{}]{}",
            checkbox, task.title, task.id, task.priority, blocked
        ));
        if let Some(owner) = &task.owner {
            md.push_str(&format!(" @{}", owner));
        }
        if !blocked_by.is_empty() {
            md.push_str(&format!(" blocked_by: {}", blocked_by.join(", ")));
        }
        md.push('\n');
    }
    md
}

fn md_format_comment(comment: &Comment) -> String {
    let mut md = String::new();
    md.push_str(&format!("### {} ({})\n\n", comment.kind, comment.id));
    if let Some(author) = &comment.author {
        md.push_str(&format!("**Author:** {} | ", author));
    }
    md.push_str(&format!("**Created:** {}\n\n", comment.created_at));
    md.push_str(&comment.content);
    md.push('\n');
    md
}

fn md_format_comments(comments: &[Comment]) -> String {
    let mut md = String::from("# Comments\n\n");
    for comment in comments {
        let author = comment.author.as_deref().unwrap_or("anonymous");
        md.push_str(&format!(
            "- **[{}]** {} - _{}_\n",
            comment.kind, comment.content, author
        ));
    }
    md
}

fn md_format_session(session: &Session) -> String {
    let mut md = String::new();
    let name = session.name.as_deref().unwrap_or("Unnamed Session");
    md.push_str(&format!("# Session: {}\n\n", name));
    md.push_str(&format!("**ID:** `{}`\n", session.id));
    if let Some(mode) = &session.mode {
        md.push_str(&format!("**Mode:** {}\n", mode));
    }
    if let Some(owner) = &session.owner {
        md.push_str(&format!("**Owner:** {}\n", owner));
    }
    let status = if session.is_closed() {
        "Closed"
    } else {
        "Active"
    };
    md.push_str(&format!("**Status:** {}\n", status));
    if let Some(focus) = &session.focus_task_id {
        md.push_str(&format!("**Focus Task:** `{}`\n", focus));
    }
    md
}

fn md_format_sessions(sessions: &[Session]) -> String {
    let mut md = String::from("# Sessions\n\n");
    for session in sessions {
        let name = session.name.as_deref().unwrap_or("Unnamed");
        let status = if session.is_closed() {
            "closed"
        } else {
            "active"
        };
        md.push_str(&format!("- **{}** (`{}`) - {}\n", name, session.id, status));
    }
    md
}

fn md_format_checkpoint(checkpoint: &Checkpoint) -> String {
    format!(
        "## Checkpoint: {}\n\n**ID:** `{}`\n**Session:** `{}`\n**Created:** {}\n",
        checkpoint.name, checkpoint.id, checkpoint.session_id, checkpoint.created_at
    )
}

fn md_format_checkpoints(checkpoints: &[Checkpoint]) -> String {
    let mut md = String::from("# Checkpoints\n\n");
    for cp in checkpoints {
        md.push_str(&format!(
            "- **{}** (`{}`) - {}\n",
            cp.name, cp.id, cp.created_at
        ));
    }
    md
}

fn yaml_format_search_results(results: &[SearchResult]) -> String {
    serde_yaml::to_string(results).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_initiative(initiative: &Initiative) -> String {
    serde_yaml::to_string(initiative).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_initiatives(initiatives: &[Initiative]) -> String {
    serde_yaml::to_string(initiatives).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn md_format_search_results(results: &[SearchResult]) -> String {
    let mut md = String::from("# Search Results\n\n");
    for result in results {
        match result {
            SearchResult::Initiative {
                id,
                name,
                description,
                status,
            } => {
                md.push_str(&format!("- **[INITIATIVE]** {} (`{}`)", name, id));
                if let Some(desc) = description {
                    md.push_str(&format!(" - {}", desc));
                }
                md.push_str(&format!(" [{}]\n", status));
            }
            SearchResult::Project {
                id,
                name,
                description,
                status,
            } => {
                md.push_str(&format!("- **[PROJECT]** {} (`{}`)", name, id));
                if let Some(desc) = description {
                    md.push_str(&format!(" - {}", desc));
                }
                md.push_str(&format!(" [{}]\n", status));
            }
            SearchResult::Task {
                id,
                title,
                description,
                status,
                priority,
                project_id,
            } => {
                md.push_str(&format!("- **[TASK]** {} (`{}`) [{}]", title, id, priority));
                if let Some(desc) = description {
                    md.push_str(&format!(" - {}", desc));
                }
                md.push_str(&format!(" - {} (project: {})\n", status, project_id));
            }
        }
    }
    md
}

fn md_format_initiative(initiative: &Initiative) -> String {
    let mut md = String::new();
    md.push_str(&format!("# {}\n\n", initiative.name));
    md.push_str(&format!("**ID:** `{}`\n", initiative.id));
    md.push_str(&format!("**Status:** {}\n", initiative.status));
    if let Some(owner) = &initiative.owner {
        md.push_str(&format!("**Owner:** {}\n", owner));
    }
    if let Some(desc) = &initiative.description {
        md.push_str(&format!("\n{}\n", desc));
    }
    let tags = initiative.tags_vec();
    if !tags.is_empty() {
        md.push_str(&format!("\n**Tags:** {}\n", tags.join(", ")));
    }
    md
}

fn md_format_initiatives(initiatives: &[Initiative]) -> String {
    let mut md = String::from("# Initiatives\n\n");
    for initiative in initiatives {
        md.push_str(&format!(
            "- **{}** (`{}`) - {}\n",
            initiative.name, initiative.id, initiative.status
        ));
    }
    md
}

fn yaml_format_initiative_summary(summary: &InitiativeSummary) -> String {
    serde_yaml::to_string(summary).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn md_format_initiative_summary(summary: &InitiativeSummary) -> String {
    let mut md = String::new();

    // Header
    md.push_str(&format!(
        "# Initiative Summary: {}\n\n",
        summary.initiative.name
    ));
    md.push_str(&format!("**ID:** `{}`\n", summary.initiative.id));
    if let Some(desc) = &summary.initiative.description {
        md.push_str(&format!("**Description:** {}\n", desc));
    }
    md.push('\n');

    // Progress
    md.push_str(&format!(
        "## Progress: {:.1}%\n\n",
        summary.status.percent_complete
    ));
    md.push_str(&format!(
        "- **Projects:** {} total, {} complete, {} blocked\n",
        summary.status.total_projects,
        summary.status.completed_projects,
        summary.status.blocked_projects
    ));
    md.push_str(&format!(
        "- **Tasks:** {}/{} complete ({} in progress, {} todo, {} blocked)\n\n",
        summary.status.tasks_done,
        summary.status.total_tasks,
        summary.status.tasks_in_progress,
        summary.status.tasks_todo,
        summary.status.tasks_blocked
    ));

    // Projects breakdown
    if !summary.projects.is_empty() {
        md.push_str("## Projects\n\n");
        for proj in &summary.projects {
            let status = if proj.done_count == proj.task_count && proj.task_count > 0 {
                "[x]"
            } else if proj.blocked {
                "[ ] (blocked)"
            } else {
                "[ ]"
            };
            md.push_str(&format!(
                "- {} **{}** ({}/{} tasks)\n",
                status, proj.name, proj.done_count, proj.task_count
            ));
        }
        md.push('\n');
    }

    // Blockers
    if !summary.blockers.is_empty() {
        md.push_str("## Blockers\n\n");
        for b in &summary.blockers {
            md.push_str(&format!(
                "- **[{}]** {}: {}\n",
                b.blocker_type, b.project_name, b.description
            ));
        }
        md.push('\n');
    }

    // Next actions
    if !summary.next_actions.is_empty() {
        md.push_str("## Next Actions\n\n");
        for a in &summary.next_actions {
            md.push_str(&format!(
                "- `[{}]` {} > {}\n",
                a.priority, a.project_name, a.task_title
            ));
        }
    }

    md
}

// === Worker YAML formatters ===

fn yaml_format_worker(worker: &worker::Worker) -> String {
    serde_yaml::to_string(worker).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_workers(workers: &[worker::Worker]) -> String {
    serde_yaml::to_string(workers).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

// === Run YAML formatters ===

fn yaml_format_run(run: &Run) -> String {
    serde_yaml::to_string(run).unwrap_or_else(|_| "Error formatting YAML".to_string())
}

fn yaml_format_runs(runs: &[Run]) -> String {
    serde_yaml::to_string(runs).unwrap_or_else(|_| "Error formatting YAML".to_string())
}
