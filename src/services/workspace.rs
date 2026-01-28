use std::env;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::db::connection::{create_pool, run_migrations};
use crate::error::{GranaryError, Result};

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

/// Workspace represents a Granary workspace directory
#[derive(Debug)]
pub struct Workspace {
    /// Root directory containing .granary/
    pub root: PathBuf,
    /// Path to .granary/ directory
    pub granary_dir: PathBuf,
    /// Path to the database file
    pub db_path: PathBuf,
}

impl Workspace {
    /// Find the workspace by walking up from the current directory
    /// Similar to how Git finds .git/
    pub fn find() -> Result<Self> {
        // Check for environment variable override first
        if let Ok(path) = env::var(WORKSPACE_ENV) {
            let root = PathBuf::from(path);
            let granary_dir = root.join(WORKSPACE_DIR);
            if granary_dir.exists() {
                return Ok(Self {
                    root: root.clone(),
                    granary_dir: granary_dir.clone(),
                    db_path: granary_dir.join(DB_FILE),
                });
            }
        }

        // Walk up from current directory
        let cwd = env::current_dir()?;
        let mut current = cwd.as_path();

        loop {
            let granary_dir = current.join(WORKSPACE_DIR);
            if granary_dir.exists() && granary_dir.is_dir() {
                return Ok(Self {
                    root: current.to_path_buf(),
                    granary_dir: granary_dir.clone(),
                    db_path: granary_dir.join(DB_FILE),
                });
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }

        Err(GranaryError::WorkspaceNotFound)
    }

    /// Find workspace or create one at the specified path
    pub fn find_or_create(path: Option<&Path>) -> Result<Self> {
        // Try to find existing workspace first
        if let Ok(ws) = Self::find() {
            return Ok(ws);
        }

        // Create new workspace
        let root = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

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
            root: root.to_path_buf(),
            granary_dir: granary_dir.clone(),
            db_path: granary_dir.join(DB_FILE),
        })
    }

    /// Open an existing workspace at the specified path.
    ///
    /// Unlike `find()`, this does not walk up the directory tree.
    /// The path should be the root directory containing `.granary/`.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let granary_dir = root.join(WORKSPACE_DIR);

        if !granary_dir.exists() {
            return Err(GranaryError::WorkspaceNotFound);
        }

        Ok(Self {
            root,
            granary_dir: granary_dir.clone(),
            db_path: granary_dir.join(DB_FILE),
        })
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
            return Err(GranaryError::WorkspaceNotFound);
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

    /// Run diagnostic checks on the workspace
    pub async fn doctor(&self) -> Result<Vec<DiagnosticResult>> {
        let mut results = Vec::new();

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
