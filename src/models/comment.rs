use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CommentKind {
    #[default]
    Note,
    Progress,
    Decision,
    Blocker,
    Handoff,
    Incident,
    Context,
}

impl CommentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            CommentKind::Note => "note",
            CommentKind::Progress => "progress",
            CommentKind::Decision => "decision",
            CommentKind::Blocker => "blocker",
            CommentKind::Handoff => "handoff",
            CommentKind::Incident => "incident",
            CommentKind::Context => "context",
        }
    }

    pub fn all() -> &'static [CommentKind] {
        &[
            CommentKind::Note,
            CommentKind::Progress,
            CommentKind::Decision,
            CommentKind::Blocker,
            CommentKind::Handoff,
            CommentKind::Incident,
            CommentKind::Context,
        ]
    }
}

impl std::str::FromStr for CommentKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "note" => Ok(CommentKind::Note),
            "progress" => Ok(CommentKind::Progress),
            "decision" => Ok(CommentKind::Decision),
            "blocker" => Ok(CommentKind::Blocker),
            "handoff" => Ok(CommentKind::Handoff),
            "incident" => Ok(CommentKind::Incident),
            "context" => Ok(CommentKind::Context),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ParentType {
    Project,
    #[default]
    Task,
    Comment,
}

impl ParentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParentType::Project => "project",
            ParentType::Task => "task",
            ParentType::Comment => "comment",
        }
    }
}

impl std::str::FromStr for ParentType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "project" => Ok(ParentType::Project),
            "task" => Ok(ParentType::Task),
            "comment" => Ok(ParentType::Comment),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Comment {
    pub id: String,
    pub parent_type: String,
    pub parent_id: String,
    pub comment_number: i64,
    pub kind: String,
    pub content: String,
    pub author: Option<String>,
    pub meta: Option<String>, // JSON
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
}

impl Comment {
    pub fn kind_enum(&self) -> CommentKind {
        self.kind.parse().unwrap_or_default()
    }

    pub fn parent_type_enum(&self) -> Option<ParentType> {
        self.parent_type.parse().ok()
    }

    pub fn meta_json(&self) -> Option<serde_json::Value> {
        self.meta
            .as_ref()
            .and_then(|m| serde_json::from_str(m).ok())
    }
}

#[derive(Debug, Default)]
pub struct CreateComment {
    pub parent_type: ParentType,
    pub parent_id: String,
    pub kind: CommentKind,
    pub content: String,
    pub author: Option<String>,
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Default)]
pub struct UpdateComment {
    pub content: Option<String>,
    pub kind: Option<CommentKind>,
    pub meta: Option<serde_json::Value>,
}
