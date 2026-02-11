//! Additional tests for the template module.
//!
//! These tests complement the inline tests in template.rs with additional
//! edge cases and integration-style tests.

#[cfg(test)]
mod tests {
    use crate::models::Event;
    use crate::services::template::{substitute, substitute_all};

    /// Helper to create a test event with custom payload
    fn create_event(payload: &str) -> Event {
        Event {
            id: 123,
            event_type: "task.unblocked".to_string(),
            entity_type: "task".to_string(),
            entity_id: "my-project-abc1-task-5".to_string(),
            actor: Some("system".to_string()),
            session_id: Some("sess-20260115-xyz1".to_string()),
            payload: payload.to_string(),
            created_at: "2026-01-15T10:30:00Z".to_string(),
        }
    }

    // ==========================================
    // Basic Substitution
    // ==========================================

    #[test]
    fn test_substitute_simple_task_id() {
        let payload = r#"{"task": {"id": "proj-abc-task-1"}}"#;
        let event = create_event(payload);

        let result = substitute("{task.id}", &event).unwrap();
        assert_eq!(result, "proj-abc-task-1");
    }

    #[test]
    fn test_substitute_no_placeholders() {
        let event = create_event("{}");
        let result = substitute("literal text only", &event).unwrap();
        assert_eq!(result, "literal text only");
    }

    #[test]
    fn test_substitute_empty_template() {
        let event = create_event("{}");
        let result = substitute("", &event).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_substitute_preserves_surrounding_text() {
        let payload = r#"{"task": {"id": "task-42"}}"#;
        let event = create_event(payload);

        let result = substitute("prefix-{task.id}-suffix", &event).unwrap();
        assert_eq!(result, "prefix-task-42-suffix");
    }

    // ==========================================
    // Event Fields
    // ==========================================

    #[test]
    fn test_substitute_event_id() {
        let event = create_event("{}");
        let result = substitute("{event.id}", &event).unwrap();
        assert_eq!(result, "123");
    }

    #[test]
    fn test_substitute_event_type() {
        let event = create_event("{}");
        let result = substitute("{event.type}", &event).unwrap();
        assert_eq!(result, "task.unblocked");
    }

    #[test]
    fn test_substitute_event_entity_type() {
        let event = create_event("{}");
        let result = substitute("{event.entity_type}", &event).unwrap();
        assert_eq!(result, "task");
    }

    #[test]
    fn test_substitute_event_entity_id() {
        let event = create_event("{}");
        let result = substitute("{event.entity_id}", &event).unwrap();
        assert_eq!(result, "my-project-abc1-task-5");
    }

    #[test]
    fn test_substitute_event_created_at() {
        let event = create_event("{}");
        let result = substitute("{event.created_at}", &event).unwrap();
        assert_eq!(result, "2026-01-15T10:30:00Z");
    }

    // ==========================================
    // Task Fields
    // ==========================================

    #[test]
    fn test_substitute_task_title() {
        let payload = r#"{"task": {"id": "task-1", "title": "Implement feature X"}}"#;
        let event = create_event(payload);

        let result = substitute("{task.title}", &event).unwrap();
        assert_eq!(result, "Implement feature X");
    }

    #[test]
    fn test_substitute_task_status() {
        let payload = r#"{"task": {"status": "in_progress"}}"#;
        let event = create_event(payload);

        let result = substitute("{task.status}", &event).unwrap();
        assert_eq!(result, "in_progress");
    }

    #[test]
    fn test_substitute_task_priority() {
        let payload = r#"{"task": {"priority": "P0"}}"#;
        let event = create_event(payload);

        let result = substitute("{task.priority}", &event).unwrap();
        assert_eq!(result, "P0");
    }

    #[test]
    fn test_substitute_all_task_fields() {
        let payload = r#"{
            "task": {
                "id": "proj-abc-task-1",
                "title": "Test Task",
                "status": "todo",
                "priority": "P1",
                "owner": "alice"
            }
        }"#;
        let event = create_event(payload);

        let result = substitute(
            "Task {task.id} ({task.title}) - {task.status} - {task.priority} - {task.owner}",
            &event,
        )
        .unwrap();
        assert_eq!(
            result,
            "Task proj-abc-task-1 (Test Task) - todo - P1 - alice"
        );
    }

    // ==========================================
    // Project Fields
    // ==========================================

    #[test]
    fn test_substitute_project_id() {
        let payload = r#"{"project": {"id": "my-project-xyz1"}}"#;
        let event = create_event(payload);

        let result = substitute("{project.id}", &event).unwrap();
        assert_eq!(result, "my-project-xyz1");
    }

    #[test]
    fn test_substitute_project_name() {
        let payload = r#"{"project": {"id": "proj-1", "name": "Backend API"}}"#;
        let event = create_event(payload);

        let result = substitute("{project.name}", &event).unwrap();
        assert_eq!(result, "Backend API");
    }

    // ==========================================
    // Session Fields
    // ==========================================

    #[test]
    fn test_substitute_session_id() {
        let payload = r#"{"session": {"id": "sess-20260115-abc1"}}"#;
        let event = create_event(payload);

        let result = substitute("{session.id}", &event).unwrap();
        assert_eq!(result, "sess-20260115-abc1");
    }

    #[test]
    fn test_substitute_session_name() {
        let payload = r#"{"session": {"name": "feature-work"}}"#;
        let event = create_event(payload);

        let result = substitute("{session.name}", &event).unwrap();
        assert_eq!(result, "feature-work");
    }

    // ==========================================
    // Top-level and Nested Fields
    // ==========================================

    #[test]
    fn test_substitute_top_level_field() {
        let payload = r#"{"custom_field": "custom_value"}"#;
        let event = create_event(payload);

        let result = substitute("{custom_field}", &event).unwrap();
        assert_eq!(result, "custom_value");
    }

    #[test]
    fn test_substitute_nested_top_level() {
        let payload = r#"{"data": {"nested": {"deep": "value"}}}"#;
        let event = create_event(payload);

        let result = substitute("{data.nested.deep}", &event).unwrap();
        assert_eq!(result, "value");
    }

    #[test]
    fn test_substitute_array_index() {
        let payload = r#"{"items": ["first", "second", "third"]}"#;
        let event = create_event(payload);

        let result = substitute("{items.0}", &event).unwrap();
        assert_eq!(result, "first");

        let result = substitute("{items.2}", &event).unwrap();
        assert_eq!(result, "third");
    }

    #[test]
    fn test_substitute_array_of_objects() {
        let payload = r#"{"items": [{"name": "alpha"}, {"name": "beta"}]}"#;
        let event = create_event(payload);

        let result = substitute("{items.0.name}", &event).unwrap();
        assert_eq!(result, "alpha");

        let result = substitute("{items.1.name}", &event).unwrap();
        assert_eq!(result, "beta");
    }

    // ==========================================
    // Type Coercion
    // ==========================================

    #[test]
    fn test_substitute_number() {
        let payload = r#"{"count": 42}"#;
        let event = create_event(payload);

        let result = substitute("{count}", &event).unwrap();
        assert_eq!(result, "42");
    }

    #[test]
    fn test_substitute_float() {
        let payload = r#"{"price": 19.99}"#;
        let event = create_event(payload);

        let result = substitute("{price}", &event).unwrap();
        assert_eq!(result, "19.99");
    }

    #[test]
    fn test_substitute_boolean_true() {
        let payload = r#"{"enabled": true}"#;
        let event = create_event(payload);

        let result = substitute("{enabled}", &event).unwrap();
        assert_eq!(result, "true");
    }

    #[test]
    fn test_substitute_boolean_false() {
        let payload = r#"{"enabled": false}"#;
        let event = create_event(payload);

        let result = substitute("{enabled}", &event).unwrap();
        assert_eq!(result, "false");
    }

    #[test]
    fn test_substitute_object_as_json() {
        let payload = r#"{"data": {"key": "value"}}"#;
        let event = create_event(payload);

        let result = substitute("{data}", &event).unwrap();
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_substitute_array_as_json() {
        let payload = r#"{"items": [1, 2, 3]}"#;
        let event = create_event(payload);

        let result = substitute("{items}", &event).unwrap();
        assert!(result.contains("1"));
        assert!(result.contains("2"));
        assert!(result.contains("3"));
    }

    // ==========================================
    // Unknown and Missing Placeholders
    // ==========================================

    #[test]
    fn test_substitute_unknown_field() {
        let event = create_event("{}");
        let result = substitute("{nonexistent}", &event).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_substitute_unknown_nested_field() {
        let payload = r#"{"task": {"id": "task-1"}}"#;
        let event = create_event(payload);

        let result = substitute("{task.nonexistent}", &event).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_substitute_partial_path() {
        let payload = r#"{"task": {"id": "task-1"}}"#;
        let event = create_event(payload);

        let result = substitute("{task.nested.deep}", &event).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_substitute_null_value() {
        let payload = r#"{"field": null}"#;
        let event = create_event(payload);

        let result = substitute("{field}", &event).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_substitute_mixed_known_unknown() {
        let payload = r#"{"task": {"id": "task-1"}}"#;
        let event = create_event(payload);

        let result = substitute("Known: {task.id}, Unknown: {missing}", &event).unwrap();
        assert_eq!(result, "Known: task-1, Unknown: ");
    }

    // ==========================================
    // Multiple Placeholders
    // ==========================================

    #[test]
    fn test_substitute_multiple_placeholders() {
        let payload = r#"{"task": {"id": "task-1", "title": "Test"}}"#;
        let event = create_event(payload);

        let result = substitute("{task.id} - {task.title}", &event).unwrap();
        assert_eq!(result, "task-1 - Test");
    }

    #[test]
    fn test_substitute_repeated_placeholder() {
        let payload = r#"{"task": {"id": "task-1"}}"#;
        let event = create_event(payload);

        let result = substitute("{task.id} and {task.id}", &event).unwrap();
        assert_eq!(result, "task-1 and task-1");
    }

    #[test]
    fn test_substitute_adjacent_placeholders() {
        let payload = r#"{"a": "A", "b": "B"}"#;
        let event = create_event(payload);

        let result = substitute("{a}{b}", &event).unwrap();
        assert_eq!(result, "AB");
    }

    // ==========================================
    // substitute_all
    // ==========================================

    #[test]
    fn test_substitute_all_basic() {
        let payload = r#"{"task": {"id": "task-1"}}"#;
        let event = create_event(payload);

        let templates = vec![
            "arg1".to_string(),
            "{task.id}".to_string(),
            "--event={event.id}".to_string(),
        ];

        let result = substitute_all(&templates, &event).unwrap();
        assert_eq!(result, vec!["arg1", "task-1", "--event=123"]);
    }

    #[test]
    fn test_substitute_all_empty() {
        let event = create_event("{}");
        let result = substitute_all(&[], &event).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_substitute_all_preserves_order() {
        let payload = r#"{"a": "1", "b": "2", "c": "3"}"#;
        let event = create_event(payload);

        let templates = vec!["{a}".to_string(), "{b}".to_string(), "{c}".to_string()];

        let result = substitute_all(&templates, &event).unwrap();
        assert_eq!(result, vec!["1", "2", "3"]);
    }

    // ==========================================
    // Edge Cases
    // ==========================================

    #[test]
    fn test_substitute_unclosed_brace() {
        let event = create_event("{}");
        // Unclosed brace should be left as-is in result
        let result = substitute("start {unclosed", &event).unwrap();
        // The implementation collects until EOF when no closing brace
        assert!(!result.contains("{")); // Opening brace consumed
    }

    #[test]
    fn test_substitute_empty_placeholder() {
        let event = create_event("{}");
        let result = substitute("{}", &event).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_substitute_only_opening_brace() {
        let event = create_event("{}");
        // Just an opening brace, parser continues to end
        let result = substitute("{", &event).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_substitute_nested_braces_not_supported() {
        let event = create_event(r#"{"nested": "value"}"#);
        // Nested braces aren't a special case - inner braces are just part of the path
        let result = substitute("{nested}", &event).unwrap();
        assert_eq!(result, "value");
    }

    // ==========================================
    // Real-world Command Templates
    // ==========================================

    #[test]
    fn test_real_world_claude_command() {
        let payload = r#"{
            "task": {
                "id": "my-project-abc1-task-5",
                "title": "Implement user authentication"
            }
        }"#;
        let event = create_event(payload);

        let templates = vec![
            "--print".to_string(),
            "--allowedTools".to_string(),
            "Bash,Read,Write,Edit,Glob,Grep".to_string(),
            "--message".to_string(),
            "Execute granary task {task.id}. Use /granary:execute-task skill.".to_string(),
        ];

        let result = substitute_all(&templates, &event).unwrap();
        assert_eq!(
            result[4],
            "Execute granary task my-project-abc1-task-5. Use /granary:execute-task skill."
        );
    }

    #[test]
    fn test_real_world_slack_notification() {
        let payload = r#"{
            "task": {
                "id": "proj-abc1-task-1",
                "title": "Fix critical bug",
                "priority": "P0"
            },
            "project": {
                "name": "Backend API"
            }
        }"#;
        let event = create_event(payload);

        // Test individual substitutions first
        let priority_result = substitute("{task.priority}", &event).unwrap();
        assert_eq!(priority_result, "P0");

        let title_result = substitute("{task.title}", &event).unwrap();
        assert_eq!(title_result, "Fix critical bug");

        let project_result = substitute("{project.name}", &event).unwrap();
        assert_eq!(project_result, "Backend API");

        // Test combined template
        let template = "[{task.priority}] {task.title} completed in {project.name}";
        let result = substitute(template, &event).unwrap();
        assert_eq!(result, "[P0] Fix critical bug completed in Backend API");
    }

    #[test]
    fn test_real_world_script_args() {
        let payload = r#"{
            "task": {"id": "task-123"},
            "project": {"id": "proj-abc"}
        }"#;
        let event = create_event(payload);

        let templates = vec![
            "{task.id}".to_string(),
            "{project.id}".to_string(),
            "--event-id={event.id}".to_string(),
        ];

        let result = substitute_all(&templates, &event).unwrap();
        assert_eq!(result, vec!["task-123", "proj-abc", "--event-id=123"]);
    }
}
