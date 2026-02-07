//! Filter parsing and evaluation for event filtering.
//!
//! Filter format:
//! - `field=value` - Equality check
//! - `field!=value` - Inequality check
//! - `field~=pattern` - Contains check (substring match)
//!
//! Fields can be nested using dot notation: `task.status`, `payload.project_id`

use crate::error::{GranaryError, Result};
use serde_json::Value;

/// Filter operation types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOp {
    /// Equality: field=value
    Eq,
    /// Inequality: field!=value
    NotEq,
    /// Contains: field~=pattern (substring match)
    Contains,
}

impl FilterOp {
    /// Get the operator string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            FilterOp::Eq => "=",
            FilterOp::NotEq => "!=",
            FilterOp::Contains => "~=",
        }
    }
}

/// A single filter expression
#[derive(Debug, Clone)]
pub struct Filter {
    /// The field path (can be nested with dots, e.g., "task.status")
    pub field: String,
    /// The filter operation
    pub op: FilterOp,
    /// The value to match against
    pub value: String,
}

impl Filter {
    /// Parse a filter string into a Filter struct
    ///
    /// Supported formats:
    /// - `field=value` (equality)
    /// - `field!=value` (inequality)
    /// - `field~=pattern` (contains)
    pub fn parse(s: &str) -> Result<Self> {
        // Try to parse in order of specificity (longest operators first)
        if let Some(pos) = s.find("~=") {
            let field = s[..pos].trim().to_string();
            let value = s[pos + 2..].trim().to_string();
            if field.is_empty() {
                return Err(GranaryError::InvalidArgument(
                    "Filter field cannot be empty".to_string(),
                ));
            }
            return Ok(Filter {
                field,
                op: FilterOp::Contains,
                value,
            });
        }

        if let Some(pos) = s.find("!=") {
            let field = s[..pos].trim().to_string();
            let value = s[pos + 2..].trim().to_string();
            if field.is_empty() {
                return Err(GranaryError::InvalidArgument(
                    "Filter field cannot be empty".to_string(),
                ));
            }
            return Ok(Filter {
                field,
                op: FilterOp::NotEq,
                value,
            });
        }

        if let Some(pos) = s.find('=') {
            let field = s[..pos].trim().to_string();
            let value = s[pos + 1..].trim().to_string();
            if field.is_empty() {
                return Err(GranaryError::InvalidArgument(
                    "Filter field cannot be empty".to_string(),
                ));
            }
            return Ok(Filter {
                field,
                op: FilterOp::Eq,
                value,
            });
        }

        Err(GranaryError::InvalidArgument(format!(
            "Invalid filter format: '{}'. Expected 'field=value', 'field!=value', or 'field~=pattern'",
            s
        )))
    }

    /// Evaluate this filter against a JSON payload
    ///
    /// Returns true if the filter matches the payload
    pub fn matches(&self, payload: &Value) -> bool {
        let field_value = get_nested_value(payload, &self.field);

        match &self.op {
            FilterOp::Eq => match field_value {
                Some(v) => value_equals(v, &self.value),
                None => self.value.is_empty() || self.value == "null",
            },
            FilterOp::NotEq => match field_value {
                Some(v) => !value_equals(v, &self.value),
                None => !self.value.is_empty() && self.value != "null",
            },
            FilterOp::Contains => match field_value {
                Some(v) => value_contains(v, &self.value),
                None => false,
            },
        }
    }
}

impl Filter {
    /// Convert filter to SQL WHERE clause fragment using json_extract.
    /// Returns (sql_fragment, json_path, bind_value).
    pub fn to_sql(&self) -> (String, String, String) {
        let json_path = self.field_to_json_path();
        match self.op {
            FilterOp::Eq => (
                "json_extract(e.payload, ?) = ?".into(),
                json_path,
                self.value.clone(),
            ),
            FilterOp::NotEq => (
                "json_extract(e.payload, ?) != ?".into(),
                json_path,
                self.value.clone(),
            ),
            FilterOp::Contains => (
                "json_extract(e.payload, ?) LIKE ?".into(),
                json_path,
                format!("%{}%", self.value),
            ),
        }
    }

    /// Convert dot-separated field path to JSON path.
    /// e.g., "a.b.0.c" -> "$.a.b[0].c"
    fn field_to_json_path(&self) -> String {
        let mut path = String::from("$");
        for part in self.field.split('.') {
            if part.parse::<usize>().is_ok() {
                path.push_str(&format!("[{}]", part));
            } else {
                path.push('.');
                path.push_str(part);
            }
        }
        path
    }
}

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}{}", self.field, self.op.as_str(), self.value)
    }
}

/// Get a nested value from a JSON object using dot notation
///
/// For example, `get_nested_value(obj, "task.status")` will return
/// the value at `obj["task"]["status"]`
fn get_nested_value<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value;

    for part in parts {
        match current {
            Value::Object(map) => {
                current = map.get(part)?;
            }
            Value::Array(arr) => {
                // Support array indexing like "items.0.name"
                let index: usize = part.parse().ok()?;
                current = arr.get(index)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

/// Check if a JSON value equals a string value
fn value_equals(json_value: &Value, string_value: &str) -> bool {
    match json_value {
        Value::String(s) => s == string_value,
        Value::Number(n) => n.to_string() == string_value,
        Value::Bool(b) => (string_value == "true" && *b) || (string_value == "false" && !*b),
        Value::Null => string_value == "null" || string_value.is_empty(),
        _ => false,
    }
}

/// Check if a JSON value contains a substring
fn value_contains(json_value: &Value, pattern: &str) -> bool {
    match json_value {
        Value::String(s) => s.contains(pattern),
        Value::Number(n) => n.to_string().contains(pattern),
        Value::Bool(b) => b.to_string().contains(pattern),
        Value::Array(arr) => {
            // Check if any element in the array matches
            arr.iter().any(|v| value_contains(v, pattern))
        }
        Value::Object(_) => {
            // Serialize and check if pattern appears anywhere
            serde_json::to_string(json_value)
                .map(|s| s.contains(pattern))
                .unwrap_or(false)
        }
        Value::Null => pattern == "null",
    }
}

/// Parse multiple filter strings into Filter structs
pub fn parse_filters(filters: &[String]) -> Result<Vec<Filter>> {
    filters.iter().map(|s| Filter::parse(s)).collect()
}

/// Check if all filters match the given payload
pub fn matches_all(filters: &[Filter], payload: &Value) -> bool {
    filters.iter().all(|f| f.matches(payload))
}

/// Check if any filter matches the given payload
pub fn matches_any(filters: &[Filter], payload: &Value) -> bool {
    filters.is_empty() || filters.iter().any(|f| f.matches(payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_equality_filter() {
        let filter = Filter::parse("status=active").unwrap();
        assert_eq!(filter.field, "status");
        assert_eq!(filter.op, FilterOp::Eq);
        assert_eq!(filter.value, "active");
    }

    #[test]
    fn test_parse_inequality_filter() {
        let filter = Filter::parse("status!=draft").unwrap();
        assert_eq!(filter.field, "status");
        assert_eq!(filter.op, FilterOp::NotEq);
        assert_eq!(filter.value, "draft");
    }

    #[test]
    fn test_parse_contains_filter() {
        let filter = Filter::parse("name~=test").unwrap();
        assert_eq!(filter.field, "name");
        assert_eq!(filter.op, FilterOp::Contains);
        assert_eq!(filter.value, "test");
    }

    #[test]
    fn test_parse_with_whitespace() {
        let filter = Filter::parse("  status  =  active  ").unwrap();
        assert_eq!(filter.field, "status");
        assert_eq!(filter.value, "active");
    }

    #[test]
    fn test_parse_invalid_filter() {
        assert!(Filter::parse("invalid").is_err());
        assert!(Filter::parse("").is_err());
        assert!(Filter::parse("=value").is_err());
    }

    #[test]
    fn test_matches_equality() {
        let filter = Filter::parse("status=active").unwrap();
        let payload = json!({"status": "active"});
        assert!(filter.matches(&payload));

        let payload = json!({"status": "inactive"});
        assert!(!filter.matches(&payload));
    }

    #[test]
    fn test_matches_inequality() {
        let filter = Filter::parse("status!=draft").unwrap();
        let payload = json!({"status": "active"});
        assert!(filter.matches(&payload));

        let payload = json!({"status": "draft"});
        assert!(!filter.matches(&payload));
    }

    #[test]
    fn test_matches_contains() {
        let filter = Filter::parse("name~=test").unwrap();
        let payload = json!({"name": "my-test-project"});
        assert!(filter.matches(&payload));

        let payload = json!({"name": "my-project"});
        assert!(!filter.matches(&payload));
    }

    #[test]
    fn test_matches_nested_field() {
        let filter = Filter::parse("task.status=done").unwrap();
        let payload = json!({"task": {"status": "done", "id": "123"}});
        assert!(filter.matches(&payload));

        let payload = json!({"task": {"status": "pending"}});
        assert!(!filter.matches(&payload));
    }

    #[test]
    fn test_matches_number() {
        let filter = Filter::parse("count=42").unwrap();
        let payload = json!({"count": 42});
        assert!(filter.matches(&payload));

        let payload = json!({"count": 41});
        assert!(!filter.matches(&payload));
    }

    #[test]
    fn test_matches_boolean() {
        let filter = Filter::parse("active=true").unwrap();
        let payload = json!({"active": true});
        assert!(filter.matches(&payload));

        let payload = json!({"active": false});
        assert!(!filter.matches(&payload));
    }

    #[test]
    fn test_matches_missing_field() {
        let filter = Filter::parse("missing=").unwrap();
        let payload = json!({"other": "value"});
        assert!(filter.matches(&payload));

        let filter = Filter::parse("missing!=").unwrap();
        assert!(!filter.matches(&payload));
    }

    #[test]
    fn test_matches_all_filters() {
        let filters =
            parse_filters(&["status=active".to_string(), "priority!=low".to_string()]).unwrap();

        let payload = json!({"status": "active", "priority": "high"});
        assert!(matches_all(&filters, &payload));

        let payload = json!({"status": "active", "priority": "low"});
        assert!(!matches_all(&filters, &payload));
    }

    #[test]
    fn test_deeply_nested_field() {
        let filter = Filter::parse("a.b.c.d=value").unwrap();
        let payload = json!({"a": {"b": {"c": {"d": "value"}}}});
        assert!(filter.matches(&payload));
    }

    #[test]
    fn test_array_indexing() {
        let filter = Filter::parse("items.0.name=first").unwrap();
        let payload = json!({"items": [{"name": "first"}, {"name": "second"}]});
        assert!(filter.matches(&payload));
    }
}
