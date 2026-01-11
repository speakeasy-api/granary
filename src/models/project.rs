use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub status: String,
    pub tags: Option<String>,                   // JSON array
    pub default_session_policy: Option<String>, // JSON
    pub steering_refs: Option<String>,          // JSON array
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
