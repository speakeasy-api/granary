use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Worker status enum representing the lifecycle states of a worker process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WorkerStatus {
    #[default]
    Pending,
    Running,
    Stopped,
    Error,
}

impl WorkerStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Error => "error",
        }
    }
}

impl std::str::FromStr for WorkerStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(WorkerStatus::Pending),
            "running" => Ok(WorkerStatus::Running),
            "stopped" => Ok(WorkerStatus::Stopped),
            "error" => Ok(WorkerStatus::Error),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Worker model representing a long-running process that subscribes to granary events
/// and spawns runners to execute commands.
///
/// Workers are stored in a global database (~/.granary/workers.db) to allow
/// `granary worker list` to show workers across all workspaces.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Worker {
    /// Unique identifier: worker-<8char>
    pub id: String,
    /// References a configured runner by name, or None for inline command
    pub runner_name: Option<String>,
    /// The command to execute
    pub command: String,
    /// Command arguments (stored as JSON array in database)
    pub args: String,
    /// Event type to subscribe to, e.g., "task.unblocked"
    pub event_type: String,
    /// Filter expressions (stored as JSON array), e.g., ["status!=draft"]
    pub filters: String,
    /// Maximum concurrent runner instances
    pub concurrency: i32,
    /// Workspace root path this worker is attached to
    pub instance_path: String,
    /// Current worker status: pending, running, stopped, error
    pub status: String,
    /// Error message if status is error
    pub error_message: Option<String>,
    /// OS process ID when the worker is running
    pub pid: Option<i64>,
    /// Whether the worker is running as a daemon (detached from terminal)
    pub detached: bool,
    /// Timestamp when the worker was created
    pub created_at: String,
    /// Timestamp when the worker was last updated
    pub updated_at: String,
    /// Timestamp when the worker was stopped
    pub stopped_at: Option<String>,
    /// Cooldown in seconds for polled events (task.next, project.next)
    /// Default is 300 seconds (5 minutes)
    pub poll_cooldown_secs: i64,
    /// ID of the last processed event for cursor-based polling
    pub last_event_id: i64,
}

impl Worker {
    /// Parse the status string to WorkerStatus enum
    pub fn status_enum(&self) -> WorkerStatus {
        self.status.parse().unwrap_or_default()
    }

    /// Parse the args JSON string to a Vec<String>
    pub fn args_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.args).unwrap_or_default()
    }

    /// Parse the filters JSON string to a Vec<String>
    pub fn filters_vec(&self) -> Vec<String> {
        serde_json::from_str(&self.filters).unwrap_or_default()
    }

    /// Check if the worker is currently running
    pub fn is_running(&self) -> bool {
        self.status_enum() == WorkerStatus::Running
    }

    /// Check if the worker has stopped (either normally or with error)
    pub fn is_stopped(&self) -> bool {
        matches!(
            self.status_enum(),
            WorkerStatus::Stopped | WorkerStatus::Error
        )
    }
}

/// Input for creating a new worker
#[derive(Debug, Clone)]
pub struct CreateWorker {
    pub runner_name: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub event_type: String,
    pub filters: Vec<String>,
    pub concurrency: i32,
    pub instance_path: String,
    pub poll_cooldown_secs: i64,
    pub detached: bool,
}

impl Default for CreateWorker {
    fn default() -> Self {
        Self {
            runner_name: None,
            command: String::new(),
            args: Vec::new(),
            event_type: String::new(),
            filters: Vec::new(),
            concurrency: 1,
            instance_path: String::new(),
            poll_cooldown_secs: 300, // 5 minutes default
            detached: false,
        }
    }
}

/// Input for updating worker status
#[derive(Debug, Clone)]
pub struct UpdateWorkerStatus {
    pub status: WorkerStatus,
    pub error_message: Option<String>,
    pub pid: Option<i64>,
}
