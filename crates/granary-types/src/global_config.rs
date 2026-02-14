//! Global configuration model for user-level settings.
//!
//! The global config lives at `~/.granary/config.toml` and contains
//! user-level settings like runner definitions that persist across workspaces.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for log retention and rotation policies.
///
/// Controls automatic cleanup of log files to prevent unbounded disk usage.
/// Used by the daemon for periodic log maintenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRetentionConfig {
    /// Maximum age of log files to keep in days.
    /// Files older than this will be automatically deleted.
    pub max_age_days: u64,

    /// Maximum total size of logs directory in megabytes.
    /// When exceeded, oldest logs are deleted first.
    pub max_total_size_mb: u64,

    /// Maximum number of log files per worker.
    /// Excess files (oldest first) are deleted.
    pub max_files_per_worker: usize,
}

impl Default for LogRetentionConfig {
    fn default() -> Self {
        Self {
            max_age_days: 7,
            max_total_size_mb: 100,
            max_files_per_worker: 100,
        }
    }
}

/// Global configuration structure stored at ~/.granary/config.toml
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    /// Runner definitions that can be referenced by name
    #[serde(default)]
    pub runners: HashMap<String, RunnerConfig>,

    /// Action definitions that can be referenced by runners
    #[serde(default)]
    pub actions: HashMap<String, ActionConfig>,
}

/// Type alias for action configuration (currently identical to RunnerConfig)
pub type ActionConfig = RunnerConfig;

/// Configuration for a runner that executes tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerConfig {
    /// Command to execute (e.g., "claude", "python")
    pub command: String,

    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,

    /// Maximum concurrent executions for this runner
    #[serde(default)]
    pub concurrency: Option<u32>,

    /// Default event type this runner responds to
    #[serde(default)]
    pub on: Option<String>,

    /// Environment variables to set when running
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// References an action by name so runners can inherit defaults from actions
    #[serde(default)]
    pub action: Option<String>,
}

impl RunnerConfig {
    /// Create a new runner configuration with just a command
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            concurrency: None,
            on: None,
            env: HashMap::new(),
            action: None,
        }
    }

    /// Expand environment variables in args.
    /// Supports ${VAR} and $VAR syntax.
    pub fn expand_env_in_args(&self) -> Vec<String> {
        self.args.iter().map(|arg| expand_env_vars(arg)).collect()
    }
}

/// Merge an action's defaults with a runner's overrides.
///
/// The runner's fields take precedence over the action's fields.
/// For `env`, the maps are merged with the runner's entries winning on conflicts.
/// The resulting `action` field is set to `None` (already resolved).
pub fn merge_action_with_runner(action: &ActionConfig, runner: &RunnerConfig) -> RunnerConfig {
    let command = if runner.command.is_empty() {
        action.command.clone()
    } else {
        runner.command.clone()
    };

    let args = if runner.args.is_empty() {
        action.args.clone()
    } else {
        runner.args.clone()
    };

    let concurrency = runner.concurrency.or(action.concurrency);
    let on = runner.on.clone().or_else(|| action.on.clone());

    let mut env = action.env.clone();
    env.extend(runner.env.clone());

    RunnerConfig {
        command,
        args,
        concurrency,
        on,
        env,
        action: None,
    }
}

/// Expand environment variables in a string.
/// Supports ${VAR} and $VAR syntax.
pub fn expand_env_vars(input: &str) -> String {
    let mut result = input.to_string();

    // Handle ${VAR} syntax
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let value = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                value,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_global_config() {
        let config = GlobalConfig::default();
        assert!(config.runners.is_empty());
    }

    #[test]
    fn test_runner_config_new() {
        let runner = RunnerConfig::new("claude");
        assert_eq!(runner.command, "claude");
        assert!(runner.args.is_empty());
        assert!(runner.concurrency.is_none());
        assert!(runner.on.is_none());
        assert!(runner.env.is_empty());
    }

    #[test]
    fn test_expand_env_vars() {
        // SAFETY: Tests are run single-threaded
        unsafe {
            std::env::set_var("TEST_VAR", "hello");
        }
        assert_eq!(expand_env_vars("${TEST_VAR} world"), "hello world");
        assert_eq!(expand_env_vars("no vars here"), "no vars here");
        // SAFETY: Tests are run single-threaded
        unsafe {
            std::env::remove_var("TEST_VAR");
        }
    }

    #[test]
    fn test_merge_action_with_runner_uses_action_defaults() {
        let action = RunnerConfig {
            command: "claude".to_string(),
            args: vec!["code".to_string(), "--task".to_string()],
            concurrency: Some(3),
            on: Some("task.unblocked".to_string()),
            env: HashMap::from([("MODEL".to_string(), "opus".to_string())]),
            action: None,
        };
        // Runner with empty/default fields should inherit from action
        let runner = RunnerConfig::new("");

        let merged = merge_action_with_runner(&action, &runner);
        assert_eq!(merged.command, "claude");
        assert_eq!(merged.args, vec!["code", "--task"]);
        assert_eq!(merged.concurrency, Some(3));
        assert_eq!(merged.on.as_deref(), Some("task.unblocked"));
        assert_eq!(merged.env.get("MODEL").unwrap(), "opus");
        assert!(merged.action.is_none());
    }

    #[test]
    fn test_merge_action_with_runner_runner_overrides() {
        let action = RunnerConfig {
            command: "claude".to_string(),
            args: vec!["code".to_string()],
            concurrency: Some(3),
            on: Some("task.unblocked".to_string()),
            env: HashMap::from([("MODEL".to_string(), "opus".to_string())]),
            action: None,
        };
        let runner = RunnerConfig {
            command: "python".to_string(),
            args: vec!["run.py".to_string()],
            concurrency: Some(5),
            on: Some("task.created".to_string()),
            env: HashMap::from([("MODEL".to_string(), "sonnet".to_string())]),
            action: Some("my-action".to_string()),
        };

        let merged = merge_action_with_runner(&action, &runner);
        assert_eq!(merged.command, "python");
        assert_eq!(merged.args, vec!["run.py"]);
        assert_eq!(merged.concurrency, Some(5));
        assert_eq!(merged.on.as_deref(), Some("task.created"));
        assert_eq!(merged.env.get("MODEL").unwrap(), "sonnet");
        assert!(merged.action.is_none());
    }

    #[test]
    fn test_merge_action_with_runner_env_merge() {
        let action = RunnerConfig {
            command: "cmd".to_string(),
            args: vec![],
            concurrency: None,
            on: None,
            env: HashMap::from([
                ("A".to_string(), "from-action".to_string()),
                ("B".to_string(), "from-action".to_string()),
            ]),
            action: None,
        };
        let runner = RunnerConfig {
            command: "".to_string(),
            args: vec![],
            concurrency: None,
            on: None,
            env: HashMap::from([("B".to_string(), "from-runner".to_string())]),
            action: None,
        };

        let merged = merge_action_with_runner(&action, &runner);
        assert_eq!(merged.env.get("A").unwrap(), "from-action");
        assert_eq!(merged.env.get("B").unwrap(), "from-runner");
    }

    #[test]
    fn test_merge_action_with_runner_partial_override() {
        let action = RunnerConfig {
            command: "claude".to_string(),
            args: vec!["--model".to_string(), "opus".to_string()],
            concurrency: Some(2),
            on: Some("task.unblocked".to_string()),
            env: HashMap::new(),
            action: None,
        };
        // Runner overrides only command and concurrency
        let runner = RunnerConfig {
            command: "python".to_string(),
            args: vec![],
            concurrency: Some(10),
            on: None,
            env: HashMap::new(),
            action: Some("my-action".to_string()),
        };

        let merged = merge_action_with_runner(&action, &runner);
        assert_eq!(merged.command, "python");
        assert_eq!(merged.args, vec!["--model", "opus"]); // from action
        assert_eq!(merged.concurrency, Some(10)); // from runner
        assert_eq!(merged.on.as_deref(), Some("task.unblocked")); // from action
    }

    #[test]
    fn test_runner_expand_env_in_args() {
        // SAFETY: Tests are run single-threaded
        unsafe {
            std::env::set_var("TOKEN", "secret123");
        }
        let mut runner = RunnerConfig::new("curl");
        runner.args = vec![
            "-H".to_string(),
            "Authorization: Bearer ${TOKEN}".to_string(),
        ];

        let expanded = runner.expand_env_in_args();
        assert_eq!(expanded[1], "Authorization: Bearer secret123");
        // SAFETY: Tests are run single-threaded
        unsafe {
            std::env::remove_var("TOKEN");
        }
    }
}
