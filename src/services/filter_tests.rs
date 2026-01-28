//! Additional tests for the filter module.
//!
//! These tests complement the inline tests in filter.rs with additional
//! edge cases and integration-style tests.

#[cfg(test)]
mod tests {
    use crate::services::filter::{Filter, FilterOp, matches_all, matches_any, parse_filters};
    use serde_json::json;

    // ==========================================
    // Filter Parsing Edge Cases
    // ==========================================

    #[test]
    fn test_parse_filter_with_equals_in_value() {
        // Value can contain equals signs
        let filter = Filter::parse("key=value=with=equals").unwrap();
        assert_eq!(filter.field, "key");
        assert_eq!(filter.op, FilterOp::Eq);
        assert_eq!(filter.value, "value=with=equals");
    }

    #[test]
    fn test_parse_filter_empty_value() {
        let filter = Filter::parse("field=").unwrap();
        assert_eq!(filter.field, "field");
        assert_eq!(filter.value, "");
    }

    #[test]
    fn test_parse_filter_special_characters_in_field() {
        let filter = Filter::parse("my_field-name.sub=value").unwrap();
        assert_eq!(filter.field, "my_field-name.sub");
    }

    #[test]
    fn test_parse_filter_unicode_value() {
        let filter = Filter::parse("name=hello world").unwrap();
        assert_eq!(filter.value, "hello world");
    }

    #[test]
    fn test_parse_filter_numeric_field_name() {
        let filter = Filter::parse("0=zero").unwrap();
        assert_eq!(filter.field, "0");
    }

    #[test]
    fn test_parse_multiple_filters() {
        let filters = parse_filters(&[
            "status=active".to_string(),
            "priority!=low".to_string(),
            "name~=test".to_string(),
        ])
        .unwrap();

        assert_eq!(filters.len(), 3);
        assert_eq!(filters[0].op, FilterOp::Eq);
        assert_eq!(filters[1].op, FilterOp::NotEq);
        assert_eq!(filters[2].op, FilterOp::Contains);
    }

    #[test]
    fn test_parse_filters_empty_input() {
        let filters = parse_filters(&[]).unwrap();
        assert!(filters.is_empty());
    }

    #[test]
    fn test_parse_filter_preserves_case() {
        let filter = Filter::parse("Status=ACTIVE").unwrap();
        assert_eq!(filter.field, "Status");
        assert_eq!(filter.value, "ACTIVE");
    }

    // ==========================================
    // Filter Matching - Null and Missing Fields
    // ==========================================

    #[test]
    fn test_matches_null_field_explicit() {
        let filter = Filter::parse("field=null").unwrap();
        let payload = json!({"field": null});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_matches_missing_field_with_empty() {
        let filter = Filter::parse("missing=").unwrap();
        let payload = json!({"other": "value"});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_matches_missing_field_with_null() {
        let filter = Filter::parse("missing=null").unwrap();
        let payload = json!({"other": "value"});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_not_equals_missing_field() {
        let filter = Filter::parse("missing!=value").unwrap();
        let payload = json!({"other": "data"});
        assert!(filter.matches(&payload));
    }

    // ==========================================
    // Filter Matching - Complex JSON Structures
    // ==========================================

    #[test]
    fn test_matches_deeply_nested_array() {
        let filter = Filter::parse("data.items.0.tags.1=important").unwrap();
        let payload = json!({
            "data": {
                "items": [
                    {
                        "tags": ["normal", "important", "urgent"]
                    }
                ]
            }
        });
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_matches_array_out_of_bounds() {
        let filter = Filter::parse("items.99.name=test").unwrap();
        let payload = json!({"items": [{"name": "first"}]});
        assert!(!filter.matches(&payload));
    }

    #[test]
    fn test_matches_mixed_nested_path() {
        let filter = Filter::parse("users.0.profile.name=Alice").unwrap();
        let payload = json!({
            "users": [
                {"profile": {"name": "Alice", "age": 30}},
                {"profile": {"name": "Bob", "age": 25}}
            ]
        });
        assert!(filter.matches(&payload));
    }

    // ==========================================
    // Filter Matching - Type Coercion
    // ==========================================

    #[test]
    fn test_matches_number_as_string() {
        let filter = Filter::parse("count=42").unwrap();
        let payload = json!({"count": 42});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_matches_float_number() {
        let filter = Filter::parse("price=19.99").unwrap();
        let payload = json!({"price": 19.99});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_matches_negative_number() {
        let filter = Filter::parse("balance=-100").unwrap();
        let payload = json!({"balance": -100});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_matches_boolean_true() {
        let filter = Filter::parse("enabled=true").unwrap();
        let payload = json!({"enabled": true});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_matches_boolean_false() {
        let filter = Filter::parse("enabled=false").unwrap();
        let payload = json!({"enabled": false});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_not_matches_wrong_boolean() {
        let filter = Filter::parse("enabled=true").unwrap();
        let payload = json!({"enabled": false});
        assert!(!filter.matches(&payload));
    }

    // ==========================================
    // Contains Operator
    // ==========================================

    #[test]
    fn test_contains_partial_string() {
        let filter = Filter::parse("title~=test").unwrap();
        let payload = json!({"title": "my-test-project-v2"});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_contains_number_string() {
        let filter = Filter::parse("version~=42").unwrap();
        let payload = json!({"version": 1423});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_contains_in_array() {
        let filter = Filter::parse("tags~=urgent").unwrap();
        let payload = json!({"tags": ["normal", "urgent", "important"]});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_contains_in_nested_object() {
        let filter = Filter::parse("data~=secret").unwrap();
        let payload = json!({
            "data": {
                "key": "secret-value",
                "other": "stuff"
            }
        });
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_contains_null_pattern() {
        let filter = Filter::parse("field~=null").unwrap();
        let payload = json!({"field": null});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_contains_missing_field_returns_false() {
        let filter = Filter::parse("missing~=test").unwrap();
        let payload = json!({"other": "value"});
        assert!(!filter.matches(&payload));
    }

    // ==========================================
    // matches_all and matches_any
    // ==========================================

    #[test]
    fn test_matches_all_with_all_matching() {
        let filters =
            parse_filters(&["status=active".to_string(), "priority=high".to_string()]).unwrap();

        let payload = json!({"status": "active", "priority": "high"});
        assert!(matches_all(&filters, &payload));
    }

    #[test]
    fn test_matches_all_with_one_not_matching() {
        let filters =
            parse_filters(&["status=active".to_string(), "priority=high".to_string()]).unwrap();

        let payload = json!({"status": "active", "priority": "low"});
        assert!(!matches_all(&filters, &payload));
    }

    #[test]
    fn test_matches_all_empty_filters() {
        let filters: Vec<Filter> = vec![];
        let payload = json!({"anything": "here"});
        assert!(matches_all(&filters, &payload));
    }

    #[test]
    fn test_matches_any_with_one_matching() {
        let filters =
            parse_filters(&["status=active".to_string(), "status=pending".to_string()]).unwrap();

        let payload = json!({"status": "active"});
        assert!(matches_any(&filters, &payload));
    }

    #[test]
    fn test_matches_any_with_none_matching() {
        let filters =
            parse_filters(&["status=active".to_string(), "status=pending".to_string()]).unwrap();

        let payload = json!({"status": "done"});
        assert!(!matches_any(&filters, &payload));
    }

    #[test]
    fn test_matches_any_empty_filters() {
        let filters: Vec<Filter> = vec![];
        let payload = json!({"anything": "here"});
        // Empty filters should return true (no filters to fail)
        assert!(matches_any(&filters, &payload));
    }

    // ==========================================
    // Filter Display
    // ==========================================

    #[test]
    fn test_filter_display_eq() {
        let filter = Filter::parse("status=active").unwrap();
        assert_eq!(format!("{}", filter), "status=active");
    }

    #[test]
    fn test_filter_display_not_eq() {
        let filter = Filter::parse("status!=draft").unwrap();
        assert_eq!(format!("{}", filter), "status!=draft");
    }

    #[test]
    fn test_filter_display_contains() {
        let filter = Filter::parse("name~=test").unwrap();
        assert_eq!(format!("{}", filter), "name~=test");
    }

    // ==========================================
    // Error Cases
    // ==========================================

    #[test]
    fn test_parse_filter_no_operator() {
        let result = Filter::parse("justtext");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_filter_empty_field() {
        let result = Filter::parse("=value");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_filter_only_operator() {
        let result = Filter::parse("=");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_filter_empty_string() {
        let result = Filter::parse("");
        assert!(result.is_err());
    }

    // ==========================================
    // Real-world Task Filtering Scenarios
    // ==========================================

    #[test]
    fn test_filter_task_by_status() {
        let filter = Filter::parse("task.status=in_progress").unwrap();
        let payload = json!({
            "task": {
                "id": "proj-abc1-task-1",
                "status": "in_progress",
                "priority": "P1"
            }
        });
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_filter_task_exclude_draft() {
        let filter = Filter::parse("task.status!=draft").unwrap();
        let payload = json!({
            "task": {
                "status": "todo",
                "priority": "P2"
            }
        });
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_filter_high_priority_tasks() {
        let filters = parse_filters(&[
            "task.priority!=P3".to_string(),
            "task.priority!=P4".to_string(),
        ])
        .unwrap();

        let p0_task = json!({"task": {"priority": "P0"}});
        let p1_task = json!({"task": {"priority": "P1"}});
        let p4_task = json!({"task": {"priority": "P4"}});

        assert!(matches_all(&filters, &p0_task));
        assert!(matches_all(&filters, &p1_task));
        assert!(!matches_all(&filters, &p4_task));
    }

    #[test]
    fn test_filter_task_by_owner() {
        let filter = Filter::parse("task.owner=claude").unwrap();
        let payload = json!({
            "task": {
                "id": "task-1",
                "owner": "claude"
            }
        });
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_filter_unowned_task() {
        let filter = Filter::parse("task.owner=").unwrap();
        let payload = json!({
            "task": {
                "id": "task-1",
                "status": "todo"
            }
        });
        assert!(filter.matches(&payload));
    }
}
