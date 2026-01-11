use serde::Serialize;
use serde_json::json;

use crate::models::*;

pub fn format_project(project: &Project) -> String {
    serde_json::to_string_pretty(project).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_projects(projects: &[Project]) -> String {
    serde_json::to_string_pretty(projects).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_task(task: &Task) -> String {
    serde_json::to_string_pretty(task).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_tasks(tasks: &[Task]) -> String {
    serde_json::to_string_pretty(tasks).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_comment(comment: &Comment) -> String {
    serde_json::to_string_pretty(comment).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_comments(comments: &[Comment]) -> String {
    serde_json::to_string_pretty(comments).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_session(session: &Session) -> String {
    serde_json::to_string_pretty(session).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_sessions(sessions: &[Session]) -> String {
    serde_json::to_string_pretty(sessions).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_checkpoint(checkpoint: &Checkpoint) -> String {
    serde_json::to_string_pretty(checkpoint).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_checkpoints(checkpoints: &[Checkpoint]) -> String {
    serde_json::to_string_pretty(checkpoints).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_artifact(artifact: &Artifact) -> String {
    serde_json::to_string_pretty(artifact).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_artifacts(artifacts: &[Artifact]) -> String {
    serde_json::to_string_pretty(artifacts).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_next_task(task: Option<&Task>, reason: Option<&str>) -> String {
    let output = if let Some(t) = task {
        json!({
            "task": t,
            "reason": reason
        })
    } else {
        json!({
            "task": null,
            "reason": "No actionable tasks found"
        })
    };
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

/// Format a summary as JSON
#[derive(Serialize)]
pub struct SummaryOutput {
    pub session: Option<SessionSummary>,
    pub state: StateSummary,
    pub focus_task: Option<Task>,
    pub blockers: Vec<Task>,
    pub next_actions: Vec<Task>,
    pub recent_decisions: Vec<Comment>,
    pub recent_artifacts: Vec<Artifact>,
}

#[derive(Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub name: Option<String>,
    pub mode: Option<String>,
    pub owner: Option<String>,
    pub focus_task_id: Option<String>,
}

#[derive(Serialize)]
pub struct StateSummary {
    pub total_tasks: usize,
    pub by_status: StatusCounts,
    pub by_priority: PriorityCounts,
}

#[derive(Serialize, Default)]
pub struct StatusCounts {
    pub todo: usize,
    pub in_progress: usize,
    pub done: usize,
    pub blocked: usize,
}

#[derive(Serialize, Default)]
pub struct PriorityCounts {
    pub p0: usize,
    pub p1: usize,
    pub p2: usize,
    pub p3: usize,
    pub p4: usize,
}

pub fn format_summary(summary: &SummaryOutput) -> String {
    serde_json::to_string_pretty(summary).unwrap_or_else(|_| "{}".to_string())
}

/// Format a context pack as JSON
#[derive(Serialize)]
pub struct ContextOutput {
    pub session: Option<SessionSummary>,
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
    pub comments: Vec<Comment>,
    pub artifacts: Vec<Artifact>,
    pub decisions: Vec<Comment>,
    pub blockers: Vec<BlockerInfo>,
}

#[derive(Serialize)]
pub struct BlockerInfo {
    pub task_id: String,
    pub task_title: String,
    pub reason: Option<String>,
    pub unmet_deps: Vec<String>,
}

pub fn format_context(context: &ContextOutput) -> String {
    serde_json::to_string_pretty(context).unwrap_or_else(|_| "{}".to_string())
}

/// Format handoff as JSON
#[derive(Serialize)]
pub struct HandoffOutput {
    pub to: String,
    pub tasks: Vec<Task>,
    pub context: Vec<Comment>,
    pub constraints: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub output_schema: Option<serde_json::Value>,
}

pub fn format_handoff(handoff: &HandoffOutput) -> String {
    serde_json::to_string_pretty(handoff).unwrap_or_else(|_| "{}".to_string())
}

/// Format checkpoint diff as JSON
#[derive(Serialize)]
pub struct CheckpointDiff {
    pub from: String,
    pub to: String,
    pub changes: Vec<DiffChange>,
}

#[derive(Serialize)]
pub struct DiffChange {
    pub entity_type: String,
    pub entity_id: String,
    pub field: String,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

pub fn format_checkpoint_diff(diff: &CheckpointDiff) -> String {
    serde_json::to_string_pretty(diff).unwrap_or_else(|_| "{}".to_string())
}
