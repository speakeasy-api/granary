use serde::Serialize;
use serde_json::json;

use crate::models::initiative::Initiative;
use crate::models::*;

/// Task output with dependency information
/// This enriched struct includes the blocked_by field that shows unmet dependencies
#[derive(Serialize)]
pub struct TaskOutput {
    #[serde(flatten)]
    pub task: Task,
    /// List of task IDs that block this task (unmet dependencies)
    pub blocked_by: Vec<String>,
}

impl TaskOutput {
    pub fn new(task: Task, blocked_by: Vec<String>) -> Self {
        Self { task, blocked_by }
    }

    pub fn from_task(task: Task) -> Self {
        Self {
            task,
            blocked_by: vec![],
        }
    }
}

pub fn format_project(project: &Project) -> String {
    serde_json::to_string_pretty(project).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_projects(projects: &[Project]) -> String {
    serde_json::to_string_pretty(projects).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_task(task: &Task) -> String {
    let output = TaskOutput::from_task(task.clone());
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_task_with_deps(task: &Task, blocked_by: Vec<String>) -> String {
    let output = TaskOutput::new(task.clone(), blocked_by);
    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_tasks(tasks: &[Task]) -> String {
    let outputs: Vec<TaskOutput> = tasks
        .iter()
        .map(|t| TaskOutput::from_task(t.clone()))
        .collect();
    serde_json::to_string_pretty(&outputs).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_tasks_with_deps(tasks_with_deps: &[(Task, Vec<String>)]) -> String {
    let outputs: Vec<TaskOutput> = tasks_with_deps
        .iter()
        .map(|(t, deps)| TaskOutput::new(t.clone(), deps.clone()))
        .collect();
    serde_json::to_string_pretty(&outputs).unwrap_or_else(|_| "[]".to_string())
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

/// Steering file information for context packs
#[derive(Serialize)]
pub struct SteeringInfo {
    pub path: String,
    pub mode: String,
    pub content: Option<String>,
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
    pub steering: Vec<SteeringInfo>,
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
    pub steering: Vec<SteeringInfo>,
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

pub fn format_search_results(results: &[SearchResult]) -> String {
    serde_json::to_string_pretty(results).unwrap_or_else(|_| "[]".to_string())
}

pub fn format_initiative(initiative: &Initiative) -> String {
    serde_json::to_string_pretty(initiative).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_initiatives(initiatives: &[Initiative]) -> String {
    serde_json::to_string_pretty(initiatives).unwrap_or_else(|_| "[]".to_string())
}

// === Initiative Summary ===

use crate::models::initiative::InitiativeSummary;

pub fn format_initiative_summary(summary: &InitiativeSummary) -> String {
    serde_json::to_string_pretty(summary).unwrap_or_else(|_| "{}".to_string())
}

// === Worker formatting ===

use crate::models::worker::Worker;

pub fn format_worker(worker: &Worker) -> String {
    serde_json::to_string_pretty(worker).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_workers(workers: &[Worker]) -> String {
    serde_json::to_string_pretty(workers).unwrap_or_else(|_| "[]".to_string())
}

// === Run formatting ===

use crate::models::run::Run;

pub fn format_run(run: &Run) -> String {
    serde_json::to_string_pretty(run).unwrap_or_else(|_| "{}".to_string())
}

pub fn format_runs(runs: &[Run]) -> String {
    serde_json::to_string_pretty(runs).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_task() -> Task {
        Task {
            id: "test-proj-task-1".to_string(),
            project_id: "test-proj".to_string(),
            task_number: 1,
            parent_task_id: None,
            title: "Test Task".to_string(),
            description: Some("A test task description".to_string()),
            status: "todo".to_string(),
            priority: "P1".to_string(),
            owner: Some("test-user".to_string()),
            tags: None,
            blocked_reason: None,
            started_at: None,
            completed_at: None,
            due_at: None,
            claim_owner: None,
            claim_claimed_at: None,
            claim_lease_expires_at: None,
            pinned: 0,
            focus_weight: 0,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            version: 1,
        }
    }

    #[test]
    fn test_task_output_with_no_dependencies() {
        let task = create_test_task();
        let output = format_task(&task);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        // Should have blocked_by as empty array, not null
        assert!(parsed.get("blocked_by").is_some());
        assert!(parsed["blocked_by"].is_array());
        assert_eq!(parsed["blocked_by"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_task_output_with_one_dependency() {
        let task = create_test_task();
        let blocked_by = vec!["dep-task-1".to_string()];
        let output = format_task_with_deps(&task, blocked_by);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        assert!(parsed["blocked_by"].is_array());
        let deps = parsed["blocked_by"].as_array().unwrap();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str().unwrap(), "dep-task-1");
    }

    #[test]
    fn test_task_output_with_multiple_dependencies() {
        let task = create_test_task();
        let blocked_by = vec![
            "dep-task-1".to_string(),
            "dep-task-2".to_string(),
            "dep-task-3".to_string(),
        ];
        let output = format_task_with_deps(&task, blocked_by);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let deps = parsed["blocked_by"].as_array().unwrap();
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].as_str().unwrap(), "dep-task-1");
        assert_eq!(deps[1].as_str().unwrap(), "dep-task-2");
        assert_eq!(deps[2].as_str().unwrap(), "dep-task-3");
    }

    #[test]
    fn test_tasks_output_includes_blocked_by() {
        let task1 = create_test_task();
        let mut task2 = create_test_task();
        task2.id = "test-proj-task-2".to_string();
        task2.task_number = 2;

        let tasks = vec![task1, task2];
        let output = format_tasks(&tasks);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        // Each task should have blocked_by array
        assert!(parsed.is_array());
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        for item in arr {
            assert!(item.get("blocked_by").is_some());
            assert!(item["blocked_by"].is_array());
        }
    }

    #[test]
    fn test_tasks_with_deps_output() {
        let task1 = create_test_task();
        let mut task2 = create_test_task();
        task2.id = "test-proj-task-2".to_string();
        task2.task_number = 2;

        let tasks_with_deps = vec![(task1, vec!["blocker-1".to_string()]), (task2, vec![])];
        let output = format_tasks_with_deps(&tasks_with_deps);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);

        // First task has one blocker
        assert_eq!(arr[0]["blocked_by"].as_array().unwrap().len(), 1);
        assert_eq!(arr[0]["blocked_by"][0].as_str().unwrap(), "blocker-1");

        // Second task has no blockers
        assert_eq!(arr[1]["blocked_by"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_task_output_preserves_all_fields() {
        let task = create_test_task();
        let output = format_task(&task);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();

        // Verify task fields are preserved
        assert_eq!(parsed["id"].as_str().unwrap(), "test-proj-task-1");
        assert_eq!(parsed["title"].as_str().unwrap(), "Test Task");
        assert_eq!(parsed["status"].as_str().unwrap(), "todo");
        assert_eq!(parsed["priority"].as_str().unwrap(), "P1");
        assert_eq!(parsed["project_id"].as_str().unwrap(), "test-proj");
        assert_eq!(parsed["owner"].as_str().unwrap(), "test-user");
    }
}
