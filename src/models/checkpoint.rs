use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Checkpoint {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub snapshot: String, // JSON snapshot of session state
    pub created_at: String,
}

impl Checkpoint {
    pub fn snapshot_json(&self) -> serde_json::Value {
        serde_json::from_str(&self.snapshot).unwrap_or(serde_json::Value::Null)
    }
}

/// Snapshot of session state at checkpoint time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub session: SessionSnapshotData,
    pub scope: Vec<ScopeItem>,
    pub tasks: Vec<TaskSnapshot>,
    pub variables: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshotData {
    pub id: String,
    pub name: Option<String>,
    pub owner: Option<String>,
    pub mode: Option<String>,
    pub focus_task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeItem {
    pub item_type: String,
    pub item_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub id: String,
    pub status: String,
    pub priority: String,
    pub owner: Option<String>,
    pub blocked_reason: Option<String>,
    pub pinned: bool,
    pub focus_weight: i64,
}

#[derive(Debug)]
pub struct CreateCheckpoint {
    pub session_id: String,
    pub name: String,
}
