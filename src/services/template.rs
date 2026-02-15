//! Template substitution for runner arguments.
//!
//! This module provides functionality to substitute placeholders in runner
//! arguments with actual values from events. Placeholders are in the format
//! `{path.to.value}` and are resolved from event payload data.
//!
//! Supported placeholders:
//! - `{event.id}` - The event ID
//! - `{event.type}` - The event type
//! - `{event.entity_id}` - The entity ID from the event row
//! - `{field}` - Top-level fields from the event payload (e.g., `{id}`, `{title}`, `{priority}`)
//! - `{task.field}` - Nested lookup under a `task` key in the payload (if present)
//! - `{project.field}` - Nested lookup under a `project` key in the payload (if present)
//! - `{session.field}` - Nested lookup under a `session` key in the payload (if present)
//!
//! Pipeline templates (resolved only when a `PipelineContext` is provided):
//! - `{steps.<name>.stdout}` - Captured stdout of a named step (trimmed)
//! - `{steps.<name>.exit_code}` - Exit code of a named step
//! - `{prev.stdout}` - Stdout of the immediately preceding step
//! - `{prev.exit_code}` - Exit code of the immediately preceding step
//!
//! Note: Most event payloads use flat top-level fields (e.g., `{id}`, `{title}`),
//! not nested objects. Use `{event.entity_id}` for the most reliable entity ID access.

use std::collections::HashMap;

use crate::error::Result;
use crate::models::Event;
use serde_json::Value;

/// Output captured from a completed pipeline step.
#[derive(Debug, Clone)]
pub struct StepOutput {
    pub stdout: String,
    pub exit_code: i32,
}

/// Accumulated outputs from completed pipeline steps.
#[derive(Debug, Clone, Default)]
pub struct PipelineContext {
    outputs: HashMap<String, StepOutput>,
    last_step: Option<String>,
}

impl PipelineContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a step's output, making it available for subsequent template resolution.
    pub fn add_step(&mut self, name: String, output: StepOutput) {
        self.last_step = Some(name.clone());
        self.outputs.insert(name, output);
    }

    /// Resolve a pipeline template path.
    ///
    /// Handles `steps.<name>.stdout`, `steps.<name>.exit_code`,
    /// `prev.stdout`, and `prev.exit_code`.
    ///
    /// Step names may contain `/` and `-`, so we strip known suffixes
    /// from the right rather than splitting on `.`.
    pub fn resolve(&self, path: &str) -> Option<String> {
        if let Some(rest) = path.strip_prefix("steps.") {
            if let Some(name) = rest.strip_suffix(".stdout") {
                return self.outputs.get(name).map(|o| o.stdout.clone());
            }
            if let Some(name) = rest.strip_suffix(".exit_code") {
                return self.outputs.get(name).map(|o| o.exit_code.to_string());
            }
            None
        } else if let Some(field) = path.strip_prefix("prev.") {
            let last = self.last_step.as_ref()?;
            let output = self.outputs.get(last)?;
            match field {
                "stdout" => Some(output.stdout.clone()),
                "exit_code" => Some(output.exit_code.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }
}

/// Placeholder pattern for template substitution
/// Matches patterns like {event.id}, {task.title}, {project.name}
const PLACEHOLDER_START: char = '{';
const PLACEHOLDER_END: char = '}';

/// Substitute placeholders in a template string with values from an event.
///
/// This is a convenience wrapper around [`substitute_with_context`] that passes
/// `None` for the pipeline context. All existing call sites remain unchanged.
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
    substitute_with_context(template, event, None)
}

/// Substitute placeholders in a template string with values from an event,
/// optionally checking a pipeline context first.
///
/// Pipeline templates (`{steps.<name>.stdout}`, `{prev.exit_code}`, etc.) are
/// checked before event-based resolution, so they take priority.
///
/// # Arguments
/// * `template` - The template string containing placeholders
/// * `event` - The event containing the payload data
/// * `pipeline_ctx` - Optional pipeline context for step output resolution
///
/// # Returns
/// The template string with all placeholders replaced with their values.
/// Unknown placeholders are replaced with an empty string.
pub fn substitute_with_context(
    template: &str,
    event: &Event,
    pipeline_ctx: Option<&PipelineContext>,
) -> Result<String> {
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

            // Try pipeline context first
            if let Some(ctx) = pipeline_ctx
                && let Some(value) = ctx.resolve(&path)
            {
                result.push_str(&value);
                continue;
            }
            // Fall through to event-based resolution
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
    substitute_all_with_context(templates, event, None)
}

/// Substitute placeholders in a list of template strings, with optional pipeline context.
///
/// # Arguments
/// * `templates` - A list of template strings
/// * `event` - The event containing the payload data
/// * `pipeline_ctx` - Optional pipeline context for step output resolution
///
/// # Returns
/// A list of strings with all placeholders replaced.
pub fn substitute_all_with_context(
    templates: &[String],
    event: &Event,
    pipeline_ctx: Option<&PipelineContext>,
) -> Result<Vec<String>> {
    templates
        .iter()
        .map(|t| substitute_with_context(t, event, pipeline_ctx))
        .collect()
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

    // --- PipelineContext unit tests ---

    #[test]
    fn test_pipeline_context_resolve_steps_stdout() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "git/worktree-create".to_string(),
            StepOutput {
                stdout: "/tmp/granary-proj-abc1-task-3".to_string(),
                exit_code: 0,
            },
        );

        assert_eq!(
            ctx.resolve("steps.git/worktree-create.stdout"),
            Some("/tmp/granary-proj-abc1-task-3".to_string())
        );
    }

    #[test]
    fn test_pipeline_context_resolve_steps_exit_code() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "git/worktree-create".to_string(),
            StepOutput {
                stdout: "/tmp/path".to_string(),
                exit_code: 0,
            },
        );

        assert_eq!(
            ctx.resolve("steps.git/worktree-create.exit_code"),
            Some("0".to_string())
        );
    }

    #[test]
    fn test_pipeline_context_resolve_steps_nonzero_exit() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "build".to_string(),
            StepOutput {
                stdout: "".to_string(),
                exit_code: 1,
            },
        );

        assert_eq!(ctx.resolve("steps.build.exit_code"), Some("1".to_string()));
    }

    #[test]
    fn test_pipeline_context_resolve_prev_stdout() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "step-a".to_string(),
            StepOutput {
                stdout: "output-a".to_string(),
                exit_code: 0,
            },
        );
        ctx.add_step(
            "step-b".to_string(),
            StepOutput {
                stdout: "output-b".to_string(),
                exit_code: 0,
            },
        );

        assert_eq!(ctx.resolve("prev.stdout"), Some("output-b".to_string()));
    }

    #[test]
    fn test_pipeline_context_resolve_prev_exit_code() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "first".to_string(),
            StepOutput {
                stdout: "".to_string(),
                exit_code: 42,
            },
        );

        assert_eq!(ctx.resolve("prev.exit_code"), Some("42".to_string()));
    }

    #[test]
    fn test_pipeline_context_resolve_prev_tracks_last_added() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "first".to_string(),
            StepOutput {
                stdout: "first-out".to_string(),
                exit_code: 0,
            },
        );
        assert_eq!(ctx.resolve("prev.stdout"), Some("first-out".to_string()));

        ctx.add_step(
            "second".to_string(),
            StepOutput {
                stdout: "second-out".to_string(),
                exit_code: 0,
            },
        );
        assert_eq!(ctx.resolve("prev.stdout"), Some("second-out".to_string()));
    }

    #[test]
    fn test_pipeline_context_resolve_unknown_step() {
        let ctx = PipelineContext::new();

        assert_eq!(ctx.resolve("steps.nonexistent.stdout"), None);
    }

    #[test]
    fn test_pipeline_context_resolve_prev_empty_context() {
        let ctx = PipelineContext::new();

        assert_eq!(ctx.resolve("prev.stdout"), None);
        assert_eq!(ctx.resolve("prev.exit_code"), None);
    }

    #[test]
    fn test_pipeline_context_resolve_unknown_field() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "test".to_string(),
            StepOutput {
                stdout: "out".to_string(),
                exit_code: 0,
            },
        );

        // Unknown suffix - neither .stdout nor .exit_code
        assert_eq!(ctx.resolve("steps.test.stderr"), None);
        assert_eq!(ctx.resolve("prev.stderr"), None);
    }

    #[test]
    fn test_pipeline_context_resolve_no_prefix_match() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "test".to_string(),
            StepOutput {
                stdout: "out".to_string(),
                exit_code: 0,
            },
        );

        // Not a pipeline path at all
        assert_eq!(ctx.resolve("task.id"), None);
        assert_eq!(ctx.resolve("event.type"), None);
    }

    #[test]
    fn test_pipeline_context_namespaced_step_with_dashes() {
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "agents/claude-work".to_string(),
            StepOutput {
                stdout: "claude output".to_string(),
                exit_code: 0,
            },
        );

        assert_eq!(
            ctx.resolve("steps.agents/claude-work.stdout"),
            Some("claude output".to_string())
        );
        assert_eq!(
            ctx.resolve("steps.agents/claude-work.exit_code"),
            Some("0".to_string())
        );
    }

    // --- substitute_with_context tests ---

    #[test]
    fn test_substitute_with_context_pipeline_templates() {
        let event = create_test_event(r#"{"id": "task-1"}"#);
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "git/worktree-create".to_string(),
            StepOutput {
                stdout: "/tmp/worktree".to_string(),
                exit_code: 0,
            },
        );

        let result = substitute_with_context(
            "cd {steps.git/worktree-create.stdout} && run {id}",
            &event,
            Some(&ctx),
        )
        .unwrap();
        assert_eq!(result, "cd /tmp/worktree && run task-1");
    }

    #[test]
    fn test_substitute_with_context_prev_templates() {
        let event = create_test_event(r#"{}"#);
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "step1".to_string(),
            StepOutput {
                stdout: "hello world".to_string(),
                exit_code: 0,
            },
        );

        assert_eq!(
            substitute_with_context("{prev.stdout}", &event, Some(&ctx)).unwrap(),
            "hello world"
        );
        assert_eq!(
            substitute_with_context("{prev.exit_code}", &event, Some(&ctx)).unwrap(),
            "0"
        );
    }

    #[test]
    fn test_substitute_with_context_pipeline_takes_priority() {
        // If there's a pipeline context, it's checked first
        let event = create_test_event(r#"{"steps": "event-value"}"#);
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "x".to_string(),
            StepOutput {
                stdout: "pipeline-value".to_string(),
                exit_code: 0,
            },
        );

        // {steps.x.stdout} resolves from pipeline, not event payload
        let result = substitute_with_context("{steps.x.stdout}", &event, Some(&ctx)).unwrap();
        assert_eq!(result, "pipeline-value");
    }

    #[test]
    fn test_substitute_with_context_none_falls_through() {
        // With None pipeline context, behaves exactly like substitute
        let event = create_test_event(r#"{"id": "task-1"}"#);

        let result = substitute_with_context("{id}", &event, None).unwrap();
        assert_eq!(result, "task-1");

        // Pipeline templates resolve to empty with no context
        let result = substitute_with_context("{steps.x.stdout}", &event, None).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_substitute_with_context_mixed_pipeline_and_event() {
        let event = create_test_event(r#"{"id": "task-5"}"#);
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "notify/macos".to_string(),
            StepOutput {
                stdout: "notified".to_string(),
                exit_code: 0,
            },
        );

        let result = substitute_with_context(
            "Task {id}: prev={prev.stdout}, exit={steps.notify/macos.exit_code}",
            &event,
            Some(&ctx),
        )
        .unwrap();
        assert_eq!(result, "Task task-5: prev=notified, exit=0");
    }

    #[test]
    fn test_substitute_all_with_context() {
        let event = create_test_event(r#"{"id": "task-1"}"#);
        let mut ctx = PipelineContext::new();
        ctx.add_step(
            "build".to_string(),
            StepOutput {
                stdout: "ok".to_string(),
                exit_code: 0,
            },
        );

        let templates = vec![
            "{id}".to_string(),
            "{prev.stdout}".to_string(),
            "--exit={steps.build.exit_code}".to_string(),
        ];

        let result = substitute_all_with_context(&templates, &event, Some(&ctx)).unwrap();
        assert_eq!(result, vec!["task-1", "ok", "--exit=0"]);
    }
}
