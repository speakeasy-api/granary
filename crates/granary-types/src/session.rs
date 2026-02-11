use serde::{Deserialize, Serialize};

#[cfg(feature = "sqlx")]
use sqlx::FromRow;

/// Session mode enum with snake_case serialization.
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

/// Session as returned by granary CLI JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct Session {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub focus_task_id: Option<String>,
    #[serde(default)]
    pub variables: Option<String>, // JSON key/value
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub closed_at: Option<String>,
    #[serde(default)]
    pub last_edited_by: Option<String>,
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

/// Scope item type enum with snake_case serialization.
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

/// Session scope as returned by granary CLI JSON output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
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

/// Parameters for creating a new session.
#[derive(Debug, Default)]
pub struct CreateSession {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub mode: SessionMode,
}

/// Parameters for updating an existing session.
#[derive(Debug, Default)]
pub struct UpdateSession {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub mode: Option<SessionMode>,
    pub focus_task_id: Option<String>,
    pub variables: Option<std::collections::HashMap<String, String>>,
}
