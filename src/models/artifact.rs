use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    #[default]
    File,
    Url,
    GitRef,
    Log,
}

impl ArtifactType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArtifactType::File => "file",
            ArtifactType::Url => "url",
            ArtifactType::GitRef => "git_ref",
            ArtifactType::Log => "log",
        }
    }
}

impl std::str::FromStr for ArtifactType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "file" => Ok(ArtifactType::File),
            "url" => Ok(ArtifactType::Url),
            "git_ref" | "git-ref" | "gitref" => Ok(ArtifactType::GitRef),
            "log" => Ok(ArtifactType::Log),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactParentType {
    Project,
    #[default]
    Task,
    Comment,
}

impl ArtifactParentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArtifactParentType::Project => "project",
            ArtifactParentType::Task => "task",
            ArtifactParentType::Comment => "comment",
        }
    }
}

impl std::str::FromStr for ArtifactParentType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "project" => Ok(ArtifactParentType::Project),
            "task" => Ok(ArtifactParentType::Task),
            "comment" => Ok(ArtifactParentType::Comment),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Artifact {
    pub id: String,
    pub parent_type: String,
    pub parent_id: String,
    pub artifact_number: i64,
    pub artifact_type: String,
    pub path_or_url: String,
    pub description: Option<String>,
    pub meta: Option<String>, // JSON
    pub created_at: String,
}

impl Artifact {
    pub fn artifact_type_enum(&self) -> ArtifactType {
        self.artifact_type.parse().unwrap_or_default()
    }

    pub fn parent_type_enum(&self) -> Option<ArtifactParentType> {
        self.parent_type.parse().ok()
    }

    pub fn meta_json(&self) -> Option<serde_json::Value> {
        self.meta
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
    }
}

#[derive(Debug, Default)]
pub struct CreateArtifact {
    pub parent_type: ArtifactParentType,
    pub parent_id: String,
    pub artifact_type: ArtifactType,
    pub path_or_url: String,
    pub description: Option<String>,
    pub meta: Option<serde_json::Value>,
}
