//! Search result types for granary.

use serde::{Deserialize, Serialize};

/// Search result item (can be an initiative, project, or task)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SearchResult {
    Initiative {
        id: String,
        name: String,
        description: Option<String>,
        status: String,
    },
    Project {
        id: String,
        name: String,
        description: Option<String>,
        status: String,
    },
    Task {
        id: String,
        title: String,
        description: Option<String>,
        status: String,
        priority: String,
        project_id: String,
    },
}

impl SearchResult {
    pub fn id(&self) -> &str {
        match self {
            SearchResult::Initiative { id, .. } => id,
            SearchResult::Project { id, .. } => id,
            SearchResult::Task { id, .. } => id,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            SearchResult::Initiative { name, .. } => name,
            SearchResult::Project { name, .. } => name,
            SearchResult::Task { title, .. } => title,
        }
    }

    pub fn entity_type(&self) -> &str {
        match self {
            SearchResult::Initiative { .. } => "initiative",
            SearchResult::Project { .. } => "project",
            SearchResult::Task { .. } => "task",
        }
    }
}
