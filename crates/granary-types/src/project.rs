use serde::{Deserialize, Serialize};

#[cfg(feature = "sqlx")]
use sqlx::FromRow;

/// Project status enum with snake_case serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    #[default]
    Active,
    Archived,
}

impl ProjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectStatus::Active => "active",
            ProjectStatus::Archived => "archived",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ProjectStatus::Active)
    }
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ProjectStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(ProjectStatus::Active),
            "archived" => Ok(ProjectStatus::Archived),
            _ => Err(()),
        }
    }
}

/// Project as returned by granary CLI JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    pub status: String,
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub default_session_policy: Option<String>,
    #[serde(default)]
    pub steering_refs: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

impl Project {
    pub fn status_enum(&self) -> ProjectStatus {
        self.status.parse().unwrap_or_default()
    }

    pub fn tags_vec(&self) -> Vec<String> {
        self.tags
            .as_ref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }

    pub fn steering_refs_vec(&self) -> Vec<String> {
        self.steering_refs
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }
}

#[derive(Debug, Default)]
pub struct CreateProject {
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub tags: Vec<String>,
    pub default_session_policy: Option<serde_json::Value>,
    pub steering_refs: Vec<String>,
}

#[derive(Debug, Default)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub status: Option<ProjectStatus>,
    pub tags: Option<Vec<String>>,
    pub default_session_policy: Option<serde_json::Value>,
    pub steering_refs: Option<Vec<String>>,
}

/// Represents a dependency relationship between two projects
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct ProjectDependency {
    pub project_id: String,
    pub depends_on_project_id: String,
    pub created_at: String,
}
