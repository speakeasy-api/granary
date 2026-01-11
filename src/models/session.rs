use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SessionMode {
    #[default]
    Execute,
    Plan,
    Review,
}

impl SessionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionMode::Plan => "plan",
            SessionMode::Execute => "execute",
            SessionMode::Review => "review",
        }
    }
}

impl std::str::FromStr for SessionMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "plan" => Ok(SessionMode::Plan),
            "execute" => Ok(SessionMode::Execute),
            "review" => Ok(SessionMode::Review),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: String,
    pub name: Option<String>,
    pub owner: Option<String>,
    pub mode: Option<String>,
    pub focus_task_id: Option<String>,
    pub variables: Option<String>, // JSON key/value
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

impl Session {
    pub fn mode_enum(&self) -> SessionMode {
        self.mode
            .as_ref()
            .and_then(|m| m.parse().ok())
            .unwrap_or_default()
    }

    pub fn variables_map(&self) -> std::collections::HashMap<String, String> {
        self.variables
            .as_ref()
            .and_then(|v| serde_json::from_str(v).ok())
            .unwrap_or_default()
    }

    pub fn is_closed(&self) -> bool {
        self.closed_at.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeItemType {
    Project,
    Task,
    Comment,
    Artifact,
}

impl ScopeItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScopeItemType::Project => "project",
            ScopeItemType::Task => "task",
            ScopeItemType::Comment => "comment",
            ScopeItemType::Artifact => "artifact",
        }
    }
}

impl std::str::FromStr for ScopeItemType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "project" => Ok(ScopeItemType::Project),
            "task" => Ok(ScopeItemType::Task),
            "comment" => Ok(ScopeItemType::Comment),
            "artifact" => Ok(ScopeItemType::Artifact),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SessionScope {
    pub session_id: String,
    pub item_type: String,
    pub item_id: String,
    pub pinned_at: String,
}

impl SessionScope {
    pub fn item_type_enum(&self) -> Option<ScopeItemType> {
        self.item_type.parse().ok()
    }
}

#[derive(Debug, Default)]
pub struct CreateSession {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub mode: SessionMode,
}

#[derive(Debug, Default)]
pub struct UpdateSession {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub mode: Option<SessionMode>,
    pub focus_task_id: Option<String>,
    pub variables: Option<std::collections::HashMap<String, String>>,
}
