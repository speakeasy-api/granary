use serde::{Deserialize, Serialize};

#[cfg(feature = "sqlx")]
use sqlx::FromRow;

/// Project status enum with snake_case serialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    #[default]
    Active,
    InReview,
    Completed,
    Archived,
}

impl ProjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectStatus::Active => "active",
            ProjectStatus::InReview => "in_review",
            ProjectStatus::Completed => "completed",
            ProjectStatus::Archived => "archived",
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self, ProjectStatus::Active)
    }

    pub fn is_in_review(&self) -> bool {
        matches!(self, ProjectStatus::InReview)
    }

    pub fn is_completed(&self) -> bool {
        matches!(self, ProjectStatus::Completed)
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
            "in_review" | "in-review" | "inreview" => Ok(ProjectStatus::InReview),
            "completed" => Ok(ProjectStatus::Completed),
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
    #[serde(default)]
    pub last_edited_by: Option<String>,
    #[serde(default)]
    pub metadata: Option<String>,
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

    pub fn metadata_value(&self) -> Option<serde_json::Value> {
        self.metadata
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
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
    pub metadata: Option<serde_json::Value>,
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
    pub metadata: Option<serde_json::Value>,
}

/// Represents a dependency relationship between two projects
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct ProjectDependency {
    pub project_id: String,
    pub depends_on_project_id: String,
    pub created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_project() -> Project {
        Project {
            id: "proj-1".to_string(),
            slug: "proj-1".to_string(),
            name: "Test".to_string(),
            description: None,
            owner: None,
            status: "active".to_string(),
            tags: None,
            default_session_policy: None,
            steering_refs: None,
            metadata: None,
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            version: 1,
            last_edited_by: None,
        }
    }

    #[test]
    fn metadata_value_returns_none_when_absent() {
        let project = make_project();
        assert!(project.metadata_value().is_none());
    }

    #[test]
    fn metadata_value_parses_valid_json() {
        let mut project = make_project();
        project.metadata = Some(r#"{"team":"platform"}"#.to_string());
        let val = project.metadata_value().unwrap();
        assert_eq!(val["team"], "platform");
    }

    #[test]
    fn metadata_value_returns_none_for_invalid_json() {
        let mut project = make_project();
        project.metadata = Some("broken".to_string());
        assert!(project.metadata_value().is_none());
    }

    #[test]
    fn metadata_deserialized_with_default_when_missing() {
        let json = r#"{
            "id": "p1", "slug": "p1", "name": "Test",
            "status": "active",
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "version": 1
        }"#;
        let project: Project = serde_json::from_str(json).unwrap();
        assert!(project.metadata.is_none());
    }
}
