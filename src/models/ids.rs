//! ID generation and parsing utilities.
//!
//! This module re-exports ID utilities from `granary_types` and provides
//! wrapper functions that convert errors to `GranaryError`.

// Re-export all generator functions directly from granary_types
pub use granary_types::ids::{
    IdParseError, generate_artifact_id, generate_checkpoint_id, generate_comment_id,
    generate_initiative_id, generate_project_id, generate_run_id, generate_session_id,
    generate_suffix, generate_task_id, generate_worker_id, normalize_slug,
};

use crate::error::{GranaryError, Result};

/// Parse an initiative ID to extract the slug
pub fn parse_initiative_slug(initiative_id: &str) -> Result<&str> {
    granary_types::ids::parse_initiative_slug(initiative_id)
        .map_err(|e| GranaryError::InvalidId(e.to_string()))
}

/// Parse a project ID to extract the slug
pub fn parse_project_slug(project_id: &str) -> Result<&str> {
    granary_types::ids::parse_project_slug(project_id)
        .map_err(|e| GranaryError::InvalidId(e.to_string()))
}

/// Parse a task ID to extract the project ID and task number
pub fn parse_task_id(task_id: &str) -> Result<(&str, i64)> {
    granary_types::ids::parse_task_id(task_id).map_err(|e| GranaryError::InvalidId(e.to_string()))
}

/// Parse a comment ID to extract the parent ID and comment number
pub fn parse_comment_id(comment_id: &str) -> Result<(&str, i64)> {
    granary_types::ids::parse_comment_id(comment_id)
        .map_err(|e| GranaryError::InvalidId(e.to_string()))
}
