use crate::cli::args::{CliOutputFormat, ReviewAction};
use crate::db;
use crate::error::Result;
use crate::output::{Output, OutputType};
use crate::services::{self, Workspace};
use granary_types::{Comment, Project, Task};

/// Output for reviewing a task
pub struct ReviewTaskOutput {
    pub task: Task,
    pub project: Project,
    pub comments: Vec<Comment>,
}

impl Output for ReviewTaskOutput {
    fn output_type() -> OutputType {
        OutputType::Prompt
    }

    fn to_json(&self) -> String {
        serde_json::json!({
            "type": "task",
            "task": &self.task,
            "project": { "id": &self.project.id, "name": &self.project.name },
            "comments": &self.comments,
        })
        .to_string()
    }

    fn to_prompt(&self) -> String {
        format_task_review_context(&self.task, &self.project, &self.comments)
    }

    fn to_text(&self) -> String {
        format!(
            "Review: {} - {} [{}]",
            self.task.id, self.task.title, self.task.status
        )
    }
}

/// Output for reviewing a project
pub struct ReviewProjectOutput {
    pub project: Project,
    pub tasks: Vec<Task>,
    pub comments: Vec<Comment>,
}

impl Output for ReviewProjectOutput {
    fn output_type() -> OutputType {
        OutputType::Prompt
    }

    fn to_json(&self) -> String {
        serde_json::json!({
            "type": "project",
            "project": &self.project,
            "tasks": &self.tasks,
            "comments": &self.comments,
        })
        .to_string()
    }

    fn to_prompt(&self) -> String {
        format_project_review_context(&self.project, &self.tasks, &self.comments)
    }

    fn to_text(&self) -> String {
        format!(
            "Review: {} - {} [{}]",
            self.project.id, self.project.name, self.project.status
        )
    }
}

/// Output for review approve/reject actions
pub struct ReviewActionOutput {
    pub entity_type: String,
    pub id: String,
    pub action: String,
    pub new_status: String,
}

impl Output for ReviewActionOutput {
    fn output_type() -> OutputType {
        OutputType::Text
    }

    fn to_json(&self) -> String {
        serde_json::json!({
            "entity_type": &self.entity_type,
            "id": &self.id,
            "action": &self.action,
            "status": &self.new_status,
        })
        .to_string()
    }

    fn to_prompt(&self) -> String {
        match self.action.as_str() {
            "approved" => format!("{} {} approved.", self.entity_type, self.id),
            "rejected" => format!("{} {} rejected.", self.entity_type, self.id),
            _ => format!("{} {} {}.", self.entity_type, self.id, self.action),
        }
    }

    fn to_text(&self) -> String {
        self.to_prompt()
    }
}

/// Handle review commands
pub async fn review(
    id: &str,
    action: Option<ReviewAction>,
    format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Determine if this is a task or project ID
    let is_project = db::projects::get(&pool, id).await?.is_some();
    let is_task = if !is_project {
        db::tasks::get(&pool, id).await?.is_some()
    } else {
        false
    };

    if !is_project && !is_task {
        return Err(crate::error::GranaryError::InvalidArgument(format!(
            "No task or project found with ID: {}",
            id
        )));
    }

    match action {
        None => {
            // Show review context
            if is_task {
                let task = services::get_task(&pool, id).await?;
                let project = services::get_project(&pool, &task.project_id).await?;
                let comments = db::comments::list_by_parent(&pool, id).await?;
                let output = ReviewTaskOutput {
                    task,
                    project,
                    comments,
                };
                println!("{}", output.format(format));
            } else {
                let project = services::get_project(&pool, id).await?;
                let tasks = services::list_tasks_by_project(&pool, id).await?;
                let comments = db::comments::list_by_parent(&pool, id).await?;
                let output = ReviewProjectOutput {
                    project,
                    tasks,
                    comments,
                };
                println!("{}", output.format(format));
            }
        }
        Some(ReviewAction::Approve { comment }) => {
            if is_task {
                let task = services::approve_task(&pool, id, comment.as_deref()).await?;
                let output = ReviewActionOutput {
                    entity_type: "Task".to_string(),
                    id: id.to_string(),
                    action: "approved".to_string(),
                    new_status: task.status,
                };
                println!("{}", output.format(format));
            } else {
                let project = services::approve_project(&pool, id, comment.as_deref()).await?;
                let output = ReviewActionOutput {
                    entity_type: "Project".to_string(),
                    id: id.to_string(),
                    action: "approved".to_string(),
                    new_status: project.status,
                };
                println!("{}", output.format(format));
            }
        }
        Some(ReviewAction::Reject { comment }) => {
            if is_task {
                let task = services::reject_task(&pool, id, &comment).await?;
                let output = ReviewActionOutput {
                    entity_type: "Task".to_string(),
                    id: id.to_string(),
                    action: "rejected".to_string(),
                    new_status: task.status,
                };
                println!("{}", output.format(format));
            } else {
                let project = services::reject_project(&pool, id, &comment).await?;
                let output = ReviewActionOutput {
                    entity_type: "Project".to_string(),
                    id: id.to_string(),
                    action: "rejected".to_string(),
                    new_status: project.status,
                };
                println!("{}", output.format(format));
            }
        }
    }

    Ok(())
}

fn format_task_review_context(task: &Task, project: &Project, comments: &[Comment]) -> String {
    let mut out = String::new();

    out.push_str(&format!("## Review: {} - {}\n\n", task.id, task.title));
    out.push_str(&format!("Project: {} ({})\n", project.name, project.id));
    out.push_str(&format!("Status: {}\n", task.status));
    out.push_str(&format!("Priority: {}\n", task.priority));
    if let Some(ref owner) = task.owner {
        out.push_str(&format!("Owner: {}\n", owner));
    }
    out.push('\n');

    if let Some(ref desc) = task.description {
        out.push_str(&format!("**Description:**\n{}\n\n", desc));
    }

    // Show progress/review comments
    let relevant: Vec<&Comment> = comments
        .iter()
        .filter(|c| c.kind == "progress" || c.kind == "review")
        .collect();
    if !relevant.is_empty() {
        out.push_str("**Comments:**\n");
        for c in &relevant {
            let author = c.author.as_deref().unwrap_or("unknown");
            out.push_str(&format!("- [{}] {}: {}\n", c.kind, author, c.content));
        }
        out.push('\n');
    }

    // Approve/reject commands
    out.push_str("**Actions:**\n");
    out.push_str("```bash\n");
    out.push_str(&format!(
        "granary review {} approve                # approve and complete\n",
        task.id
    ));
    out.push_str(&format!(
        "granary review {} reject \"feedback\"      # reject with feedback\n",
        task.id
    ));
    out.push_str("```");

    out
}

fn format_project_review_context(
    project: &Project,
    tasks: &[Task],
    comments: &[Comment],
) -> String {
    let mut out = String::new();

    out.push_str(&format!("## Review: {} - {}\n\n", project.id, project.name));
    out.push_str(&format!("Status: {}\n", project.status));
    if let Some(ref owner) = project.owner {
        out.push_str(&format!("Owner: {}\n", owner));
    }
    out.push('\n');

    if let Some(ref desc) = project.description {
        out.push_str(&format!("**Description:**\n{}\n\n", desc));
    }

    // Task table
    out.push_str("**Tasks:**\n");
    for task in tasks {
        let status_marker = match task.status.as_str() {
            "done" => "[x]",
            "in_review" => "[~]",
            "in_progress" => "[>]",
            "blocked" => "[!]",
            _ => "[ ]",
        };
        out.push_str(&format!(
            "  {} {} - {} ({})\n",
            status_marker, task.id, task.title, task.status
        ));
    }
    out.push('\n');

    // Show review comments
    let relevant: Vec<&Comment> = comments.iter().filter(|c| c.kind == "review").collect();
    if !relevant.is_empty() {
        out.push_str("**Review history:**\n");
        for c in &relevant {
            let author = c.author.as_deref().unwrap_or("unknown");
            out.push_str(&format!("- {}: {}\n", author, c.content));
        }
        out.push('\n');
    }

    // Actions
    out.push_str("**Actions:**\n");
    out.push_str("```bash\n");
    out.push_str(&format!(
        "granary review {} approve                # approve and complete project\n",
        project.id
    ));
    out.push_str(&format!(
        "# To reject: create follow-up tasks first, then reject:\n"
    ));
    out.push_str(&format!(
        "granary project {} tasks create \"Fix: <issue>\"\n",
        project.id
    ));
    out.push_str(&format!(
        "granary review {} reject \"feedback\"      # reopen project, draft tasks -> todo\n",
        project.id
    ));
    out.push_str("```");

    out
}
