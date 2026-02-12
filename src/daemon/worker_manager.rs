//! Worker manager for coordinating worker lifecycles within the daemon.
//!
//! The WorkerManager is the core daemon component that:
//! - Starts and stops workers
//! - Tracks active worker handles with shutdown channels
//! - Manages graceful shutdown of all workers
//!
//! It reuses the existing `WorkerRuntime` for actual worker execution.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::{RwLock, watch};
use tokio::task::JoinHandle;

use crate::daemon::protocol::{LogTarget, LogsResponse};
use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::run::{Run, RunStatus, UpdateRunStatus};
use crate::models::{CreateWorker, UpdateWorkerStatus, Worker, WorkerStatus};
use crate::services::Workspace;
use crate::services::global_config as global_config_service;
use crate::services::worker_runtime::{WorkerRuntime, WorkerRuntimeConfig};

/// Handle to a running worker, containing the task handle and shutdown channel.
struct WorkerHandle {
    /// The worker ID this handle corresponds to
    #[allow(dead_code)]
    worker_id: String,
    /// The spawned tokio task running the worker
    task: JoinHandle<Result<()>>,
    /// Sender to signal shutdown to the worker
    shutdown_tx: watch::Sender<bool>,
}

/// Manages all worker lifecycles within the daemon.
///
/// The WorkerManager owns worker handles and provides methods to start, stop,
/// and query workers. It coordinates with the database for persistence and
/// uses WorkerRuntime for actual worker execution.
///
/// # Thread Safety
///
/// WorkerManager uses `RwLock` to allow concurrent reads while serializing writes
/// to the workers HashMap. This is important since multiple IPC clients may
/// query worker state simultaneously.
pub struct WorkerManager {
    /// Global database pool for worker/run persistence
    global_pool: SqlitePool,
    /// Map of worker ID to active handle
    workers: RwLock<HashMap<String, WorkerHandle>>,
}

impl WorkerManager {
    /// Create a new WorkerManager with the given global database pool.
    ///
    /// # Arguments
    ///
    /// * `global_pool` - Connection pool for the global database (~/.granary/workers.db)
    pub fn new(global_pool: SqlitePool) -> Self {
        Self {
            global_pool,
            workers: RwLock::new(HashMap::new()),
        }
    }

    /// Start a new worker.
    ///
    /// This method:
    /// 1. Creates a database record for the worker
    /// 2. Opens the workspace and gets its database pool
    /// 3. Creates a WorkerRuntime with a shutdown channel
    /// 4. Spawns the runtime as a tokio task
    /// 5. Tracks the handle in our internal HashMap
    ///
    /// # Arguments
    ///
    /// * `create` - Worker creation parameters
    ///
    /// # Returns
    ///
    /// The created Worker record.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Database operations fail
    /// - The workspace cannot be opened
    /// - The log directory cannot be created
    pub async fn start_worker(&self, create: CreateWorker) -> Result<Worker> {
        // 1. Create DB record
        let worker = db::workers::create(&self.global_pool, &create).await?;

        // 2. Get workspace pool
        let workspace = Workspace::open(&worker.instance_path)?;
        let workspace_pool = workspace.pool().await?;

        // 3. Create runtime with shutdown channel
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let log_dir = global_config_service::worker_logs_dir(&worker.id)?;
        std::fs::create_dir_all(&log_dir)?;

        let config = WorkerRuntimeConfig {
            log_dir: Some(log_dir),
            since: create.since,
            ..Default::default()
        };

        let mut runtime = WorkerRuntime::new(
            worker.clone(),
            self.global_pool.clone(),
            workspace_pool,
            shutdown_rx,
            config,
        )
        .await?;

        // 4. Spawn as tokio task
        let worker_id = worker.id.clone();
        let task = tokio::spawn(async move { runtime.run().await });

        // 5. Track handle
        let handle = WorkerHandle {
            worker_id: worker_id.clone(),
            task,
            shutdown_tx,
        };

        self.workers.write().await.insert(worker_id, handle);

        Ok(worker)
    }

    /// Stop a worker by ID.
    ///
    /// This method:
    /// 1. Removes the worker handle from our tracking HashMap
    /// 2. Signals shutdown via the watch channel
    /// 3. Waits for the task to complete (with 30 second timeout)
    /// 4. Updates the database status to stopped
    /// 5. Optionally cancels active runs
    ///
    /// # Arguments
    ///
    /// * `worker_id` - The ID of the worker to stop
    /// * `stop_runs` - If true, also cancel any active runs for this worker
    ///
    /// # Errors
    ///
    /// Returns an error if database operations fail.
    pub async fn stop_worker(&self, worker_id: &str, stop_runs: bool) -> Result<()> {
        let mut workers = self.workers.write().await;

        if let Some(handle) = workers.remove(worker_id) {
            // Signal shutdown
            let _ = handle.shutdown_tx.send(true);

            // Wait for task to complete (with timeout)
            let _ = tokio::time::timeout(Duration::from_secs(30), handle.task).await;
        }

        // Update DB status
        let update = UpdateWorkerStatus {
            status: WorkerStatus::Stopped,
            error_message: None,
            pid: None,
        };
        db::workers::update_status(&self.global_pool, worker_id, &update).await?;

        // Optionally cancel active runs
        if stop_runs {
            db::runs::cancel_by_worker(&self.global_pool, worker_id).await?;
        }

        Ok(())
    }

    /// Get a worker by ID from the database.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - The ID of the worker to retrieve
    ///
    /// # Returns
    ///
    /// The Worker if found, None otherwise.
    pub async fn get_worker(&self, worker_id: &str) -> Result<Option<Worker>> {
        db::workers::get(&self.global_pool, worker_id).await
    }

    /// List workers from the database.
    ///
    /// # Arguments
    ///
    /// * `include_all` - If true, include stopped/errored workers.
    ///   If false, only return running/pending workers.
    ///
    /// # Returns
    ///
    /// A vector of Worker records.
    pub async fn list_workers(&self, include_all: bool) -> Result<Vec<Worker>> {
        if include_all {
            db::workers::list(&self.global_pool).await
        } else {
            db::workers::list_active(&self.global_pool).await
        }
    }

    /// Shutdown all workers gracefully.
    ///
    /// This method:
    /// 1. Signals shutdown to all active workers
    /// 2. Waits for all tasks to complete (with 30 second timeout)
    ///
    /// This is typically called when the daemon itself is shutting down.
    ///
    /// # Errors
    ///
    /// This method currently does not return errors, but the Result type
    /// is kept for future extensibility.
    pub async fn shutdown_all(&self) -> Result<()> {
        let mut workers = self.workers.write().await;

        // Signal all workers to stop
        for handle in workers.values() {
            let _ = handle.shutdown_tx.send(true);
        }

        // Collect all task handles
        let handles: Vec<_> = workers.drain().map(|(_, h)| h.task).collect();

        if handles.is_empty() {
            return Ok(());
        }

        // Wait for all tasks with timeout using JoinSet
        let deadline = Duration::from_secs(30);
        let mut join_set = tokio::task::JoinSet::new();

        for handle in handles {
            join_set.spawn(async move {
                // Wrap the handle in our own future that we can await
                let _ = handle.await;
            });
        }

        // Wait for all tasks with timeout
        let _ = tokio::time::timeout(deadline, async {
            while join_set.join_next().await.is_some() {}
        })
        .await;

        Ok(())
    }

    /// Check if a worker is running in this daemon instance.
    ///
    /// Note: This only checks if the worker is tracked in this daemon's
    /// in-memory HashMap. A worker may be marked as "running" in the database
    /// but not tracked here if it was started by a different daemon instance.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - The ID of the worker to check
    ///
    /// # Returns
    ///
    /// True if the worker is actively tracked by this daemon.
    pub async fn is_worker_running(&self, worker_id: &str) -> bool {
        self.workers.read().await.contains_key(worker_id)
    }

    /// Get the number of active workers tracked by this daemon.
    pub async fn active_worker_count(&self) -> usize {
        self.workers.read().await.len()
    }

    /// Restore workers that were running when the daemon last stopped.
    ///
    /// This method is called on daemon startup to resume workers that were
    /// previously in "running" state. This ensures workers survive daemon restarts.
    ///
    /// # Behavior
    ///
    /// For each worker in "running" state:
    /// 1. Check if the workspace directory still exists
    /// 2. If missing, mark the worker as error with appropriate message
    /// 3. If present, attempt to restart the worker using `start_existing_worker`
    /// 4. Log restoration results
    ///
    /// # Errors
    ///
    /// Returns an error if database operations fail. Individual worker
    /// restoration failures are logged but don't cause the method to fail.
    pub async fn restore_workers(&self) -> Result<()> {
        // Find workers that were running when daemon last stopped
        let workers = db::workers::list_by_status(&self.global_pool, WorkerStatus::Running).await?;

        if workers.is_empty() {
            return Ok(());
        }

        let mut restored = 0;
        let mut errors = 0;

        for worker in workers {
            // Check if workspace still exists
            let workspace_path = std::path::Path::new(&worker.instance_path);
            if !workspace_path.exists() {
                eprintln!(
                    "[daemon] Worker {} workspace missing, marking as error",
                    worker.id
                );
                let update = UpdateWorkerStatus {
                    status: WorkerStatus::Error,
                    error_message: Some("Workspace directory missing".to_string()),
                    pid: None,
                };
                db::workers::update_status(&self.global_pool, &worker.id, &update).await?;
                errors += 1;
                continue;
            }

            // Try to restart the worker
            match self.start_existing_worker(worker.clone()).await {
                Ok(()) => {
                    eprintln!("[daemon] Restored worker {}", worker.id);
                    restored += 1;
                }
                Err(e) => {
                    eprintln!("[daemon] Failed to restore worker {}: {}", worker.id, e);
                    let update = UpdateWorkerStatus {
                        status: WorkerStatus::Error,
                        error_message: Some(format!("Failed to restore: {}", e)),
                        pid: None,
                    };
                    db::workers::update_status(&self.global_pool, &worker.id, &update).await?;
                    errors += 1;
                }
            }
        }

        if restored > 0 || errors > 0 {
            eprintln!("[daemon] Restored {} workers, {} errors", restored, errors);
        }

        Ok(())
    }

    /// Start an existing worker (used for restoration and manual restart).
    ///
    /// Unlike `start_worker`, this method does not create a new database record.
    /// Instead, it resumes a worker from an existing record.
    ///
    /// # Arguments
    ///
    /// * `worker` - The existing worker record to restart
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The workspace cannot be opened
    /// - The log directory cannot be created
    /// - The worker runtime cannot be created
    async fn start_existing_worker(&self, worker: Worker) -> Result<()> {
        // Get workspace pool
        let workspace = Workspace::open(&worker.instance_path)?;
        let workspace_pool = workspace.pool().await?;

        // Create runtime with shutdown channel
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let log_dir = global_config_service::worker_logs_dir(&worker.id)?;
        std::fs::create_dir_all(&log_dir)?;

        let config = WorkerRuntimeConfig {
            log_dir: Some(log_dir),
            ..Default::default()
        };

        let mut runtime = WorkerRuntime::new(
            worker.clone(),
            self.global_pool.clone(),
            workspace_pool,
            shutdown_rx,
            config,
        )
        .await?;

        // Spawn as tokio task
        let worker_id = worker.id.clone();
        let task = tokio::spawn(async move { runtime.run().await });

        // Track handle
        let handle = WorkerHandle {
            worker_id: worker_id.clone(),
            task,
            shutdown_tx,
        };

        self.workers.write().await.insert(worker_id, handle);

        Ok(())
    }

    // ========================================================================
    // Run management methods
    // ========================================================================

    /// Get a run by ID.
    ///
    /// # Arguments
    ///
    /// * `run_id` - The ID of the run to retrieve
    ///
    /// # Returns
    ///
    /// The Run if found, None otherwise.
    pub async fn get_run(&self, run_id: &str) -> Result<Option<Run>> {
        db::runs::get(&self.global_pool, run_id).await
    }

    /// List runs, optionally filtered by worker or status.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - Optional filter by worker ID
    /// * `status` - Optional filter by status string (e.g., "running", "completed")
    /// * `all` - If true, include completed/failed/cancelled runs; otherwise only active runs
    ///
    /// # Returns
    ///
    /// A vector of Run records matching the filter criteria.
    pub async fn list_runs(
        &self,
        worker_id: Option<&str>,
        status: Option<&str>,
        all: bool,
    ) -> Result<Vec<Run>> {
        // Parse status if provided
        let status_enum: Option<RunStatus> =
            if let Some(s) = status {
                Some(s.parse().map_err(|_| {
                    GranaryError::InvalidArgument(format!("Invalid run status: {}", s))
                })?)
            } else {
                None
            };

        match (worker_id, status_enum) {
            (Some(wid), Some(s)) => {
                db::runs::list_by_worker_and_status(&self.global_pool, wid, s).await
            }
            (Some(wid), None) => {
                let runs = db::runs::list_by_worker(&self.global_pool, wid).await?;
                if all {
                    Ok(runs)
                } else {
                    // Filter to active runs only
                    Ok(runs
                        .into_iter()
                        .filter(|r| {
                            matches!(
                                r.status_enum(),
                                RunStatus::Pending | RunStatus::Running | RunStatus::Paused
                            )
                        })
                        .collect())
                }
            }
            (None, Some(s)) => db::runs::list_by_status(&self.global_pool, s).await,
            (None, None) => {
                if all {
                    db::runs::list_all(&self.global_pool).await
                } else {
                    db::runs::list_active(&self.global_pool).await
                }
            }
        }
    }

    /// Stop a specific run by ID.
    ///
    /// This method:
    /// 1. Finds the run in the database
    /// 2. If the run has a PID, sends SIGTERM to the process
    /// 3. Updates the run status to cancelled
    ///
    /// # Arguments
    ///
    /// * `run_id` - The ID of the run to stop
    ///
    /// # Errors
    ///
    /// Returns an error if the run is not found or database operations fail.
    pub async fn stop_run(&self, run_id: &str) -> Result<()> {
        let run = db::runs::get(&self.global_pool, run_id)
            .await?
            .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

        // Check if run is already finished
        if run.is_finished() {
            return Err(GranaryError::InvalidArgument(format!(
                "Run {} is already finished (status: {})",
                run_id, run.status
            )));
        }

        // If run has a PID, try to kill the process
        if let Some(pid) = run.pid {
            kill_process(pid as u32, ProcessSignal::Term);
        }

        // Update status to cancelled
        let update = UpdateRunStatus {
            status: RunStatus::Cancelled,
            exit_code: None,
            error_message: Some("Stopped by user".to_string()),
            pid: None,
        };
        db::runs::update_status(&self.global_pool, run_id, &update).await?;

        Ok(())
    }

    /// Pause a running run.
    ///
    /// Sends SIGSTOP to the run's process and updates the status to paused.
    ///
    /// # Arguments
    ///
    /// * `run_id` - The ID of the run to pause
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The run is not found
    /// - The run is not in 'running' status
    /// - The run has no PID
    pub async fn pause_run(&self, run_id: &str) -> Result<()> {
        let run = db::runs::get(&self.global_pool, run_id)
            .await?
            .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

        // Check if run is running
        if run.status_enum() != RunStatus::Running {
            return Err(GranaryError::InvalidArgument(format!(
                "Cannot pause run: status is '{}', must be 'running'",
                run.status
            )));
        }

        // Get PID
        let pid = run.pid.ok_or_else(|| {
            GranaryError::InvalidArgument("Run has no PID, cannot pause".to_string())
        })?;

        // Send SIGSTOP
        kill_process(pid as u32, ProcessSignal::Stop);

        // Update status to paused
        let update = UpdateRunStatus {
            status: RunStatus::Paused,
            exit_code: None,
            error_message: None,
            pid: run.pid,
        };
        db::runs::update_status(&self.global_pool, run_id, &update).await?;

        Ok(())
    }

    /// Resume a paused run.
    ///
    /// Sends SIGCONT to the run's process and updates the status to running.
    ///
    /// # Arguments
    ///
    /// * `run_id` - The ID of the run to resume
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The run is not found
    /// - The run is not in 'paused' status
    /// - The run has no PID
    pub async fn resume_run(&self, run_id: &str) -> Result<()> {
        let run = db::runs::get(&self.global_pool, run_id)
            .await?
            .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

        // Check if run is paused
        if run.status_enum() != RunStatus::Paused {
            return Err(GranaryError::InvalidArgument(format!(
                "Cannot resume run: status is '{}', must be 'paused'",
                run.status
            )));
        }

        // Get PID
        let pid = run.pid.ok_or_else(|| {
            GranaryError::InvalidArgument("Run has no PID, cannot resume".to_string())
        })?;

        // Send SIGCONT
        kill_process(pid as u32, ProcessSignal::Cont);

        // Update status to running
        let update = UpdateRunStatus {
            status: RunStatus::Running,
            exit_code: None,
            error_message: None,
            pid: run.pid,
        };
        db::runs::update_status(&self.global_pool, run_id, &update).await?;

        Ok(())
    }

    /// Get the log path for a run.
    ///
    /// # Arguments
    ///
    /// * `run_id` - The ID of the run
    ///
    /// # Returns
    ///
    /// The path to the run's log file, or an error if the run is not found.
    pub async fn get_run_log_path(&self, run_id: &str) -> Result<Option<std::path::PathBuf>> {
        let run = db::runs::get(&self.global_pool, run_id)
            .await?
            .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

        if let Some(ref path) = run.log_path {
            Ok(Some(std::path::PathBuf::from(path)))
        } else {
            // Fallback: try to find log in worker's log directory
            let log_dir = global_config_service::worker_logs_dir(&run.worker_id)?;
            let log_path = log_dir.join(format!("{}.log", run_id));
            if log_path.exists() {
                Ok(Some(log_path))
            } else {
                Ok(None)
            }
        }
    }

    // ========================================================================
    // Log streaming methods
    // ========================================================================

    /// Get log lines for a worker or run with offset-based pagination.
    ///
    /// This method supports streaming logs by returning lines after a given offset.
    /// The client can repeatedly call this with the returned `next_line` value to
    /// follow logs in real-time.
    ///
    /// # Arguments
    ///
    /// * `target_id` - The worker_id or run_id
    /// * `target_type` - Whether this is a worker or run log request
    /// * `since_line` - Return lines after this line number (0-indexed)
    /// * `limit` - Maximum number of lines to return
    ///
    /// # Returns
    ///
    /// A `LogsResponse` containing the requested log lines and metadata for
    /// subsequent polling requests.
    pub async fn get_logs(
        &self,
        target_id: &str,
        target_type: LogTarget,
        since_line: u64,
        limit: u64,
    ) -> Result<LogsResponse> {
        let log_path = match target_type {
            LogTarget::Worker => {
                // Worker log is always at ~/.granary/workers/{worker_id}/worker.log
                global_config_service::worker_logs_dir(target_id)?.join("worker.log")
            }
            LogTarget::Run => {
                // Get run to find log path
                let run = db::runs::get(&self.global_pool, target_id)
                    .await?
                    .ok_or_else(|| GranaryError::RunNotFound(target_id.to_string()))?;

                if let Some(ref path) = run.log_path {
                    PathBuf::from(path)
                } else {
                    // Fallback: try worker's log directory
                    let log_dir = global_config_service::worker_logs_dir(&run.worker_id)?;
                    log_dir.join(format!("{}.log", target_id))
                }
            }
        };

        // If log file doesn't exist, return empty response
        if !log_path.exists() {
            return Ok(LogsResponse {
                lines: vec![],
                next_line: 0,
                has_more: self.is_target_active(target_id, &target_type).await,
                log_path: Some(log_path),
            });
        }

        // Read lines from file starting at since_line
        let file = std::fs::File::open(&log_path)?;
        let reader = BufReader::new(file);

        let lines: Vec<String> = reader
            .lines()
            .skip(since_line as usize)
            .take(limit as usize)
            .collect::<std::io::Result<_>>()?;

        let next_line = since_line + lines.len() as u64;

        // Check if target is still active (more logs might come)
        let has_more = self.is_target_active(target_id, &target_type).await;

        Ok(LogsResponse {
            lines,
            next_line,
            has_more,
            log_path: Some(log_path),
        })
    }

    /// Check if a target (worker or run) is still active and may produce more logs.
    async fn is_target_active(&self, target_id: &str, target_type: &LogTarget) -> bool {
        match target_type {
            LogTarget::Worker => {
                // Check if worker is running in this daemon instance
                if self.is_worker_running(target_id).await {
                    return true;
                }
                // Also check database status
                if let Ok(Some(worker)) = db::workers::get(&self.global_pool, target_id).await {
                    matches!(worker.status.as_str(), "running" | "pending" | "starting")
                } else {
                    false
                }
            }
            LogTarget::Run => {
                if let Ok(Some(run)) = db::runs::get(&self.global_pool, target_id).await {
                    matches!(
                        run.status_enum(),
                        RunStatus::Pending | RunStatus::Running | RunStatus::Paused
                    )
                } else {
                    false
                }
            }
        }
    }

    /// Get the worker log path for a given worker ID.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - The ID of the worker
    ///
    /// # Returns
    ///
    /// The path to the worker's log file.
    pub fn get_worker_log_path(&self, worker_id: &str) -> Result<PathBuf> {
        Ok(global_config_service::worker_logs_dir(worker_id)?.join("worker.log"))
    }

    /// Prune stopped and errored workers, their runs, and log files.
    ///
    /// This method cleans up workers that are no longer active by:
    /// 1. Finding all workers with "stopped" or "error" status
    /// 2. Deleting their associated run records from the database
    /// 3. Removing their log directories from disk
    /// 4. Deleting the worker records from the database
    ///
    /// # Returns
    ///
    /// The number of workers that were pruned.
    ///
    /// # Errors
    ///
    /// Returns an error if database operations fail. Log directory removal
    /// failures are logged but do not cause the method to fail.
    pub async fn prune_workers(&self) -> Result<i32> {
        // Find workers with stopped or error status
        let stopped_workers =
            db::workers::list_by_status(&self.global_pool, WorkerStatus::Stopped).await?;
        let mut error_workers =
            db::workers::list_by_status(&self.global_pool, WorkerStatus::Error).await?;

        let mut all_workers = stopped_workers;
        all_workers.append(&mut error_workers);

        let mut pruned = 0;
        for worker in all_workers {
            // Delete runs for this worker
            db::runs::delete_by_worker(&self.global_pool, &worker.id).await?;

            // Delete log directory
            if let Ok(log_dir) = global_config_service::worker_logs_dir(&worker.id)
                && log_dir.exists()
            {
                let _ = std::fs::remove_dir_all(&log_dir);
            }

            // Delete worker record
            db::workers::delete(&self.global_pool, &worker.id).await?;
            pruned += 1;
        }

        Ok(pruned)
    }

    // ========================================================================
    // Log retention and cleanup methods
    // ========================================================================

    /// Clean up old log files based on retention policy.
    ///
    /// This method enforces the log retention policy by:
    /// 1. Iterating through all worker log directories
    /// 2. Deleting log files older than `max_age_days`
    /// 3. Keeping only the most recent `max_files_per_worker` files per worker
    ///
    /// # Arguments
    ///
    /// * `config` - The log retention configuration specifying cleanup thresholds
    ///
    /// # Returns
    ///
    /// The number of log files that were deleted.
    ///
    /// # Errors
    ///
    /// Returns an error if the logs directory cannot be read. Individual file
    /// deletion failures are silently ignored to ensure cleanup continues.
    pub fn cleanup_old_logs(&self, config: &crate::models::LogRetentionConfig) -> Result<u64> {
        let logs_base_dir = global_config_service::logs_dir()?;

        // If logs directory doesn't exist, nothing to clean
        if !logs_base_dir.exists() {
            return Ok(0);
        }

        let max_age_secs = config.max_age_days * 86400;
        let mut deleted = 0u64;

        // Iterate through worker directories
        let entries = match std::fs::read_dir(&logs_base_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(0),
        };

        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }

            let worker_dir = entry.path();
            deleted +=
                self.cleanup_worker_logs(&worker_dir, max_age_secs, config.max_files_per_worker);
        }

        Ok(deleted)
    }

    /// Clean up log files in a single worker's log directory.
    ///
    /// Deletes files that are either:
    /// - Older than the maximum age threshold
    /// - Exceeding the maximum file count (oldest files first)
    ///
    /// # Arguments
    ///
    /// * `worker_dir` - Path to the worker's log directory
    /// * `max_age_secs` - Maximum age in seconds for log files
    /// * `max_files` - Maximum number of log files to keep
    ///
    /// # Returns
    ///
    /// The number of files deleted from this worker directory.
    fn cleanup_worker_logs(
        &self,
        worker_dir: &std::path::Path,
        max_age_secs: u64,
        max_files: usize,
    ) -> u64 {
        let entries = match std::fs::read_dir(worker_dir) {
            Ok(entries) => entries,
            Err(_) => return 0,
        };

        // Collect all log files with their modification times
        let mut log_files: Vec<(std::path::PathBuf, std::time::SystemTime)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
            .filter_map(|e| {
                let path = e.path();
                let modified = e.metadata().ok()?.modified().ok()?;
                Some((path, modified))
            })
            .collect();

        // Sort by modification time (oldest first)
        log_files.sort_by_key(|(_, modified)| *modified);

        let now = std::time::SystemTime::now();
        let mut deleted = 0u64;
        let total_files = log_files.len();

        for (i, (path, modified)) in log_files.iter().enumerate() {
            // Check if file is too old
            let is_too_old = now
                .duration_since(*modified)
                .map(|d| d.as_secs() > max_age_secs)
                .unwrap_or(false);

            // Check if we have too many files (keep the newest max_files)
            let exceeds_max_count = total_files > max_files && i < (total_files - max_files);

            if (is_too_old || exceeds_max_count) && std::fs::remove_file(path).is_ok() {
                deleted += 1;
            }
        }

        deleted
    }
}

/// Signal types for process control
enum ProcessSignal {
    Term,
    Stop,
    Cont,
}

/// Send a signal to a process group.
///
/// On Unix, signals are sent to the entire process group (using negative PID).
/// This ensures that when a runner spawns child processes, they all receive
/// the signal when the run is stopped/paused/resumed.
///
/// On Windows, for TERM signals, taskkill /T is used to kill the process tree.
fn kill_process(pid: u32, signal: ProcessSignal) {
    #[cfg(unix)]
    {
        let sig = match signal {
            ProcessSignal::Term => "-TERM",
            ProcessSignal::Stop => "-STOP",
            ProcessSignal::Cont => "-CONT",
        };
        // Negative PID means kill entire process group
        // The process group ID equals the PID of the group leader (our spawned process)
        // because we used setsid() when spawning
        let pgid = format!("-{}", pid);
        let _ = std::process::Command::new("kill")
            .args([sig, &pgid])
            .output();
    }

    #[cfg(not(unix))]
    {
        // On Windows, use taskkill /T to kill the entire process tree
        if matches!(signal, ProcessSignal::Term) {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .output();
        }
        // STOP and CONT are not easily supported on Windows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::{create_pool, run_migrations};
    use tempfile::tempdir;

    async fn setup_test_db() -> (SqlitePool, tempfile::TempDir) {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_pool(&db_path).await.unwrap();
        run_migrations(&pool).await.unwrap();
        (pool, temp_dir)
    }

    #[tokio::test]
    async fn test_worker_manager_new() {
        let (pool, _temp) = setup_test_db().await;
        let manager = WorkerManager::new(pool);
        assert_eq!(manager.active_worker_count().await, 0);
    }

    #[tokio::test]
    async fn test_is_worker_running_not_tracked() {
        let (pool, _temp) = setup_test_db().await;
        let manager = WorkerManager::new(pool);
        assert!(!manager.is_worker_running("nonexistent").await);
    }

    #[tokio::test]
    async fn test_list_workers_empty() {
        let (pool, _temp) = setup_test_db().await;
        let manager = WorkerManager::new(pool);

        let workers = manager.list_workers(true).await.unwrap();
        assert!(workers.is_empty());

        let active_workers = manager.list_workers(false).await.unwrap();
        assert!(active_workers.is_empty());
    }

    #[tokio::test]
    async fn test_shutdown_all_empty() {
        let (pool, _temp) = setup_test_db().await;
        let manager = WorkerManager::new(pool);

        // Should not panic when no workers are running
        manager.shutdown_all().await.unwrap();
    }

    #[tokio::test]
    async fn test_restore_workers_empty() {
        let (pool, _temp) = setup_test_db().await;
        let manager = WorkerManager::new(pool);

        // Should succeed when no workers need restoration
        manager.restore_workers().await.unwrap();
        assert_eq!(manager.active_worker_count().await, 0);
    }

    #[tokio::test]
    async fn test_restore_workers_missing_workspace() {
        let (pool, _temp) = setup_test_db().await;

        // Manually insert a worker with "running" status but missing workspace
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO workers (id, command, args, event_type, filters, concurrency,
                instance_path, status, detached, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind("worker-test123")
        .bind("echo")
        .bind("[]")
        .bind("task.created")
        .bind("[]")
        .bind(1)
        .bind("/nonexistent/workspace/path")
        .bind("running")
        .bind(false)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let manager = WorkerManager::new(pool.clone());

        // Should succeed but mark the worker as error
        manager.restore_workers().await.unwrap();

        // Worker should not be tracked (failed to restore)
        assert_eq!(manager.active_worker_count().await, 0);

        // Worker in database should be marked as error
        let worker = db::workers::get(&pool, "worker-test123")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(worker.status, "error");
        assert!(
            worker
                .error_message
                .unwrap()
                .contains("Workspace directory missing")
        );
    }

    #[tokio::test]
    async fn test_cleanup_worker_logs_by_count() {
        let (pool, temp_dir) = setup_test_db().await;
        let manager = WorkerManager::new(pool);

        // Create a fake worker log directory
        let worker_dir = temp_dir.path().join("worker-test");
        std::fs::create_dir_all(&worker_dir).unwrap();

        // Create multiple log files
        for i in 0..5 {
            let log_path = worker_dir.join(format!("run-{}.log", i));
            std::fs::write(&log_path, format!("Log content {}", i)).unwrap();
            // Add small delay to ensure different modification times
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Verify all files were created
        let files_before: Vec<_> = std::fs::read_dir(&worker_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files_before.len(), 5);

        // Cleanup with max 3 files
        let deleted = manager.cleanup_worker_logs(&worker_dir, u64::MAX, 3);
        assert_eq!(deleted, 2); // Should delete 2 oldest files

        // Verify only 3 newest files remain
        let files_after: Vec<_> = std::fs::read_dir(&worker_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(files_after.len(), 3);
    }

    #[tokio::test]
    async fn test_cleanup_worker_logs_empty_directory() {
        let (pool, temp_dir) = setup_test_db().await;
        let manager = WorkerManager::new(pool);

        // Create an empty worker log directory
        let worker_dir = temp_dir.path().join("worker-empty");
        std::fs::create_dir_all(&worker_dir).unwrap();

        // Cleanup should return 0
        let deleted = manager.cleanup_worker_logs(&worker_dir, u64::MAX, 100);
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_cleanup_worker_logs_nonexistent_directory() {
        let (pool, temp_dir) = setup_test_db().await;
        let manager = WorkerManager::new(pool);

        // Try to cleanup a nonexistent directory
        let worker_dir = temp_dir.path().join("nonexistent");
        let deleted = manager.cleanup_worker_logs(&worker_dir, u64::MAX, 100);
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn test_cleanup_worker_logs_ignores_non_log_files() {
        let (pool, temp_dir) = setup_test_db().await;
        let manager = WorkerManager::new(pool);

        // Create a fake worker log directory
        let worker_dir = temp_dir.path().join("worker-mixed");
        std::fs::create_dir_all(&worker_dir).unwrap();

        // Create some log files and some non-log files
        std::fs::write(worker_dir.join("run-1.log"), "log1").unwrap();
        std::fs::write(worker_dir.join("run-2.log"), "log2").unwrap();
        std::fs::write(worker_dir.join("config.json"), "{}").unwrap();
        std::fs::write(worker_dir.join("data.txt"), "data").unwrap();

        // Cleanup with max 1 log file
        let deleted = manager.cleanup_worker_logs(&worker_dir, u64::MAX, 1);
        assert_eq!(deleted, 1); // Should delete 1 oldest log file

        // Non-log files should still exist
        assert!(worker_dir.join("config.json").exists());
        assert!(worker_dir.join("data.txt").exists());

        // One log file should remain
        let log_count = std::fs::read_dir(&worker_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "log"))
            .count();
        assert_eq!(log_count, 1);
    }
}
