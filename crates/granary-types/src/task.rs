use serde::{Deserialize, Serialize};

#[cfg(feature = "sqlx")]
use sqlx::FromRow;

/// Task as returned by granary CLI JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub task_number: i64,
    #[serde(default)]
    pub parent_task_id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub worker_ids: Option<String>,
    #[serde(default)]
    pub run_ids: Option<String>,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub claim_owner: Option<String>,
    #[serde(default)]
    pub claim_claimed_at: Option<String>,
    #[serde(default)]
    pub claim_lease_expires_at: Option<String>,
    #[serde(default)]
    pub pinned: i64,
    #[serde(default)]
    pub focus_weight: i64,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
    #[serde(default)]
    pub last_edited_by: Option<String>,
    #[serde(default)]
    pub metadata: Option<String>,
}

impl Task {
    pub fn status_enum(&self) -> TaskStatus {
        self.status.parse().unwrap_or_default()
    }

    pub fn priority_enum(&self) -> TaskPriority {
        self.priority.parse().unwrap_or_default()
    }

    pub fn tags_vec(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }

    pub fn worker_ids_vec(&self) -> Vec<String> {
        self.worker_ids
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }

    pub fn metadata_value(&self) -> Option<serde_json::Value> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
    }

    pub fn run_ids_vec(&self) -> Vec<String> {
        self.run_ids
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }

    pub fn is_blocked(&self) -> bool {
        self.status_enum() == TaskStatus::Blocked || self.blocked_reason.is_some()
    }

    pub fn is_claimed(&self) -> bool {
        if let (Some(expires_at), Some(_)) = (&self.claim_lease_expires_at, &self.claim_owner) {
            // Check if lease is still valid
            if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_at) {
                return expires > chrono::Utc::now();
            }
        }
        false
    }

    pub fn claim_info(&self) -> Option<ClaimInfo> {
        if let (Some(owner), Some(claimed_at)) = (&self.claim_owner, &self.claim_claimed_at) {
            Some(ClaimInfo {
                owner: owner.clone(),
                claimed_at: claimed_at.clone(),
                lease_expires_at: self.claim_lease_expires_at.clone(),
            })
        } else {
            None
        }
    }
}

/// Task status enum with snake_case serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Draft,
    Todo,
    InProgress,
    InReview,
    Done,
    Blocked,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Draft => "draft",
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::InReview => "in_review",
            TaskStatus::Done => "done",
            TaskStatus::Blocked => "blocked",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Done)
    }

    pub fn is_actionable(&self) -> bool {
        matches!(self, TaskStatus::Todo | TaskStatus::Draft)
    }

    pub fn is_in_progress(&self) -> bool {
        matches!(self, TaskStatus::InProgress)
    }

    pub fn is_in_review(&self) -> bool {
        matches!(self, TaskStatus::InReview)
    }

    pub fn is_draft(&self) -> bool {
        matches!(self, TaskStatus::Draft)
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(TaskStatus::Draft),
            "todo" => Ok(TaskStatus::Todo),
            "in_progress" | "in-progress" | "inprogress" => Ok(TaskStatus::InProgress),
            "in_review" | "in-review" | "inreview" => Ok(TaskStatus::InReview),
            "done" | "completed" => Ok(TaskStatus::Done),
            "blocked" => Ok(TaskStatus::Blocked),
            _ => Err(()),
        }
    }
}

/// Task priority levels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TaskPriority {
    P0,
    P1,
    #[default]
    P2,
    P3,
    P4,
}

impl TaskPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskPriority::P0 => "P0",
            TaskPriority::P1 => "P1",
            TaskPriority::P2 => "P2",
            TaskPriority::P3 => "P3",
            TaskPriority::P4 => "P4",
        }
    }

    pub fn order(&self) -> i32 {
        match self {
            TaskPriority::P0 => 0,
            TaskPriority::P1 => 1,
            TaskPriority::P2 => 2,
            TaskPriority::P3 => 3,
            TaskPriority::P4 => 4,
        }
    }
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for TaskPriority {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "P0" => Ok(TaskPriority::P0),
            "P1" => Ok(TaskPriority::P1),
            "P2" => Ok(TaskPriority::P2),
            "P3" => Ok(TaskPriority::P3),
            "P4" => Ok(TaskPriority::P4),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimInfo {
    pub owner: String,
    pub claimed_at: String,
    pub lease_expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct TaskDependency {
    pub task_id: String,
    pub depends_on_task_id: String,
    pub created_at: String,
}

#[derive(Debug, Default)]
pub struct CreateTask {
    pub project_id: String,
    pub parent_task_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub owner: Option<String>,
    pub tags: Vec<String>,
    pub due_at: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Default)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub owner: Option<String>,
    pub tags: Option<Vec<String>>,
    pub worker_ids: Option<Vec<String>>,
    pub run_ids: Option<Vec<String>>,
    pub blocked_reason: Option<String>,
    pub due_at: Option<String>,
    pub pinned: Option<bool>,
    pub focus_weight: Option<i64>,
    pub metadata: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_task() -> Task {
        Task {
            id: "test-task-1".to_string(),
            project_id: "proj-1".to_string(),
            task_number: 1,
            parent_task_id: None,
            title: "Test".to_string(),
            description: None,
            status: "todo".to_string(),
            priority: "P2".to_string(),
            owner: None,
            tags: None,
            worker_ids: None,
            run_ids: None,
            blocked_reason: None,
            started_at: None,
            completed_at: None,
            due_at: None,
            claim_owner: None,
            claim_claimed_at: None,
            claim_lease_expires_at: None,
            pinned: 0,
            focus_weight: 0,
            metadata: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            version: 1,
            last_edited_by: None,
        }
    }

    #[test]
    fn metadata_value_returns_none_when_absent() {
        let task = make_task();
        assert!(task.metadata_value().is_none());
    }

    #[test]
    fn metadata_value_parses_valid_json() {
        let mut task = make_task();
        task.metadata = Some(r#"{"env":"production","retries":3}"#.to_string());
        let val = task.metadata_value().unwrap();
        assert_eq!(val["env"], "production");
        assert_eq!(val["retries"], 3);
    }

    #[test]
    fn metadata_value_returns_none_for_invalid_json() {
        let mut task = make_task();
        task.metadata = Some("not valid json".to_string());
        assert!(task.metadata_value().is_none());
    }

    #[test]
    fn metadata_value_handles_nested_objects() {
        let mut task = make_task();
        task.metadata = Some(r#"{"config":{"timeout":30,"debug":true}}"#.to_string());
        let val = task.metadata_value().unwrap();
        assert_eq!(val["config"]["timeout"], 30);
        assert_eq!(val["config"]["debug"], true);
    }

    #[test]
    fn metadata_roundtrips_through_serde() {
        let mut task = make_task();
        task.metadata = Some(r#"{"key":"value"}"#.to_string());

        let json = serde_json::to_string(&task).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["metadata"].as_str().unwrap(), r#"{"key":"value"}"#);
    }

    #[test]
    fn metadata_null_in_serialized_json_when_none() {
        let task = make_task();
        let json = serde_json::to_string(&task).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["metadata"].is_null());
    }

    #[test]
    fn metadata_deserialized_with_default_when_missing() {
        let json = r#"{
            "id": "t1", "project_id": "p1", "task_number": 1,
            "title": "test", "status": "todo", "priority": "P2",
            "pinned": 0, "focus_weight": 0,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "version": 1
        }"#;
        let task: Task = serde_json::from_str(json).unwrap();
        assert!(task.metadata.is_none());
        assert!(task.metadata_value().is_none());
    }
}
