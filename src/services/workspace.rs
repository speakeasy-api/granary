use std::env;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::db::connection::{create_pool, run_migrations};
use crate::error::{GranaryError, Result};
use crate::services::global_config_service;
use crate::services::workspace_registry::WorkspaceRegistry;

/// The name of the workspace directory
pub const WORKSPACE_DIR: &str = ".granary";
/// The name of the database file
pub const DB_FILE: &str = "granary.db";
/// The name of the session pointer file
pub const SESSION_FILE: &str = "session";
/// Environment variable for workspace path override
pub const WORKSPACE_ENV: &str = "GRANARY_HOME";
/// Environment variable for current session
pub const SESSION_ENV: &str = "GRANARY_SESSION";

/// How this workspace was resolved
#[derive(Debug, Clone)]
pub enum WorkspaceMode {
    /// Default global workspace (~/.granary/granary.db)
    Default,
    /// Named workspace under ~/.granary/workspaces/<name>/
    Named(String),
    /// Local .granary/ directory in the project tree
    Local,
}

impl WorkspaceMode {
    /// Returns the mode as a display string: "default", "named", or "local"
    pub fn label(&self) -> &str {
        match self {
            WorkspaceMode::Default => "default",
            WorkspaceMode::Named(_) => "named",
            WorkspaceMode::Local => "local",
        }
    }
}

/// Workspace represents a Granary workspace directory
#[derive(Debug)]
pub struct Workspace {
    /// Workspace name (None for local-only workspaces)
    pub name: Option<String>,
    /// Root directory containing .granary/
    pub root: PathBuf,
    /// Path to .granary/ directory
    pub granary_dir: PathBuf,
    /// Path to the database file
    pub db_path: PathBuf,
    /// How this workspace was resolved
    pub mode: WorkspaceMode,
}

impl Workspace {
    /// Returns the display name for this workspace.
    /// Named workspaces use their name, default shows "default",
    /// local shows the root directory name.
    pub fn display_name(&self) -> String {
        match &self.mode {
            WorkspaceMode::Default => "default".to_string(),
            WorkspaceMode::Named(name) => name.clone(),
            WorkspaceMode::Local => self
                .root
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| self.root.display().to_string()),
        }
    }

    /// Find the workspace using the RFC resolution order:
    /// 1. GRANARY_HOME env var -> explicit override
    /// 2. Local .granary/ -> walk up from cwd, stop before $HOME
    /// 3. Registry lookup -> deepest matching root wins
    /// 4. Default -> ~/.granary/granary.db
    pub fn find() -> Result<Self> {
        // Step 1: GRANARY_HOME env var override
        if let Ok(path) = env::var(WORKSPACE_ENV) {
            let root = PathBuf::from(&path);
            let granary_dir = root.join(WORKSPACE_DIR);
            if granary_dir.exists() {
                return Ok(Self {
                    name: None,
                    root: root.clone(),
                    granary_dir: granary_dir.clone(),
                    db_path: granary_dir.join(DB_FILE),
                    mode: WorkspaceMode::Local,
                });
            }
            // GRANARY_HOME set but path doesn't exist — check if root itself
            // is a granary dir (e.g. GRANARY_HOME=~/.granary)
            if root.join(DB_FILE).exists() {
                return Ok(Self {
                    name: None,
                    root: root.clone(),
                    granary_dir: root.clone(),
                    db_path: root.join(DB_FILE),
                    mode: WorkspaceMode::Local,
                });
            }
            return Err(GranaryError::WorkspaceNotFound(path));
        }

        let cwd = env::current_dir()?;
        let home_dir = dirs::home_dir();

        // Step 2: Walk up from cwd looking for .granary/, stop BEFORE $HOME
        let mut current = cwd.as_path();
        loop {
            // Stop before $HOME — don't pick up ~/.granary as a local workspace
            if let Some(ref home) = home_dir
                && current == home.as_path()
            {
                break;
            }

            let granary_dir = current.join(WORKSPACE_DIR);
            if granary_dir.exists() && granary_dir.is_dir() {
                return Ok(Self {
                    name: None,
                    root: current.to_path_buf(),
                    granary_dir: granary_dir.clone(),
                    db_path: granary_dir.join(DB_FILE),
                    mode: WorkspaceMode::Local,
                });
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }

        // Step 3: Registry lookup — check if cwd or ancestor matches a registered root
        if let Ok(registry) = WorkspaceRegistry::load() {
            // lookup_root walks up from cwd and finds the deepest match
            if let Some(workspace_name) = registry.lookup_root(&cwd)
                && let Ok(db_path) = WorkspaceRegistry::workspace_db_path(workspace_name)
            {
                let ws_dir = db_path.parent().unwrap().to_path_buf();
                return Ok(Self {
                    name: Some(workspace_name.to_string()),
                    root: cwd.clone(),
                    granary_dir: ws_dir,
                    db_path,
                    mode: WorkspaceMode::Named(workspace_name.to_string()),
                });
            }
        }

        // Step 4: Default — use ~/.granary/granary.db
        let config_dir = global_config_service::config_dir()?;
        let db_path = config_dir.join(DB_FILE);
        // Auto-create ~/.granary/ if it doesn't exist
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir)?;
        }
        Ok(Self {
            name: None,
            root: home_dir.unwrap_or(cwd),
            granary_dir: config_dir,
            db_path,
            mode: WorkspaceMode::Default,
        })
    }

    /// Find workspace or create a local one at the specified path.
    /// Since find() now always resolves to a workspace (falling through to default),
    /// this is only needed for explicit `init` to create a local .granary/ directory.
    pub fn find_or_create(path: Option<&Path>) -> Result<Self> {
        let root = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

        let granary_dir = root.join(WORKSPACE_DIR);
        if granary_dir.exists() {
            // Already has a local workspace, just return it
            return Ok(Self {
                name: None,
                root: root.clone(),
                granary_dir: granary_dir.clone(),
                db_path: granary_dir.join(DB_FILE),
                mode: WorkspaceMode::Local,
            });
        }

        Self::create(&root)
    }

    /// Create a new workspace at the specified path
    pub fn create(root: &Path) -> Result<Self> {
        let granary_dir = root.join(WORKSPACE_DIR);

        if granary_dir.exists() {
            return Err(GranaryError::WorkspaceAlreadyExists(
                granary_dir.display().to_string(),
            ));
        }

        // Create the .granary directory
        std::fs::create_dir_all(&granary_dir)?;

        Ok(Self {
            name: None,
            root: root.to_path_buf(),
            granary_dir: granary_dir.clone(),
            db_path: granary_dir.join(DB_FILE),
            mode: WorkspaceMode::Local,
        })
    }

    /// Open an existing workspace at the specified path.
    ///
    /// Checks for a local `.granary/` directory first, then falls back to
    /// registry lookup for named workspaces whose data lives under
    /// `~/.granary/workspaces/<name>/`.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let granary_dir = root.join(WORKSPACE_DIR);

        // Local .granary/ directory exists — use it directly
        if granary_dir.exists() {
            return Ok(Self {
                name: None,
                root,
                granary_dir: granary_dir.clone(),
                db_path: granary_dir.join(DB_FILE),
                mode: WorkspaceMode::Local,
            });
        }

        // Fall back to registry: the path may be a root registered to a named workspace
        if let Ok(registry) = WorkspaceRegistry::load() {
            if let Some(workspace_name) = registry.lookup_root(&root) {
                if let Ok(db_path) = WorkspaceRegistry::workspace_db_path(workspace_name) {
                    let ws_dir = db_path.parent().unwrap().to_path_buf();
                    return Ok(Self {
                        name: Some(workspace_name.to_string()),
                        root,
                        granary_dir: ws_dir,
                        db_path,
                        mode: WorkspaceMode::Named(workspace_name.to_string()),
                    });
                }
            }
        }

        Err(GranaryError::WorkspaceNotFound(root.display().to_string()))
    }

    /// Initialize the database and run migrations
    pub async fn init_db(&self) -> Result<SqlitePool> {
        let pool = create_pool(&self.db_path).await?;
        run_migrations(&pool).await?;
        Ok(pool)
    }

    /// Get a connection pool to the database
    pub async fn pool(&self) -> Result<SqlitePool> {
        if !self.db_path.exists() {
            return Err(GranaryError::WorkspaceNotFound(
                self.db_path.display().to_string(),
            ));
        }
        let pool = create_pool(&self.db_path).await?;
        // Run migrations to ensure schema is up to date
        run_migrations(&pool).await?;
        Ok(pool)
    }

    /// Get the current session ID from file or environment
    pub fn current_session_id(&self) -> Option<String> {
        // Check environment variable first
        if let Ok(session_id) = env::var(SESSION_ENV) {
            return Some(session_id);
        }

        // Check session file
        let session_file = self.granary_dir.join(SESSION_FILE);
        if !session_file.exists() {
            return None;
        }
        if let Ok(content) = std::fs::read_to_string(&session_file) {
            let id = content.trim().to_string();
            if !id.is_empty() {
                return Some(id);
            }
        }

        None
    }

    /// Set the current session ID
    pub fn set_current_session(&self, session_id: &str) -> Result<()> {
        let session_file = self.granary_dir.join(SESSION_FILE);
        std::fs::write(&session_file, session_id)?;
        Ok(())
    }

    /// Clear the current session
    pub fn clear_current_session(&self) -> Result<()> {
        let session_file = self.granary_dir.join(SESSION_FILE);
        if session_file.exists() {
            std::fs::remove_file(&session_file)?;
        }
        Ok(())
    }

    /// Returns the matched root directory for named workspaces by looking up
    /// the registry. Returns None for default/local modes.
    pub fn matched_root(&self) -> Option<String> {
        if let WorkspaceMode::Named(name) = &self.mode
            && let Ok(registry) = crate::services::workspace_registry::WorkspaceRegistry::load()
        {
            for (path, ws_name) in &registry.roots {
                if ws_name == name {
                    return Some(path.display().to_string());
                }
            }
        }
        None
    }

    /// Run diagnostic checks on the workspace
    pub async fn doctor(&self) -> Result<Vec<DiagnosticResult>> {
        let mut results = Vec::new();

        // Workspace name
        results.push(DiagnosticResult {
            check: "Workspace".to_string(),
            status: DiagnosticStatus::Info,
            message: self.display_name(),
        });

        // Workspace mode
        results.push(DiagnosticResult {
            check: "Mode".to_string(),
            status: DiagnosticStatus::Info,
            message: self.mode.label().to_string(),
        });

        // Database path
        results.push(DiagnosticResult {
            check: "Database".to_string(),
            status: if self.db_path.exists() {
                DiagnosticStatus::Ok
            } else {
                DiagnosticStatus::Error
            },
            message: format!("{}", self.db_path.display()),
        });

        // Root directory for named workspaces
        if let Some(root) = self.matched_root() {
            results.push(DiagnosticResult {
                check: "Root".to_string(),
                status: DiagnosticStatus::Info,
                message: format!("{} (matched from registry)", root),
            });
        }

        // Check .granary directory
        results.push(DiagnosticResult {
            check: "Workspace directory".to_string(),
            status: if self.granary_dir.exists() {
                DiagnosticStatus::Ok
            } else {
                DiagnosticStatus::Error
            },
            message: format!("{}", self.granary_dir.display()),
        });

        // Check database file
        results.push(DiagnosticResult {
            check: "Database file".to_string(),
            status: if self.db_path.exists() {
                DiagnosticStatus::Ok
            } else {
                DiagnosticStatus::Error
            },
            message: format!("{}", self.db_path.display()),
        });

        // Check database connection
        match self.pool().await {
            Ok(pool) => {
                results.push(DiagnosticResult {
                    check: "Database connection".to_string(),
                    status: DiagnosticStatus::Ok,
                    message: "Connected successfully".to_string(),
                });

                // Check WAL mode
                let wal_check = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
                    .fetch_one(&pool)
                    .await;
                results.push(DiagnosticResult {
                    check: "WAL mode".to_string(),
                    status: match &wal_check {
                        Ok(mode) if mode.to_lowercase() == "wal" => DiagnosticStatus::Ok,
                        Ok(_) => DiagnosticStatus::Warning,
                        Err(_) => DiagnosticStatus::Error,
                    },
                    message: wal_check.unwrap_or_else(|e| e.to_string()),
                });

                // Check foreign keys
                let fk_check = sqlx::query_scalar::<_, i32>("PRAGMA foreign_keys")
                    .fetch_one(&pool)
                    .await;
                let (fk_status, fk_message) = match &fk_check {
                    Ok(1) => (DiagnosticStatus::Ok, "Enabled".to_string()),
                    Ok(_) => (DiagnosticStatus::Warning, "Disabled".to_string()),
                    Err(e) => (DiagnosticStatus::Error, e.to_string()),
                };
                results.push(DiagnosticResult {
                    check: "Foreign keys".to_string(),
                    status: fk_status,
                    message: fk_message,
                });

                // Count entities
                let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);
                let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);
                let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);

                results.push(DiagnosticResult {
                    check: "Data summary".to_string(),
                    status: DiagnosticStatus::Ok,
                    message: format!(
                        "{} projects, {} tasks, {} sessions",
                        project_count, task_count, session_count
                    ),
                });
            }
            Err(e) => {
                results.push(DiagnosticResult {
                    check: "Database connection".to_string(),
                    status: DiagnosticStatus::Error,
                    message: e.to_string(),
                });
            }
        }

        // Check current session
        let session_status = match self.current_session_id() {
            Some(id) => DiagnosticResult {
                check: "Current session".to_string(),
                status: DiagnosticStatus::Ok,
                message: id,
            },
            None => DiagnosticResult {
                check: "Current session".to_string(),
                status: DiagnosticStatus::Info,
                message: "None".to_string(),
            },
        };
        results.push(session_status);

        Ok(results)
    }
}

#[derive(Debug, Clone)]
pub enum DiagnosticStatus {
    Ok,
    Warning,
    Error,
    Info,
}

#[derive(Debug)]
pub struct DiagnosticResult {
    pub check: String,
    pub status: DiagnosticStatus,
    pub message: String,
}

impl DiagnosticResult {
    pub fn status_symbol(&self) -> &'static str {
        match self.status {
            DiagnosticStatus::Ok => "[OK]",
            DiagnosticStatus::Warning => "[WARN]",
            DiagnosticStatus::Error => "[ERR]",
            DiagnosticStatus::Info => "[INFO]",
        }
    }
}
