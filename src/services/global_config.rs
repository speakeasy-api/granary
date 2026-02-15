//! Global configuration service for loading and saving user-level settings.
//!
//! Manages the config file at `~/.granary/config.toml` and the global database
//! at `~/.granary/workers.db`.

use crate::db::connection::{create_pool, run_migrations};
use crate::error::{GranaryError, Result};
use crate::models::{ActionConfig, GlobalConfig, RunnerConfig};
use sqlx::SqlitePool;
use std::path::PathBuf;
use tokio::sync::OnceCell;

/// Singleton for the global database pool.
/// Ensures migrations run exactly once before any queries.
static GLOBAL_POOL: OnceCell<SqlitePool> = OnceCell::const_new();

/// Get the global granary config directory (~/.granary)
pub fn config_dir() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|home| home.join(".granary"))
        .ok_or_else(|| GranaryError::GlobalConfig("Could not determine home directory".into()))
}

/// Check if this is the first time granary is running on this system.
///
/// Returns `true` if the ~/.granary directory does not exist, indicating
/// this is a fresh installation. This check must be performed before
/// any operations that might create the directory.
pub fn is_first_run() -> Result<bool> {
    let dir = config_dir()?;
    Ok(!dir.exists())
}

/// Get the path to the global config file (~/.granary/config.toml)
pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

/// Get the path to the global workers database (~/.granary/workers.db)
pub fn global_db_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("workers.db"))
}

/// Get the path to the logs directory (~/.granary/logs)
pub fn logs_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("logs"))
}

/// Get the daemon directory (~/.granary/daemon)
pub fn daemon_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("daemon"))
}

/// Get the daemon socket path (~/.granary/daemon/granaryd.sock)
#[cfg(unix)]
pub fn daemon_socket_path() -> Result<PathBuf> {
    Ok(daemon_dir()?.join("granaryd.sock"))
}

/// Get the daemon pipe name (Windows)
///
/// Returns a named pipe path in the format `\\.\pipe\granaryd-{username}`.
/// The username is included for per-user isolation, ensuring each user
/// has their own daemon instance.
#[cfg(windows)]
pub fn daemon_pipe_name() -> String {
    let username = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
    format!(r"\\.\pipe\granaryd-{}", username)
}

/// Get the daemon PID file path (~/.granary/daemon/granaryd.pid)
pub fn daemon_pid_path() -> Result<PathBuf> {
    Ok(daemon_dir()?.join("granaryd.pid"))
}

/// Get the daemon log path (~/.granary/daemon/daemon.log)
pub fn daemon_log_path() -> Result<PathBuf> {
    Ok(daemon_dir()?.join("daemon.log"))
}

/// Get the daemon auth token path (~/.granary/daemon/auth.token)
pub fn daemon_auth_token_path() -> Result<PathBuf> {
    Ok(daemon_dir()?.join("auth.token"))
}

/// Generate or read existing auth token for daemon IPC authentication.
///
/// If the token file exists, reads and returns it.
/// Otherwise, generates a new UUID token, writes it to disk with
/// secure permissions (0600 on Unix), and returns it.
pub fn get_or_create_auth_token() -> Result<String> {
    let path = daemon_auth_token_path()?;

    // Ensure the daemon directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if path.exists() {
        Ok(std::fs::read_to_string(&path)?.trim().to_string())
    } else {
        let token = uuid::Uuid::new_v4().to_string();
        std::fs::write(&path, &token)?;
        // Set file permissions to 0600 on Unix for security
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(token)
    }
}

/// Get the logs directory for a specific worker (~/.granary/logs/<worker_id>)
pub fn worker_logs_dir(worker_id: &str) -> Result<PathBuf> {
    Ok(logs_dir()?.join(worker_id))
}

/// Get a connection pool to the global workers database.
///
/// This returns a singleton pool that is initialized once per process.
/// Migrations are guaranteed to complete before any queries can run.
pub async fn global_pool() -> Result<SqlitePool> {
    GLOBAL_POOL
        .get_or_try_init(|| async {
            let db_path = global_db_path()?;

            // Ensure the directory exists
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let pool = create_pool(&db_path).await?;
            run_migrations(&pool).await?;
            Ok(pool)
        })
        .await
        .cloned()
}

/// Load the global configuration from ~/.granary/config.toml
/// Returns default config if file doesn't exist.
pub fn load() -> Result<GlobalConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(GlobalConfig::default());
    }

    let content = std::fs::read_to_string(&path)?;
    toml::from_str(&content)
        .map_err(|e| GranaryError::GlobalConfig(format!("Failed to parse config: {}", e)))
}

/// Save the global configuration to ~/.granary/config.toml
pub fn save(config: &GlobalConfig) -> Result<()> {
    let path = config_path()?;

    // Ensure the directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = toml::to_string_pretty(config)
        .map_err(|e| GranaryError::GlobalConfig(format!("Failed to serialize config: {}", e)))?;

    std::fs::write(&path, content)?;
    Ok(())
}

/// Get the actions directory (~/.granary/actions)
pub fn actions_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("actions"))
}

/// Load an action from a standalone TOML file in the given directory.
///
/// Returns `Ok(None)` if the file does not exist.
fn load_action_from_dir(dir: &std::path::Path, name: &str) -> Result<Option<ActionConfig>> {
    let path = dir.join(format!("{}.toml", name));
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)?;
    let action: ActionConfig = toml::from_str(&content).map_err(|e| {
        GranaryError::GlobalConfig(format!("Failed to parse action file '{}': {}", name, e))
    })?;
    Ok(Some(action))
}

/// Load an action from its standalone file (~/.granary/actions/<name>.toml)
///
/// Returns `Ok(None)` if the file does not exist.
pub fn load_action(name: &str) -> Result<Option<ActionConfig>> {
    load_action_from_dir(&actions_dir()?, name)
}

/// List .toml file stem names from a directory, recursively walking subdirectories.
/// Returns namespaced names using `/` as separator (e.g., `git/worktree-create`).
fn list_action_files_in_dir(dir: &std::path::Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    fn walk(
        base: &std::path::Path,
        current: &std::path::Path,
        names: &mut Vec<String>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, names)?;
            } else if path.extension().and_then(|e| e.to_str()) == Some("toml")
                && let Ok(rel) = path.strip_prefix(base)
            {
                let name = rel.with_extension("").to_string_lossy().to_string();
                names.push(name.replace('\\', "/"));
            }
        }
        Ok(())
    }
    walk(dir, dir, &mut names).map_err(GranaryError::Io)?;
    names.sort();
    Ok(names)
}

/// List action file names from ~/.granary/actions/ (returns stem names without .toml)
pub fn list_action_files() -> Result<Vec<String>> {
    list_action_files_in_dir(&actions_dir()?)
}

/// Get an action by name, checking inline config actions first, then file-based.
fn get_action_with(
    inline: &std::collections::HashMap<String, ActionConfig>,
    dir: &std::path::Path,
    name: &str,
) -> Result<Option<ActionConfig>> {
    if let Some(action) = inline.get(name) {
        return Ok(Some(action.clone()));
    }
    load_action_from_dir(dir, name)
}

/// Get an action by name, checking inline config first, then file-based actions.
///
/// Inline actions (from config.toml `[actions]`) take precedence over
/// file-based actions (from `~/.granary/actions/<name>.toml`).
pub fn get_action(name: &str) -> Result<Option<ActionConfig>> {
    let config = load()?;
    get_action_with(&config.actions, &actions_dir()?, name)
}

/// Merge inline actions and file-based actions, with inline taking precedence.
fn list_all_actions_with(
    inline: &std::collections::HashMap<String, ActionConfig>,
    dir: &std::path::Path,
) -> Result<Vec<(String, ActionConfig)>> {
    let file_names = list_action_files_in_dir(dir)?;

    let mut result: Vec<(String, ActionConfig)> = Vec::new();

    // File-based actions (only if not overridden by inline)
    for name in file_names {
        if !inline.contains_key(&name)
            && let Some(action) = load_action_from_dir(dir, &name)?
        {
            result.push((name, action));
        }
    }

    // Inline actions take precedence
    for (name, action) in inline {
        result.push((name.clone(), action.clone()));
    }

    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}

/// List all actions from both inline config and action files.
///
/// Returns a merged list where inline actions take precedence on name conflicts.
pub fn list_all_actions() -> Result<Vec<(String, ActionConfig)>> {
    let config = load()?;
    list_all_actions_with(&config.actions, &actions_dir()?)
}

/// Add or update an inline action configuration
pub fn set_action(name: &str, action: ActionConfig) -> Result<()> {
    let mut config = load()?;
    config.actions.insert(name.to_string(), action);
    save(&config)
}

/// Remove an inline action configuration
pub fn remove_action(name: &str) -> Result<bool> {
    let mut config = load()?;
    let removed = config.actions.remove(name).is_some();
    if removed {
        save(&config)?;
    }
    Ok(removed)
}

/// Get a specific runner by name
pub fn get_runner(name: &str) -> Result<Option<RunnerConfig>> {
    let config = load()?;
    Ok(config.runners.get(name).cloned())
}

/// Add or update a runner configuration
pub fn set_runner(name: &str, runner: RunnerConfig) -> Result<()> {
    let mut config = load()?;
    config.runners.insert(name.to_string(), runner);
    save(&config)
}

/// Remove a runner configuration
pub fn remove_runner(name: &str) -> Result<bool> {
    let mut config = load()?;
    let removed = config.runners.remove(name).is_some();
    if removed {
        save(&config)?;
    }
    Ok(removed)
}

/// Load the global config as a serde_json::Value and resolve a dot-path into it.
///
/// If `key` is `None`, returns the entire config. Otherwise splits on '.'
/// and traverses tables/maps to reach the requested subtree.
pub fn get_by_path(key: Option<&str>) -> Result<serde_json::Value> {
    let config = load()?;
    let value = serde_json::to_value(&config)
        .map_err(|e| GranaryError::GlobalConfig(format!("Failed to serialize config: {}", e)))?;

    let key = match key {
        None => return Ok(value),
        Some("") => return Ok(value),
        Some(k) => k,
    };

    let segments: Vec<&str> = key.split('.').collect();
    let mut current = &value;

    for (i, segment) in segments.iter().enumerate() {
        match current {
            serde_json::Value::Object(map) => match map.get(*segment) {
                Some(v) => current = v,
                None => {
                    let path_so_far = segments[..=i].join(".");
                    return Err(GranaryError::GlobalConfig(format!(
                        "Key not found: {}",
                        path_so_far
                    )));
                }
            },
            _ => {
                let path_so_far = segments[..i].join(".");
                return Err(GranaryError::GlobalConfig(format!(
                    "'{}' is not a table/section, cannot traverse further",
                    path_so_far
                )));
            }
        }
    }

    Ok(current.clone())
}

/// List all runner names
pub fn list_runners() -> Result<Vec<String>> {
    let config = load()?;
    Ok(config.runners.keys().cloned().collect())
}

/// Open the config file in the user's editor
pub fn edit_config() -> Result<()> {
    let path = config_path()?;

    // Ensure the directory and file exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Create default config file if it doesn't exist
    if !path.exists() {
        let default_config = GlobalConfig::default();
        save(&default_config)?;
    }

    // Get the editor from environment
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    // Open the editor
    let status = std::process::Command::new(&editor).arg(&path).status()?;

    if !status.success() {
        return Err(GranaryError::GlobalConfig(format!(
            "Editor '{}' exited with non-zero status",
            editor
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_dir() {
        let dir = config_dir();
        assert!(dir.is_ok());
        let dir = dir.unwrap();
        assert!(dir.ends_with(".granary"));
    }

    #[test]
    fn test_config_path() {
        let path = config_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("config.toml"));
    }

    #[test]
    fn test_daemon_dir() {
        let dir = daemon_dir();
        assert!(dir.is_ok());
        let dir = dir.unwrap();
        assert!(dir.ends_with("daemon"));
        assert!(dir.parent().unwrap().ends_with(".granary"));
    }

    #[cfg(unix)]
    #[test]
    fn test_daemon_socket_path() {
        let path = daemon_socket_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("granaryd.sock"));
    }

    #[cfg(windows)]
    #[test]
    fn test_daemon_pipe_name() {
        let pipe_name = daemon_pipe_name();
        assert!(pipe_name.starts_with(r"\\.\pipe\granaryd-"));
        // Verify it contains a username component
        assert!(pipe_name.len() > r"\\.\pipe\granaryd-".len());
    }

    #[test]
    fn test_daemon_pid_path() {
        let path = daemon_pid_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("granaryd.pid"));
    }

    #[test]
    fn test_daemon_log_path() {
        let path = daemon_log_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("daemon.log"));
    }

    #[test]
    fn test_daemon_auth_token_path() {
        let path = daemon_auth_token_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("auth.token"));
        assert!(path.parent().unwrap().ends_with("daemon"));
    }

    #[test]
    fn test_is_first_run() {
        // This test verifies is_first_run returns a valid result.
        // The actual value depends on whether ~/.granary exists on the system.
        let result = is_first_run();
        assert!(result.is_ok());
        // The result should be a boolean (true if ~/.granary doesn't exist)
        let _is_first = result.unwrap();
    }

    #[test]
    fn test_actions_dir() {
        let dir = actions_dir();
        assert!(dir.is_ok());
        let dir = dir.unwrap();
        assert!(dir.ends_with("actions"));
        assert!(dir.parent().unwrap().ends_with(".granary"));
    }

    #[test]
    fn test_load_action_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_action_from_dir(tmp.path(), "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_load_action_valid_file() {
        let tmp = tempfile::tempdir().unwrap();
        let content = r#"
command = "claude"
args = ["code", "--task"]
concurrency = 2
on = "task.unblocked"

[env]
CLAUDE_MODEL = "opus"
"#;
        std::fs::write(tmp.path().join("my-action.toml"), content).unwrap();

        let action = load_action_from_dir(tmp.path(), "my-action")
            .unwrap()
            .expect("action should exist");
        assert_eq!(action.command.as_deref(), Some("claude"));
        assert_eq!(action.args, vec!["code", "--task"]);
        assert_eq!(action.concurrency, Some(2));
        assert_eq!(action.on.as_deref(), Some("task.unblocked"));
        assert_eq!(action.env.get("CLAUDE_MODEL").unwrap(), "opus");
    }

    #[test]
    fn test_list_action_files_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let names = list_action_files_in_dir(tmp.path()).unwrap();
        assert!(names.is_empty());
    }

    #[test]
    fn test_list_action_files_nonexistent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("does-not-exist");
        let names = list_action_files_in_dir(&missing).unwrap();
        assert!(names.is_empty());
    }

    #[test]
    fn test_list_action_files_with_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("beta.toml"), "command = \"b\"\n").unwrap();
        std::fs::write(tmp.path().join("alpha.toml"), "command = \"a\"\n").unwrap();
        // Non-toml file should be ignored
        std::fs::write(tmp.path().join("readme.md"), "# hi").unwrap();

        let names = list_action_files_in_dir(tmp.path()).unwrap();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_get_action_inline_takes_precedence() {
        let tmp = tempfile::tempdir().unwrap();

        // File-based action
        std::fs::write(
            tmp.path().join("deploy.toml"),
            "command = \"file-deploy\"\n",
        )
        .unwrap();

        // Inline action with the same name
        let mut inline = std::collections::HashMap::new();
        inline.insert("deploy".to_string(), ActionConfig::new("inline-deploy"));

        let action = get_action_with(&inline, tmp.path(), "deploy")
            .unwrap()
            .expect("action should exist");
        assert_eq!(action.command.as_deref(), Some("inline-deploy"));
    }

    #[test]
    fn test_get_action_falls_back_to_file() {
        let tmp = tempfile::tempdir().unwrap();

        std::fs::write(
            tmp.path().join("deploy.toml"),
            "command = \"file-deploy\"\n",
        )
        .unwrap();

        let inline = std::collections::HashMap::new();

        let action = get_action_with(&inline, tmp.path(), "deploy")
            .unwrap()
            .expect("action should exist");
        assert_eq!(action.command.as_deref(), Some("file-deploy"));
    }

    #[test]
    fn test_get_action_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let inline = std::collections::HashMap::new();
        let action = get_action_with(&inline, tmp.path(), "missing").unwrap();
        assert!(action.is_none());
    }

    #[test]
    fn test_list_all_actions_merge() {
        let tmp = tempfile::tempdir().unwrap();

        // File-based actions
        std::fs::write(tmp.path().join("alpha.toml"), "command = \"file-alpha\"\n").unwrap();
        std::fs::write(
            tmp.path().join("shared.toml"),
            "command = \"file-shared\"\n",
        )
        .unwrap();

        // Inline actions (shared should override file)
        let mut inline = std::collections::HashMap::new();
        inline.insert("shared".to_string(), ActionConfig::new("inline-shared"));
        inline.insert("beta".to_string(), ActionConfig::new("inline-beta"));

        let all = list_all_actions_with(&inline, tmp.path()).unwrap();
        let names: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["alpha", "beta", "shared"]);

        // Verify sources
        assert_eq!(
            all.iter()
                .find(|(n, _)| n == "alpha")
                .unwrap()
                .1
                .command
                .as_deref(),
            Some("file-alpha")
        );
        assert_eq!(
            all.iter()
                .find(|(n, _)| n == "beta")
                .unwrap()
                .1
                .command
                .as_deref(),
            Some("inline-beta")
        );
        assert_eq!(
            all.iter()
                .find(|(n, _)| n == "shared")
                .unwrap()
                .1
                .command
                .as_deref(),
            Some("inline-shared")
        );
    }

    #[test]
    fn test_list_action_files_recursive() {
        let tmp = tempfile::tempdir().unwrap();
        // Flat actions
        std::fs::write(tmp.path().join("slack-message.toml"), "command = \"s\"\n").unwrap();
        // Namespaced actions
        std::fs::create_dir_all(tmp.path().join("git")).unwrap();
        std::fs::write(
            tmp.path().join("git/worktree-create.toml"),
            "command = \"g\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("agents")).unwrap();
        std::fs::write(
            tmp.path().join("agents/claude-work.toml"),
            "command = \"c\"\n",
        )
        .unwrap();
        // Non-toml in subdirectory should be ignored
        std::fs::write(tmp.path().join("git/README.md"), "# hi").unwrap();

        let names = list_action_files_in_dir(tmp.path()).unwrap();
        assert_eq!(
            names,
            vec!["agents/claude-work", "git/worktree-create", "slack-message"]
        );
    }

    #[test]
    fn test_load_action_namespaced() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("git")).unwrap();
        let content = "command = \"git\"\nargs = [\"worktree\", \"add\"]\n";
        std::fs::write(tmp.path().join("git/worktree-create.toml"), content).unwrap();

        let action = load_action_from_dir(tmp.path(), "git/worktree-create")
            .unwrap()
            .expect("namespaced action should exist");
        assert_eq!(action.command.as_deref(), Some("git"));
        assert_eq!(action.args, vec!["worktree", "add"]);
    }

    #[test]
    fn test_list_action_files_deeply_nested() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("a/b")).unwrap();
        std::fs::write(tmp.path().join("a/b/deep.toml"), "command = \"d\"\n").unwrap();
        std::fs::write(tmp.path().join("a/shallow.toml"), "command = \"s\"\n").unwrap();

        let names = list_action_files_in_dir(tmp.path()).unwrap();
        assert_eq!(names, vec!["a/b/deep", "a/shallow"]);
    }

    #[test]
    fn test_inline_config_namespaced_key() {
        let tmp = tempfile::tempdir().unwrap();
        let mut inline = std::collections::HashMap::new();
        inline.insert(
            "git/worktree-create".to_string(),
            ActionConfig::new("git-wt"),
        );

        let action = get_action_with(&inline, tmp.path(), "git/worktree-create")
            .unwrap()
            .expect("inline namespaced action should exist");
        assert_eq!(action.command.as_deref(), Some("git-wt"));
    }

    #[test]
    fn test_list_all_actions_with_namespaced() {
        let tmp = tempfile::tempdir().unwrap();
        // File-based namespaced action
        std::fs::create_dir_all(tmp.path().join("git")).unwrap();
        std::fs::write(
            tmp.path().join("git/worktree-create.toml"),
            "command = \"file-git\"\n",
        )
        .unwrap();
        // Flat file action
        std::fs::write(
            tmp.path().join("notify.toml"),
            "command = \"file-notify\"\n",
        )
        .unwrap();
        // Inline namespaced action (should override file)
        let mut inline = std::collections::HashMap::new();
        inline.insert(
            "git/worktree-create".to_string(),
            ActionConfig::new("inline-git"),
        );

        let all = list_all_actions_with(&inline, tmp.path()).unwrap();
        let names: Vec<&str> = all.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["git/worktree-create", "notify"]);

        // Inline should take precedence
        assert_eq!(
            all.iter()
                .find(|(n, _)| n == "git/worktree-create")
                .unwrap()
                .1
                .command
                .as_deref(),
            Some("inline-git")
        );
    }
}
