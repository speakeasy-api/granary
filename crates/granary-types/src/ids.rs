//! ID generation and parsing utilities for granary entities.
//!
//! This module provides functions for generating and parsing various entity IDs
//! used throughout granary (projects, tasks, initiatives, sessions, etc.).

use rand::RngExt;
use std::fmt;

/// Base32 alphabet (Crockford-style, excludes I, L, O, U to avoid confusion)
const BASE32_ALPHABET: &[u8] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// Error type for ID parsing operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdParseError {
    message: String,
}

impl IdParseError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for IdParseError {}

/// Result type for ID parsing operations
pub type IdParseResult<T> = std::result::Result<T, IdParseError>;

/// Generate a random suffix using base32 encoding
pub fn generate_suffix(len: usize) -> String {
    let mut rng = rand::rng();
    (0..len)
        .map(|_| BASE32_ALPHABET[rng.random_range(0..32)] as char)
        .collect()
}

/// Normalize a string to be used as a slug
/// - Lowercase
/// - Replace non-alphanumeric with hyphens
/// - Collapse multiple hyphens
/// - Trim leading/trailing hyphens
pub fn normalize_slug(s: &str) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse multiple hyphens and trim
    let mut result = String::new();
    let mut prev_hyphen = true; // Start true to skip leading hyphens
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push(c);
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }

    // Trim trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }

    result
}

/// Generate a project ID from a name
/// Format: <slug>-<suffix>
/// Example: "my-project-5h18"
pub fn generate_project_id(name: &str) -> String {
    let slug = normalize_slug(name);
    let suffix = generate_suffix(4);
    format!("{}-{}", slug, suffix)
}

/// Generate a task ID from a project ID and task number
/// Format: <project_id>-task-<n>
/// Example: "my-project-5h18-task-321"
pub fn generate_task_id(project_id: &str, task_number: i64) -> String {
    format!("{}-task-{}", project_id, task_number)
}

/// Generate a comment ID from a parent ID and comment number
/// Format: <parent_id>-comment-<n>
/// Example: "my-project-5h18-task-321-comment-2"
pub fn generate_comment_id(parent_id: &str, comment_number: i64) -> String {
    format!("{}-comment-{}", parent_id, comment_number)
}

/// Generate an artifact ID from a parent ID and artifact number
/// Format: <parent_id>-artifact-<n>
pub fn generate_artifact_id(parent_id: &str, artifact_number: i64) -> String {
    format!("{}-artifact-{}", parent_id, artifact_number)
}

/// Generate a session ID
/// Format: sess-<date>-<suffix>
/// Example: "sess-20260111-7f2c"
pub fn generate_session_id() -> String {
    let date = chrono::Utc::now().format("%Y%m%d");
    let suffix = generate_suffix(4);
    format!("sess-{}-{}", date, suffix)
}

/// Generate a checkpoint ID
/// Format: chkpt-<suffix>
pub fn generate_checkpoint_id() -> String {
    let suffix = generate_suffix(8);
    format!("chkpt-{}", suffix)
}

/// Generate a worker ID
/// Format: worker-<suffix>
/// Example: "worker-a3f8k2m1"
pub fn generate_worker_id() -> String {
    let suffix = generate_suffix(8);
    format!("worker-{}", suffix)
}

/// Generate a run ID
/// Format: run-<suffix>
/// Example: "run-a3f8k2m1"
pub fn generate_run_id() -> String {
    let suffix = generate_suffix(8);
    format!("run-{}", suffix)
}

/// Generate an initiative ID from a name
/// Format: <slug>-<suffix>
/// Example: "my-initiative-5h18"
pub fn generate_initiative_id(name: &str) -> String {
    let slug = normalize_slug(name);
    let suffix = generate_suffix(4);
    format!("{}-{}", slug, suffix)
}

/// Parse an initiative ID to extract the slug
pub fn parse_initiative_slug(initiative_id: &str) -> IdParseResult<&str> {
    // Initiative ID format: <slug>-<4char suffix>
    // Find the last hyphen that separates slug from suffix
    if initiative_id.len() < 5 {
        return Err(IdParseError::new(format!(
            "Initiative ID too short: {}",
            initiative_id
        )));
    }

    let suffix_start = initiative_id.len() - 4;
    if initiative_id.as_bytes()[suffix_start - 1] != b'-' {
        return Err(IdParseError::new(format!(
            "Invalid initiative ID format: {}",
            initiative_id
        )));
    }

    Ok(&initiative_id[..suffix_start - 1])
}

/// Parse a project ID to extract the slug
pub fn parse_project_slug(project_id: &str) -> IdParseResult<&str> {
    // Project ID format: <slug>-<4char suffix>
    // Find the last hyphen that separates slug from suffix
    if project_id.len() < 5 {
        return Err(IdParseError::new(format!(
            "Project ID too short: {}",
            project_id
        )));
    }

    let suffix_start = project_id.len() - 4;
    if project_id.as_bytes()[suffix_start - 1] != b'-' {
        return Err(IdParseError::new(format!(
            "Invalid project ID format: {}",
            project_id
        )));
    }

    Ok(&project_id[..suffix_start - 1])
}

/// Parse a task ID to extract the project ID and task number
pub fn parse_task_id(task_id: &str) -> IdParseResult<(&str, i64)> {
    // Task ID format: <project_id>-task-<n>
    let task_marker = "-task-";
    let pos = task_id
        .rfind(task_marker)
        .ok_or_else(|| IdParseError::new(format!("Invalid task ID format: {}", task_id)))?;

    let project_id = &task_id[..pos];
    let task_num_str = &task_id[pos + task_marker.len()..];
    let task_number = task_num_str
        .parse::<i64>()
        .map_err(|_| IdParseError::new(format!("Invalid task number in ID: {}", task_id)))?;

    Ok((project_id, task_number))
}

/// Parse a comment ID to extract the parent ID and comment number
pub fn parse_comment_id(comment_id: &str) -> IdParseResult<(&str, i64)> {
    // Comment ID format: <parent_id>-comment-<n>
    let comment_marker = "-comment-";
    let pos = comment_id
        .rfind(comment_marker)
        .ok_or_else(|| IdParseError::new(format!("Invalid comment ID format: {}", comment_id)))?;

    let parent_id = &comment_id[..pos];
    let comment_num_str = &comment_id[pos + comment_marker.len()..];
    let comment_number = comment_num_str
        .parse::<i64>()
        .map_err(|_| IdParseError::new(format!("Invalid comment number in ID: {}", comment_id)))?;

    Ok((parent_id, comment_number))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_slug() {
        assert_eq!(normalize_slug("My Big Project"), "my-big-project");
        assert_eq!(normalize_slug("  test  "), "test");
        assert_eq!(normalize_slug("foo--bar"), "foo-bar");
        assert_eq!(normalize_slug("Hello World!"), "hello-world");
    }

    #[test]
    fn test_generate_project_id() {
        let id = generate_project_id("My Project");
        assert!(id.starts_with("my-project-"));
        assert_eq!(id.len(), "my-project-".len() + 4);
    }

    #[test]
    fn test_generate_task_id() {
        let id = generate_task_id("my-project-5h18", 42);
        assert_eq!(id, "my-project-5h18-task-42");
    }

    #[test]
    fn test_parse_task_id() {
        let (project_id, task_num) = parse_task_id("my-project-5h18-task-42").unwrap();
        assert_eq!(project_id, "my-project-5h18");
        assert_eq!(task_num, 42);
    }

    #[test]
    fn test_parse_comment_id() {
        let (parent_id, comment_num) =
            parse_comment_id("my-project-5h18-task-42-comment-3").unwrap();
        assert_eq!(parent_id, "my-project-5h18-task-42");
        assert_eq!(comment_num, 3);
    }

    #[test]
    fn test_generate_initiative_id() {
        let id = generate_initiative_id("My Initiative");
        assert!(id.starts_with("my-initiative-"));
        assert_eq!(id.len(), "my-initiative-".len() + 4);
    }

    #[test]
    fn test_parse_initiative_slug() {
        let slug = parse_initiative_slug("my-initiative-5h18").unwrap();
        assert_eq!(slug, "my-initiative");
    }

    #[test]
    fn test_generate_worker_id() {
        let id = generate_worker_id();
        assert!(id.starts_with("worker-"));
        assert_eq!(id.len(), "worker-".len() + 8);
    }

    #[test]
    fn test_generate_run_id() {
        let id = generate_run_id();
        assert!(id.starts_with("run-"));
        assert_eq!(id.len(), "run-".len() + 8);
    }

    #[test]
    fn test_parse_project_slug() {
        let slug = parse_project_slug("my-project-5h18").unwrap();
        assert_eq!(slug, "my-project");
    }

    #[test]
    fn test_id_parse_error_display() {
        let err = IdParseError::new("test error");
        assert_eq!(format!("{}", err), "test error");
    }
}
