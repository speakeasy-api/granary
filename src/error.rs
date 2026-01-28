use thiserror::Error;

/// Exit codes as specified in the design doc
pub mod exit_codes {
    pub const SUCCESS: i32 = 0;
    pub const USER_ERROR: i32 = 2;
    pub const NOT_FOUND: i32 = 3;
    pub const CONFLICT: i32 = 4;
    pub const BLOCKED: i32 = 5;
    pub const INTERNAL: i32 = 1;
}

#[derive(Error, Debug)]
pub enum GranaryError {
    #[error("Workspace not found. Run 'granary init' first.")]
    WorkspaceNotFound,

    #[error("Workspace already exists at {0}")]
    WorkspaceAlreadyExists(String),

    #[error("Project not found: {0}")]
    ProjectNotFound(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Comment not found: {0}")]
    CommentNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(String),

    #[error("Artifact not found: {0}")]
    ArtifactNotFound(String),

    #[error("Initiative not found: {0}")]
    InitiativeNotFound(String),

    #[error("Worker not found: {0}")]
    WorkerNotFound(String),

    #[error("Run not found: {0}")]
    RunNotFound(String),

    #[error("Runner not found: {0}")]
    RunnerNotFound(String),

    #[error("No active session. Start one with 'granary session start <name>'.")]
    NoActiveSession,

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: i64, found: i64 },

    #[error("Task is blocked: {0}")]
    TaskBlocked(String),

    #[error("Task has unmet dependencies: {0}")]
    UnmetDependencies(String),

    #[error("Dependency cycle detected: {0}")]
    DependencyCycle(String),

    #[error("Claim conflict: task is claimed by {owner} until {expires_at}")]
    ClaimConflict { owner: String, expires_at: String },

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Invalid ID format: {0}")]
    InvalidId(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Update error: {0}")]
    Update(String),

    #[error("Global config error: {0}")]
    GlobalConfig(String),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Failed to connect to daemon: {0}")]
    DaemonConnection(String),

    #[error("Daemon protocol error: {0}")]
    DaemonProtocol(String),

    #[error("Daemon error: {0}")]
    DaemonError(String),
}

impl GranaryError {
    pub fn exit_code(&self) -> i32 {
        match self {
            // User errors (bad arguments, invalid input)
            GranaryError::InvalidArgument(_) | GranaryError::InvalidId(_) => exit_codes::USER_ERROR,

            // Not found errors
            GranaryError::WorkspaceNotFound
            | GranaryError::ProjectNotFound(_)
            | GranaryError::TaskNotFound(_)
            | GranaryError::CommentNotFound(_)
            | GranaryError::SessionNotFound(_)
            | GranaryError::CheckpointNotFound(_)
            | GranaryError::ArtifactNotFound(_)
            | GranaryError::InitiativeNotFound(_)
            | GranaryError::WorkerNotFound(_)
            | GranaryError::RunNotFound(_)
            | GranaryError::RunnerNotFound(_)
            | GranaryError::NoActiveSession => exit_codes::NOT_FOUND,

            // Conflict errors (concurrency, claims)
            GranaryError::Conflict(_)
            | GranaryError::VersionMismatch { .. }
            | GranaryError::ClaimConflict { .. }
            | GranaryError::WorkspaceAlreadyExists(_)
            | GranaryError::DependencyCycle(_) => exit_codes::CONFLICT,

            // Blocked errors (deps unmet, task blocked)
            GranaryError::TaskBlocked(_) | GranaryError::UnmetDependencies(_) => {
                exit_codes::BLOCKED
            }

            // Internal errors
            GranaryError::Database(_)
            | GranaryError::Migration(_)
            | GranaryError::Io(_)
            | GranaryError::Json(_)
            | GranaryError::Yaml(_)
            | GranaryError::Network(_)
            | GranaryError::Update(_)
            | GranaryError::GlobalConfig(_)
            | GranaryError::Toml(_)
            | GranaryError::DaemonConnection(_)
            | GranaryError::DaemonProtocol(_)
            | GranaryError::DaemonError(_) => exit_codes::INTERNAL,
        }
    }
}

pub type Result<T> = std::result::Result<T, GranaryError>;
