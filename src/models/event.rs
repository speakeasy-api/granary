use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Event types for the audit log
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    // Project events
    ProjectCreated,
    ProjectUpdated,
    ProjectArchived,

    // Task events
    TaskCreated,
    TaskUpdated,
    TaskStatusChanged,
    TaskStarted,
    TaskCompleted,
    TaskBlocked,
    TaskUnblocked,
    TaskClaimed,
    TaskReleased,

    // Dependency events
    DependencyAdded,
    DependencyRemoved,

    // Comment events
    CommentCreated,
    CommentUpdated,

    // Session events
    SessionStarted,
    SessionUpdated,
    SessionClosed,
    SessionScopeAdded,
    SessionScopeRemoved,
    SessionFocusChanged,

    // Checkpoint events
    CheckpointCreated,
    CheckpointRestored,

    // Artifact events
    ArtifactAdded,
    ArtifactRemoved,

    // Other
    Custom(String),
}

impl EventType {
    pub fn as_str(&self) -> String {
        match self {
            EventType::ProjectCreated => "project.created".to_string(),
            EventType::ProjectUpdated => "project.updated".to_string(),
            EventType::ProjectArchived => "project.archived".to_string(),
            EventType::TaskCreated => "task.created".to_string(),
            EventType::TaskUpdated => "task.updated".to_string(),
            EventType::TaskStatusChanged => "task.status_changed".to_string(),
            EventType::TaskStarted => "task.started".to_string(),
            EventType::TaskCompleted => "task.completed".to_string(),
            EventType::TaskBlocked => "task.blocked".to_string(),
            EventType::TaskUnblocked => "task.unblocked".to_string(),
            EventType::TaskClaimed => "task.claimed".to_string(),
            EventType::TaskReleased => "task.released".to_string(),
            EventType::DependencyAdded => "dependency.added".to_string(),
            EventType::DependencyRemoved => "dependency.removed".to_string(),
            EventType::CommentCreated => "comment.created".to_string(),
            EventType::CommentUpdated => "comment.updated".to_string(),
            EventType::SessionStarted => "session.started".to_string(),
            EventType::SessionUpdated => "session.updated".to_string(),
            EventType::SessionClosed => "session.closed".to_string(),
            EventType::SessionScopeAdded => "session.scope_added".to_string(),
            EventType::SessionScopeRemoved => "session.scope_removed".to_string(),
            EventType::SessionFocusChanged => "session.focus_changed".to_string(),
            EventType::CheckpointCreated => "checkpoint.created".to_string(),
            EventType::CheckpointRestored => "checkpoint.restored".to_string(),
            EventType::ArtifactAdded => "artifact.added".to_string(),
            EventType::ArtifactRemoved => "artifact.removed".to_string(),
            EventType::Custom(s) => s.clone(),
        }
    }
}

impl std::str::FromStr for EventType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "project.created" => EventType::ProjectCreated,
            "project.updated" => EventType::ProjectUpdated,
            "project.archived" => EventType::ProjectArchived,
            "task.created" => EventType::TaskCreated,
            "task.updated" => EventType::TaskUpdated,
            "task.status_changed" => EventType::TaskStatusChanged,
            "task.started" => EventType::TaskStarted,
            "task.completed" => EventType::TaskCompleted,
            "task.blocked" => EventType::TaskBlocked,
            "task.unblocked" => EventType::TaskUnblocked,
            "task.claimed" => EventType::TaskClaimed,
            "task.released" => EventType::TaskReleased,
            "dependency.added" => EventType::DependencyAdded,
            "dependency.removed" => EventType::DependencyRemoved,
            "comment.created" => EventType::CommentCreated,
            "comment.updated" => EventType::CommentUpdated,
            "session.started" => EventType::SessionStarted,
            "session.updated" => EventType::SessionUpdated,
            "session.closed" => EventType::SessionClosed,
            "session.scope_added" => EventType::SessionScopeAdded,
            "session.scope_removed" => EventType::SessionScopeRemoved,
            "session.focus_changed" => EventType::SessionFocusChanged,
            "checkpoint.created" => EventType::CheckpointCreated,
            "checkpoint.restored" => EventType::CheckpointRestored,
            "artifact.added" => EventType::ArtifactAdded,
            "artifact.removed" => EventType::ArtifactRemoved,
            other => EventType::Custom(other.to_string()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Project,
    Task,
    Comment,
    Session,
    Checkpoint,
    Artifact,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EntityType::Project => "project",
            EntityType::Task => "task",
            EntityType::Comment => "comment",
            EntityType::Session => "session",
            EntityType::Checkpoint => "checkpoint",
            EntityType::Artifact => "artifact",
        }
    }
}

impl std::str::FromStr for EntityType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "project" => Ok(EntityType::Project),
            "task" => Ok(EntityType::Task),
            "comment" => Ok(EntityType::Comment),
            "session" => Ok(EntityType::Session),
            "checkpoint" => Ok(EntityType::Checkpoint),
            "artifact" => Ok(EntityType::Artifact),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Event {
    pub id: i64,
    pub event_type: String,
    pub entity_type: String,
    pub entity_id: String,
    pub actor: Option<String>,
    pub session_id: Option<String>,
    pub payload: String, // JSON
    pub created_at: String,
}

impl Event {
    pub fn event_type_enum(&self) -> EventType {
        self.event_type.parse().unwrap()
    }

    pub fn entity_type_enum(&self) -> Option<EntityType> {
        self.entity_type.parse().ok()
    }

    pub fn payload_json(&self) -> serde_json::Value {
        serde_json::from_str(&self.payload).unwrap_or(serde_json::Value::Null)
    }
}

#[derive(Debug)]
pub struct CreateEvent {
    pub event_type: EventType,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub actor: Option<String>,
    pub session_id: Option<String>,
    pub payload: serde_json::Value,
}
