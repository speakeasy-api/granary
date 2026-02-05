//! Template substitution for runner arguments.
//!
//! This module provides functionality to substitute placeholders in runner
//! arguments with actual values from events. Placeholders are in the format
//! `{path.to.value}` and are resolved from event payload data.
//!
//! Supported placeholders:
//! - `{event.id}` - The event ID
//! - `{event.type}` - The event type
//! - `{task.id}`, `{task.title}`, etc. - Task fields from the event payload
//! - `{project.id}`, `{project.name}`, etc. - Project fields from the event payload
//! - `{field}` - Top-level fields from the event payload

use crate::error::Result;
use crate::models::Event;
use serde_json::Value;

/// Placeholder pattern for template substitution
/// Matches patterns like {event.id}, {task.title}, {project.name}
const PLACEHOLDER_START: char = '{';
const PLACEHOLDER_END: char = '}';

/// Substitute placeholders in a template string with values from an event.
///
/// # Arguments
/// * `template` - The template string containing placeholders like `{task.id}`
/// * `event` - The event containing the payload data
///
/// # Returns
/// The template string with all placeholders replaced with their values.
/// Unknown placeholders are replaced with an empty string.
///
/// # Examples
/// ```ignore
/// let result = substitute("granary execute {task.id}", &event)?;
/// // Result: "granary execute project-abc-task-1"
/// ```
pub fn substitute(template: &str, event: &Event) -> Result<String> {
    let payload: Value = serde_json::from_str(&event.payload)?;
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == PLACEHOLDER_START {
            // Collect the placeholder path
            let mut path = String::new();
            while let Some(&next) = chars.peek() {
                if next == PLACEHOLDER_END {
                    chars.next(); // consume the closing brace
                    break;
                }
                path.push(chars.next().unwrap());
            }

            // Resolve the path and append the value
            if let Some(value) = resolve_path(&payload, &path, event) {
                result.push_str(&value);
            }
            // If not found, we append nothing (empty string)
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

/// Substitute placeholders in a list of template strings.
///
/// # Arguments
/// * `templates` - A list of template strings
/// * `event` - The event containing the payload data
///
/// # Returns
/// A list of strings with all placeholders replaced.
pub fn substitute_all(templates: &[String], event: &Event) -> Result<Vec<String>> {
    templates.iter().map(|t| substitute(t, event)).collect()
}

/// Resolve a path to a value from the event payload.
///
/// # Arguments
/// * `payload` - The JSON payload from the event
/// * `path` - The path to resolve (e.g., "task.id", "event.type")
/// * `event` - The original event (for event-level fields)
///
/// # Returns
/// The resolved value as a string, or None if not found.
fn resolve_path(payload: &Value, path: &str, event: &Event) -> Option<String> {
    // Handle special event-level paths first
    match path {
        "event.id" => return Some(event.id.to_string()),
        "event.type" => return Some(event.event_type.clone()),
        "event.entity_type" => return Some(event.entity_type.clone()),
        "event.entity_id" => return Some(event.entity_id.clone()),
        "event.created_at" => return Some(event.created_at.clone()),
        _ => {}
    }

    // Handle nested paths (e.g., "task.id", "project.name")
    if let Some(field) = path.strip_prefix("task.") {
        // Skip "task."
        return resolve_nested_path(payload.get("task"), field);
    }

    if let Some(field) = path.strip_prefix("project.") {
        // Skip "project."
        return resolve_nested_path(payload.get("project"), field);
    }

    if let Some(field) = path.strip_prefix("session.") {
        // Skip "session."
        return resolve_nested_path(payload.get("session"), field);
    }

    // Handle direct top-level paths
    resolve_nested_path(Some(payload), path)
}

/// Resolve a nested path within a JSON value.
///
/// # Arguments
/// * `value` - The JSON value to search within
/// * `path` - The path to resolve (can be nested with dots)
///
/// # Returns
/// The resolved value as a string, or None if not found.
fn resolve_nested_path(value: Option<&Value>, path: &str) -> Option<String> {
    let value = value?;

    // Split path by dots for nested access
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts {
        match current {
            Value::Object(map) => {
                current = map.get(part)?;
            }
            Value::Array(arr) => {
                // Support array indexing like "items.0.name"
                if let Ok(index) = part.parse::<usize>() {
                    current = arr.get(index)?;
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }

    // Convert the final value to a string
    value_to_string(current)
}

/// Convert a JSON value to a string representation.
fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        // For arrays and objects, return JSON representation
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(payload: &str) -> Event {
        Event {
            id: 42,
            event_type: "task.started".to_string(),
            entity_type: "task".to_string(),
            entity_id: "test-task-1".to_string(),
            actor: Some("test-actor".to_string()),
            session_id: None,
            payload: payload.to_string(),
            created_at: "2024-01-15T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_substitute_event_fields() {
        let event = create_test_event("{}");

        assert_eq!(substitute("{event.id}", &event).unwrap(), "42");
        assert_eq!(substitute("{event.type}", &event).unwrap(), "task.started");
        assert_eq!(
            substitute("{event.entity_id}", &event).unwrap(),
            "test-task-1"
        );
    }

    #[test]
    fn test_substitute_task_fields() {
        let payload =
            r#"{"task": {"id": "proj-abc-task-1", "title": "Test Task", "status": "in_progress"}}"#;
        let event = create_test_event(payload);

        assert_eq!(substitute("{task.id}", &event).unwrap(), "proj-abc-task-1");
        assert_eq!(substitute("{task.title}", &event).unwrap(), "Test Task");
        assert_eq!(substitute("{task.status}", &event).unwrap(), "in_progress");
    }

    #[test]
    fn test_substitute_project_fields() {
        let payload = r#"{"project": {"id": "proj-xyz", "name": "My Project"}}"#;
        let event = create_test_event(payload);

        assert_eq!(substitute("{project.id}", &event).unwrap(), "proj-xyz");
        assert_eq!(substitute("{project.name}", &event).unwrap(), "My Project");
    }

    #[test]
    fn test_substitute_mixed_template() {
        let payload = r#"{"task": {"id": "task-1"}, "project": {"id": "proj-1"}}"#;
        let event = create_test_event(payload);

        let result = substitute(
            "granary execute --task {task.id} --project {project.id}",
            &event,
        )
        .unwrap();
        assert_eq!(result, "granary execute --task task-1 --project proj-1");
    }

    #[test]
    fn test_substitute_unknown_placeholder() {
        let event = create_test_event("{}");

        // Unknown placeholders become empty strings
        assert_eq!(substitute("{unknown.field}", &event).unwrap(), "");
        assert_eq!(
            substitute("prefix-{unknown}-suffix", &event).unwrap(),
            "prefix--suffix"
        );
    }

    #[test]
    fn test_substitute_no_placeholders() {
        let event = create_test_event("{}");

        assert_eq!(
            substitute("no placeholders here", &event).unwrap(),
            "no placeholders here"
        );
    }

    #[test]
    fn test_substitute_all() {
        let payload = r#"{"task": {"id": "task-1"}}"#;
        let event = create_test_event(payload);

        let templates = vec![
            "arg1".to_string(),
            "{task.id}".to_string(),
            "--event={event.id}".to_string(),
        ];

        let result = substitute_all(&templates, &event).unwrap();
        assert_eq!(result, vec!["arg1", "task-1", "--event=42"]);
    }

    #[test]
    fn test_substitute_numeric_values() {
        let payload = r#"{"count": 42, "enabled": true}"#;
        let event = create_test_event(payload);

        assert_eq!(substitute("{count}", &event).unwrap(), "42");
        assert_eq!(substitute("{enabled}", &event).unwrap(), "true");
    }
}
