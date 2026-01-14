use tabled::{Table, Tabled};

use crate::models::*;

#[derive(Tabled)]
struct ProjectRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Owner")]
    owner: String,
    #[tabled(rename = "Created")]
    created: String,
}

impl From<&Project> for ProjectRow {
    fn from(p: &Project) -> Self {
        Self {
            id: p.id.clone(),
            name: truncate(&p.name, 30),
            status: p.status.clone(),
            owner: p.owner.clone().unwrap_or_else(|| "-".to_string()),
            created: format_date(&p.created_at),
        }
    }
}

pub fn format_project(project: &Project) -> String {
    let mut output = String::new();
    output.push_str(&format!("Project: {}\n", project.name));
    output.push_str(&format!("  ID:          {}\n", project.id));
    output.push_str(&format!("  Status:      {}\n", project.status));
    output.push_str(&format!(
        "  Owner:       {}\n",
        project.owner.as_deref().unwrap_or("-")
    ));
    if let Some(desc) = &project.description {
        output.push_str(&format!("  Description: {}\n", desc));
    }
    let tags = project.tags_vec();
    if !tags.is_empty() {
        output.push_str(&format!("  Tags:        {}\n", tags.join(", ")));
    }
    output.push_str(&format!("  Created:     {}\n", project.created_at));
    output.push_str(&format!("  Updated:     {}\n", project.updated_at));
    output
}

pub fn format_projects(projects: &[Project]) -> String {
    if projects.is_empty() {
        return "No projects found.\n".to_string();
    }
    let rows: Vec<ProjectRow> = projects.iter().map(ProjectRow::from).collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct TaskRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Priority")]
    priority: String,
    #[tabled(rename = "Owner")]
    owner: String,
}

impl From<&Task> for TaskRow {
    fn from(t: &Task) -> Self {
        let status = if t.blocked_reason.is_some() {
            format!("{} (blocked)", t.status)
        } else {
            t.status.clone()
        };
        Self {
            id: t.id.clone(),
            title: truncate(&t.title, 40),
            status,
            priority: t.priority.clone(),
            owner: t.owner.clone().unwrap_or_else(|| "-".to_string()),
        }
    }
}

pub fn format_task(task: &Task) -> String {
    let mut output = String::new();
    output.push_str(&format!("Task: {}\n", task.title));
    output.push_str(&format!("  ID:          {}\n", task.id));
    output.push_str(&format!("  Project:     {}\n", task.project_id));
    output.push_str(&format!("  Status:      {}\n", task.status));
    output.push_str(&format!("  Priority:    {}\n", task.priority));
    output.push_str(&format!(
        "  Owner:       {}\n",
        task.owner.as_deref().unwrap_or("-")
    ));
    if let Some(parent) = &task.parent_task_id {
        output.push_str(&format!("  Parent:      {}\n", parent));
    }
    if let Some(desc) = &task.description {
        output.push_str(&format!("  Description: {}\n", desc));
    }
    if let Some(reason) = &task.blocked_reason {
        output.push_str(&format!("  Blocked:     {}\n", reason));
    }
    if let Some(due) = &task.due_at {
        output.push_str(&format!("  Due:         {}\n", due));
    }
    if task.pinned != 0 {
        output.push_str("  Pinned:      yes\n");
    }
    if let Some(claim) = task.claim_info() {
        output.push_str(&format!("  Claimed by:  {}\n", claim.owner));
        if let Some(expires) = claim.lease_expires_at {
            output.push_str(&format!("  Lease until: {}\n", expires));
        }
    }
    output.push_str(&format!("  Created:     {}\n", task.created_at));
    output.push_str(&format!("  Updated:     {}\n", task.updated_at));
    output
}

pub fn format_tasks(tasks: &[Task]) -> String {
    if tasks.is_empty() {
        return "No tasks found.\n".to_string();
    }
    let rows: Vec<TaskRow> = tasks.iter().map(TaskRow::from).collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct CommentRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Kind")]
    kind: String,
    #[tabled(rename = "Content")]
    content: String,
    #[tabled(rename = "Author")]
    author: String,
    #[tabled(rename = "Created")]
    created: String,
}

impl From<&Comment> for CommentRow {
    fn from(c: &Comment) -> Self {
        Self {
            id: c.id.clone(),
            kind: c.kind.clone(),
            content: truncate(&c.content, 50),
            author: c.author.clone().unwrap_or_else(|| "-".to_string()),
            created: format_date(&c.created_at),
        }
    }
}

pub fn format_comment(comment: &Comment) -> String {
    let mut output = String::new();
    output.push_str(&format!("Comment: {}\n", comment.id));
    output.push_str(&format!("  Kind:    {}\n", comment.kind));
    output.push_str(&format!("  Parent:  {}\n", comment.parent_id));
    output.push_str(&format!(
        "  Author:  {}\n",
        comment.author.as_deref().unwrap_or("-")
    ));
    output.push_str(&format!("  Created: {}\n", comment.created_at));
    output.push_str(&format!("\n{}\n", comment.content));
    output
}

pub fn format_comments(comments: &[Comment]) -> String {
    if comments.is_empty() {
        return "No comments found.\n".to_string();
    }
    let rows: Vec<CommentRow> = comments.iter().map(CommentRow::from).collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct SessionRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Mode")]
    mode: String,
    #[tabled(rename = "Owner")]
    owner: String,
    #[tabled(rename = "Status")]
    status: String,
}

impl From<&Session> for SessionRow {
    fn from(s: &Session) -> Self {
        Self {
            id: s.id.clone(),
            name: s.name.clone().unwrap_or_else(|| "-".to_string()),
            mode: s.mode.clone().unwrap_or_else(|| "execute".to_string()),
            owner: s.owner.clone().unwrap_or_else(|| "-".to_string()),
            status: if s.is_closed() {
                "closed".to_string()
            } else {
                "active".to_string()
            },
        }
    }
}

pub fn format_session(session: &Session) -> String {
    let mut output = String::new();
    let name = session.name.as_deref().unwrap_or("Unnamed Session");
    output.push_str(&format!("Session: {}\n", name));
    output.push_str(&format!("  ID:     {}\n", session.id));
    output.push_str(&format!(
        "  Mode:   {}\n",
        session.mode.as_deref().unwrap_or("execute")
    ));
    output.push_str(&format!(
        "  Owner:  {}\n",
        session.owner.as_deref().unwrap_or("-")
    ));
    let status = if session.is_closed() {
        "closed"
    } else {
        "active"
    };
    output.push_str(&format!("  Status: {}\n", status));
    if let Some(focus) = &session.focus_task_id {
        output.push_str(&format!("  Focus:  {}\n", focus));
    }
    output.push_str(&format!("  Created: {}\n", session.created_at));
    if let Some(closed) = &session.closed_at {
        output.push_str(&format!("  Closed:  {}\n", closed));
    }
    output
}

pub fn format_sessions(sessions: &[Session]) -> String {
    if sessions.is_empty() {
        return "No sessions found.\n".to_string();
    }
    let rows: Vec<SessionRow> = sessions.iter().map(SessionRow::from).collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct CheckpointRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Session")]
    session: String,
    #[tabled(rename = "Created")]
    created: String,
}

impl From<&Checkpoint> for CheckpointRow {
    fn from(c: &Checkpoint) -> Self {
        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            session: c.session_id.clone(),
            created: format_date(&c.created_at),
        }
    }
}

pub fn format_checkpoint(checkpoint: &Checkpoint) -> String {
    let mut output = String::new();
    output.push_str(&format!("Checkpoint: {}\n", checkpoint.name));
    output.push_str(&format!("  ID:      {}\n", checkpoint.id));
    output.push_str(&format!("  Session: {}\n", checkpoint.session_id));
    output.push_str(&format!("  Created: {}\n", checkpoint.created_at));
    output
}

pub fn format_checkpoints(checkpoints: &[Checkpoint]) -> String {
    if checkpoints.is_empty() {
        return "No checkpoints found.\n".to_string();
    }
    let rows: Vec<CheckpointRow> = checkpoints.iter().map(CheckpointRow::from).collect();
    Table::new(rows).to_string()
}

#[derive(Tabled)]
struct ArtifactRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Type")]
    artifact_type: String,
    #[tabled(rename = "Path/URL")]
    path: String,
    #[tabled(rename = "Description")]
    description: String,
}

impl From<&Artifact> for ArtifactRow {
    fn from(a: &Artifact) -> Self {
        Self {
            id: a.id.clone(),
            artifact_type: a.artifact_type.clone(),
            path: truncate(&a.path_or_url, 40),
            description: a.description.clone().unwrap_or_else(|| "-".to_string()),
        }
    }
}

pub fn format_artifact(artifact: &Artifact) -> String {
    let mut output = String::new();
    output.push_str(&format!("Artifact: {}\n", artifact.id));
    output.push_str(&format!("  Type:   {}\n", artifact.artifact_type));
    output.push_str(&format!("  Path:   {}\n", artifact.path_or_url));
    if let Some(desc) = &artifact.description {
        output.push_str(&format!("  Desc:   {}\n", desc));
    }
    output.push_str(&format!("  Parent: {}\n", artifact.parent_id));
    output
}

pub fn format_artifacts(artifacts: &[Artifact]) -> String {
    if artifacts.is_empty() {
        return "No artifacts found.\n".to_string();
    }
    let rows: Vec<ArtifactRow> = artifacts.iter().map(ArtifactRow::from).collect();
    Table::new(rows).to_string()
}

pub fn format_next_task(task: Option<&Task>, reason: Option<&str>) -> String {
    match task {
        Some(t) => {
            let mut output = String::new();
            output.push_str("Next task:\n");
            output.push_str(&format!("  ID:       {}\n", t.id));
            output.push_str(&format!("  Title:    {}\n", t.title));
            output.push_str(&format!("  Priority: {}\n", t.priority));
            output.push_str(&format!("  Status:   {}\n", t.status));
            if let Some(r) = reason {
                output.push_str(&format!("  Reason:   {}\n", r));
            }
            output
        }
        None => {
            let reason = reason.unwrap_or("No actionable tasks found");
            format!("No next task: {}\n", reason)
        }
    }
}

#[derive(Tabled)]
struct SearchResultRow {
    #[tabled(rename = "Type")]
    entity_type: String,
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Title/Name")]
    title: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Priority")]
    priority: String,
}

impl From<&SearchResult> for SearchResultRow {
    fn from(result: &SearchResult) -> Self {
        match result {
            SearchResult::Project {
                id, name, status, ..
            } => Self {
                entity_type: "project".to_string(),
                id: id.clone(),
                title: truncate(name, 40),
                status: status.clone(),
                priority: "-".to_string(),
            },
            SearchResult::Task {
                id,
                title,
                status,
                priority,
                ..
            } => Self {
                entity_type: "task".to_string(),
                id: id.clone(),
                title: truncate(title, 40),
                status: status.clone(),
                priority: priority.clone(),
            },
        }
    }
}

pub fn format_search_results(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "No results found.\n".to_string();
    }
    let rows: Vec<SearchResultRow> = results.iter().map(SearchResultRow::from).collect();
    Table::new(rows).to_string()
}

// Helper functions
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

fn format_date(iso_date: &str) -> String {
    // Just return date portion for brevity in tables
    if iso_date.len() >= 10 {
        iso_date[..10].to_string()
    } else {
        iso_date.to_string()
    }
}
