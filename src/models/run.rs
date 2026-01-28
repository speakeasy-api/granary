use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Run status enum representing the lifecycle states of a runner execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    #[default]
    Pending, // queued, waiting to start
    Running,   // currently executing
    Completed, // finished successfully (exit code 0)
    Failed,    // finished with error (exit code != 0)
    Paused,    // manually paused
    Cancelled, // manually cancelled
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Paused => "paused",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(RunStatus::Pending),
            "running" => Ok(RunStatus::Running),
            "completed" => Ok(RunStatus::Completed),
            "failed" => Ok(RunStatus::Failed),
            "paused" => Ok(RunStatus::Paused),
            "cancelled" => Ok(RunStatus::Cancelled),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Run model representing a single execution of a runner process.
///
/// Each time a worker spawns a runner in response to an event, that execution
/// is tracked as a "Run". Runs have their own lifecycle and support retry
/// with exponential backoff.
///
/// Runs are stored in the same global database as workers (~/.granary/workers.db).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Run {
    /// Unique identifier: run-<8char>
    pub id: String,
    /// Which worker spawned this run
    pub worker_id: String,
    /// Which event triggered this run
    pub event_id: i64,
    /// Event type, e.g., "task.unblocked"
    pub event_type: String,
    /// Entity ID that triggered the event, e.g., task ID
    pub entity_id: String,
    /// Resolved command to execute
    pub command: String,
    /// Resolved arguments (stored as JSON array)
    pub args: String,
    /// Current run status: pending, running, completed, failed, paused, cancelled
    pub status: String,
    /// Exit code when completed or failed
    pub exit_code: Option<i32>,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Retry attempt number (1-based)
    pub attempt: i32,
    /// Maximum retry attempts before giving up
    pub max_attempts: i32,
    /// When to retry (with exponential backoff), RFC3339 timestamp
    pub next_retry_at: Option<String>,
    /// OS process ID when running
    pub pid: Option<i64>,
    /// Path to stdout/stderr log file
    pub log_path: Option<String>,
    /// Timestamp when execution started
    pub started_at: Option<String>,
    /// Timestamp when execution completed
    pub completed_at: Option<String>,
    /// Timestamp when the run was created
    pub created_at: String,
    /// Timestamp when the run was last updated
    pub updated_at: String,
}

impl Run {
    /// Parse the status string to RunStatus enum
    pub fn status_enum(&self) -> RunStatus {
        self.status.parse().unwrap_or_default()
    }

    /// Parse the args JSON string to a Vec<String>
    pub fn args_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.args).unwrap_or_default()
    }

    /// Check if the run is currently executing
    pub fn is_running(&self) -> bool {
        self.status_enum() == RunStatus::Running
    }

    /// Check if the run has finished (completed, failed, or cancelled)
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status_enum(),
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
        )
    }

    /// Check if the run can be retried
    pub fn can_retry(&self) -> bool {
        self.status_enum() == RunStatus::Failed && self.attempt < self.max_attempts
    }

    /// Check if the run is waiting for retry
    pub fn is_pending_retry(&self) -> bool {
        self.status_enum() == RunStatus::Pending && self.attempt > 1
    }
}

/// Input for creating a new run
#[derive(Debug, Clone)]
pub struct CreateRun {
    pub worker_id: String,
    pub event_id: i64,
    pub event_type: String,
    pub entity_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub max_attempts: i32,
    pub log_path: Option<String>,
}

impl Default for CreateRun {
    fn default() -> Self {
        Self {
            worker_id: String::new(),
            event_id: 0,
            event_type: String::new(),
            entity_id: String::new(),
            command: String::new(),
            args: Vec::new(),
            max_attempts: 3,
            log_path: None,
        }
    }
}

/// Input for updating run status
#[derive(Debug, Clone)]
pub struct UpdateRunStatus {
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    pub pid: Option<i64>,
}

/// Input for scheduling a retry
#[derive(Debug, Clone)]
pub struct ScheduleRetry {
    pub next_retry_at: String,
    pub attempt: i32,
}
