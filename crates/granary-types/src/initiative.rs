use serde::{Deserialize, Serialize};

#[cfg(feature = "sqlx")]
use sqlx::FromRow;

/// Initiative status enum with snake_case serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum InitiativeStatus {
    #[default]
    Active,
    Archived,
}

impl InitiativeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            InitiativeStatus::Active => "active",
            InitiativeStatus::Archived => "archived",
        }
    }
}

impl std::fmt::Display for InitiativeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for InitiativeStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(InitiativeStatus::Active),
            "archived" => Ok(InitiativeStatus::Archived),
            _ => Err(()),
        }
    }
}

/// Initiative as returned by granary CLI JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct Initiative {
    pub id: String,
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    pub status: String,
    #[serde(default)]
    pub tags: Option<String>, // JSON array
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

impl Initiative {
    pub fn status_enum(&self) -> InitiativeStatus {
        self.status.parse().unwrap_or_default()
    }

    pub fn tags_vec(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }
}

/// Input for creating a new initiative.
#[derive(Debug, Default)]
pub struct CreateInitiative {
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub tags: Vec<String>,
}

/// Input for updating an existing initiative.
#[derive(Debug, Default)]
pub struct UpdateInitiative {
    pub name: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub status: Option<InitiativeStatus>,
    pub tags: Option<Vec<String>>,
}

/// Junction table model for initiative-project many-to-many relationship.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct InitiativeProject {
    pub initiative_id: String,
    pub project_id: String,
    pub added_at: String,
}

// === Initiative Summary Models ===

/// High-level summary of an initiative for orchestration scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiativeSummary {
    pub initiative: InitiativeInfo,
    pub status: InitiativeStatusSummary,
    pub projects: Vec<ProjectSummary>,
    pub blockers: Vec<InitiativeBlockerInfo>,
    pub next_actions: Vec<NextAction>,
}

/// Basic initiative identification info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiativeInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

/// Aggregated status counts for an initiative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiativeStatusSummary {
    pub total_projects: usize,
    pub completed_projects: usize,
    pub blocked_projects: usize,
    pub total_tasks: usize,
    pub tasks_done: usize,
    pub tasks_in_progress: usize,
    pub tasks_blocked: usize,
    pub tasks_todo: usize,
    pub percent_complete: f32,
}

/// Summary of a project within an initiative.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub task_count: usize,
    pub done_count: usize,
    pub blocked: bool,
    pub blocked_by: Vec<String>,
}

/// Blocker information for initiative summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiativeBlockerInfo {
    pub project_id: String,
    pub project_name: String,
    pub blocker_type: String,
    pub description: String,
}

/// Next actionable task for initiative summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAction {
    pub task_id: String,
    pub task_title: String,
    pub project_id: String,
    pub project_name: String,
    pub priority: String,
}
