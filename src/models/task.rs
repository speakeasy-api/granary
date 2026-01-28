use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Draft,
    Todo,
    InProgress,
    Done,
    Blocked,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Draft => "draft",
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Done => "done",
            TaskStatus::Blocked => "blocked",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Done)
    }

    pub fn is_draft(&self) -> bool {
        matches!(self, TaskStatus::Draft)
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(TaskStatus::Draft),
            "todo" => Ok(TaskStatus::Todo),
            "in_progress" | "in-progress" | "inprogress" => Ok(TaskStatus::InProgress),
            "done" | "completed" => Ok(TaskStatus::Done),
            "blocked" => Ok(TaskStatus::Blocked),
            _ => Err(()),
        }
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub task_number: i64,
    pub parent_task_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: String,
    pub owner: Option<String>,
    pub tags: Option<String>, // JSON array
    pub blocked_reason: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub due_at: Option<String>,

    // Claim/lease fields
    pub claim_owner: Option<String>,
    pub claim_claimed_at: Option<String>,
    pub claim_lease_expires_at: Option<String>,

    // Attention/focus fields
    pub pinned: i64,
    pub focus_weight: i64,

    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimInfo {
    pub owner: String,
    pub claimed_at: String,
    pub lease_expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
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
    pub owner: Option<String>,
    pub tags: Vec<String>,
    pub due_at: Option<String>,
}

#[derive(Debug, Default)]
pub struct UpdateTask {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub owner: Option<String>,
    pub tags: Option<Vec<String>>,
    pub blocked_reason: Option<String>,
    pub due_at: Option<String>,
    pub pinned: Option<bool>,
    pub focus_weight: Option<i64>,
}
