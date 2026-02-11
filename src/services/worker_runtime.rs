//! Worker runtime for event-driven task execution.
//!
//! The worker runtime is the core component that:
//! 1. Polls for events matching the worker's subscription
//! 2. Spawns runner processes to handle events
//! 3. Manages concurrency limits
//! 4. Handles retries with exponential backoff
//! 5. Tracks run status and logs
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     WorkerRuntime                           │
//! │                                                             │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
//! │  │ EventPoller  │───>│ Run Manager  │───>│ Runner Procs │  │
//! │  └──────────────┘    └──────────────┘    └──────────────┘  │
//! │         │                   │                    │         │
//! │         ▼                   ▼                    ▼         │
//! │  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
//! │  │ Workspace DB │    │  Global DB   │    │   Log Files  │  │
//! │  │   (events)   │    │(workers,runs)│    │              │  │
//! │  └──────────────┘    └──────────────┘    └──────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::watch;

use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::Event;
use crate::models::run::{CreateRun, RunStatus, ScheduleRetry, UpdateRunStatus};
use crate::models::{UpdateWorkerStatus, Worker, WorkerStatus};
use crate::services::event_poller::{EventPoller, EventPollerConfig, create_poller_for_worker};

use crate::services::global_config;
use crate::services::runner::{RunnerHandle, spawn_runner};
use crate::services::template;

/// Default base delay for exponential backoff (in seconds)
const DEFAULT_BASE_DELAY_SECS: u64 = 5;

/// Default maximum retry attempts
const DEFAULT_MAX_ATTEMPTS: i32 = 3;

/// Default poll interval (in milliseconds)
const DEFAULT_POLL_INTERVAL_MS: u64 = 1000;

/// Worker runtime configuration
#[derive(Debug, Clone)]
pub struct WorkerRuntimeConfig {
    /// Base delay for exponential backoff retries
    pub base_delay_secs: u64,
    /// Maximum retry attempts for failed runs
    pub max_attempts: i32,
    /// Interval between event polls
    pub poll_interval: Duration,
    /// Directory for log files (defaults to ~/.granary/logs/{worker_id}/)
    pub log_dir: Option<PathBuf>,
}

impl Default for WorkerRuntimeConfig {
    fn default() -> Self {
        Self {
            base_delay_secs: DEFAULT_BASE_DELAY_SECS,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            log_dir: None,
        }
    }
}

/// Worker runtime for polling events and spawning runners.
///
/// The runtime manages the lifecycle of a worker, including:
/// - Event polling and filtering
/// - Runner process spawning
/// - Concurrency control
/// - Retry scheduling
/// - Status tracking
pub struct WorkerRuntime {
    /// The worker configuration
    worker: Worker,
    /// Global database pool (workers and runs)
    global_pool: SqlitePool,
    /// Workspace database pool (events)
    workspace_pool: SqlitePool,
    /// Event poller
    poller: EventPoller,
    /// Currently active runner handles
    active_runs: HashMap<String, RunnerHandle>,
    /// Shutdown signal receiver
    shutdown_rx: watch::Receiver<bool>,
    /// Runtime configuration
    config: WorkerRuntimeConfig,
    /// Log directory path
    log_dir: PathBuf,
}

impl WorkerRuntime {
    /// Create a new worker runtime.
    ///
    /// # Arguments
    /// * `worker` - The worker configuration
    /// * `global_pool` - Connection pool for the global database
    /// * `workspace_pool` - Connection pool for the workspace database
    /// * `shutdown_rx` - Receiver for shutdown signal
    /// * `config` - Runtime configuration
    pub async fn new(
        worker: Worker,
        global_pool: SqlitePool,
        workspace_pool: SqlitePool,
        shutdown_rx: watch::Receiver<bool>,
        config: WorkerRuntimeConfig,
    ) -> Result<Self> {
        // Create event poller
        let poller_config =
            EventPollerConfig::with_poll_interval(config.poll_interval).auto_update_cursor(false); // We manage cursor updates manually

        let poller =
            create_poller_for_worker(&worker, workspace_pool.clone(), poller_config).await?;

        // Determine log directory
        let log_dir = config.log_dir.clone().unwrap_or_else(|| {
            global_config::config_dir()
                .unwrap_or_else(|_| PathBuf::from("~/.granary"))
                .join("logs")
                .join(&worker.id)
        });

        Ok(Self {
            worker,
            global_pool,
            workspace_pool,
            poller,
            active_runs: HashMap::new(),
            shutdown_rx,
            config,
            log_dir,
        })
    }

    /// Run the worker runtime main loop.
    ///
    /// This method will run until:
    /// - A shutdown signal is received
    /// - The workspace is deleted
    /// - An unrecoverable error occurs
    pub async fn run(&mut self) -> Result<()> {
        // Mark worker as running
        self.update_worker_status(WorkerStatus::Running, None)
            .await?;

        // Main event loop
        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() {
                        eprintln!("[worker:{}] Shutdown signal received, stopping worker", self.worker.id);
                        break;
                    }
                }

                // Poll for new events
                _ = tokio::time::sleep(self.config.poll_interval) => {
                    // Check if workspace still exists
                    if !self.workspace_exists().await {
                        self.transition_to_error("Workspace no longer exists").await?;
                        break;
                    }

                    // Process pending retries
                    if let Err(e) = self.process_pending_retries().await {
                        eprintln!("[worker:{}] Error processing retries: {}", self.worker.id, e);
                    }

                    // Check for completed runs
                    self.check_completed_runs().await?;

                    // Poll and handle new events
                    if let Err(e) = self.poll_and_handle_events().await {
                        eprintln!("[worker:{}] Error polling events: {}", self.worker.id, e);
                    }
                }
            }
        }

        // Graceful shutdown: wait for active runs
        self.graceful_shutdown().await?;

        // Mark worker as stopped
        self.update_worker_status(WorkerStatus::Stopped, None)
            .await?;

        Ok(())
    }

    /// Poll for new events and handle them.
    async fn poll_and_handle_events(&mut self) -> Result<()> {
        let events = self.poller.poll().await?;

        for event in events {
            if let Err(e) = self.handle_event(event).await {
                eprintln!("[worker:{}] Error handling event: {}", self.worker.id, e);
            }
        }

        Ok(())
    }

    /// Handle a single event by creating and spawning a run.
    async fn handle_event(&mut self, event: Event) -> Result<()> {
        // Check concurrency limit
        if self.active_runs.len() >= self.worker.concurrency as usize {
            // Don't acknowledge the event - it will be picked up on next poll
            return Ok(());
        }

        // Substitute template variables in args
        let worker_args = self.worker.args_vec();
        let resolved_args = template::substitute_all(&worker_args, &event)?;

        // Create run record
        let create_run = CreateRun {
            worker_id: self.worker.id.clone(),
            event_id: event.id,
            event_type: event.event_type.clone(),
            entity_id: event.entity_id.clone(),
            command: self.worker.command.clone(),
            args: resolved_args.clone(),
            max_attempts: self.config.max_attempts,
            log_path: Some(
                self.log_dir
                    .join("run-placeholder.log")
                    .to_string_lossy()
                    .to_string(),
            ),
        };

        let run = db::runs::create(&self.global_pool, &create_run).await?;

        // Update log path with actual run ID
        let log_path = self.log_dir.join(format!("{}.log", run.id));
        sqlx::query("UPDATE runs SET log_path = ? WHERE id = ?")
            .bind(log_path.to_string_lossy().to_string())
            .bind(&run.id)
            .execute(&self.global_pool)
            .await?;

        // Spawn the runner in the workspace directory
        let workspace_path = std::path::Path::new(&self.worker.instance_path);
        let handle = spawn_runner(&run, &self.log_dir, workspace_path).await?;

        // Update run status to running with PID
        let update = UpdateRunStatus {
            status: RunStatus::Running,
            exit_code: None,
            error_message: None,
            pid: Some(handle.pid() as i64),
        };
        db::runs::update_status(&self.global_pool, &run.id, &update).await?;

        // Track the active run
        self.active_runs.insert(run.id.clone(), handle);

        // Acknowledge the event
        self.poller.acknowledge(event.id).await?;

        eprintln!(
            "[worker:{}] Started run {} for event {} ({})",
            self.worker.id, run.id, event.id, event.event_type
        );

        Ok(())
    }

    /// Check for completed runs and update their status.
    async fn check_completed_runs(&mut self) -> Result<()> {
        let mut completed_runs = Vec::new();

        for (run_id, handle) in self.active_runs.iter_mut() {
            if let Some((exit_code, error)) = handle.try_wait()? {
                completed_runs.push((run_id.clone(), exit_code, error));
            }
        }

        for (run_id, exit_code, error) in completed_runs {
            self.handle_run_completion(&run_id, exit_code, error)
                .await?;
            self.active_runs.remove(&run_id);
        }

        Ok(())
    }

    /// Handle a run completion (success or failure).
    async fn handle_run_completion(
        &self,
        run_id: &str,
        exit_code: i32,
        error: Option<String>,
    ) -> Result<()> {
        let run = db::runs::get(&self.global_pool, run_id)
            .await?
            .ok_or_else(|| GranaryError::Conflict(format!("Run {} not found", run_id)))?;

        if exit_code == 0 {
            // Success
            let update = UpdateRunStatus {
                status: RunStatus::Completed,
                exit_code: Some(exit_code),
                error_message: None,
                pid: None,
            };
            db::runs::update_status(&self.global_pool, run_id, &update).await?;
            eprintln!(
                "[worker:{}] Run {} completed successfully",
                self.worker.id, run_id
            );
        } else {
            // Failure - check if we should retry
            if run.can_retry() {
                let next_attempt = run.attempt + 1;
                let backoff = calculate_backoff(run.attempt, self.config.base_delay_secs);
                let next_retry_at =
                    chrono::Utc::now() + chrono::Duration::from_std(backoff).unwrap();

                let retry = ScheduleRetry {
                    next_retry_at: next_retry_at.to_rfc3339(),
                    attempt: next_attempt,
                };
                db::runs::update_for_retry(&self.global_pool, run_id, &retry).await?;

                eprintln!(
                    "[worker:{}] Run {} failed (attempt {}/{}), scheduled retry at {}",
                    self.worker.id, run_id, run.attempt, run.max_attempts, next_retry_at
                );
            } else {
                // No more retries
                let update = UpdateRunStatus {
                    status: RunStatus::Failed,
                    exit_code: Some(exit_code),
                    error_message: error,
                    pid: None,
                };
                db::runs::update_status(&self.global_pool, run_id, &update).await?;
                eprintln!(
                    "[worker:{}] Run {} failed after {} attempts",
                    self.worker.id, run_id, run.attempt
                );
            }
        }

        Ok(())
    }

    /// Process pending retries that are due.
    async fn process_pending_retries(&mut self) -> Result<()> {
        // Check concurrency limit
        let available_slots = self.worker.concurrency as usize - self.active_runs.len();
        if available_slots == 0 {
            return Ok(());
        }

        let now = chrono::Utc::now().to_rfc3339();
        let pending_retries = db::runs::list_pending_retries(&self.global_pool, &now).await?;

        for run in pending_retries.into_iter().take(available_slots) {
            // Only retry runs for this worker
            if run.worker_id != self.worker.id {
                continue;
            }

            eprintln!(
                "[worker:{}] Retrying run {} (attempt {}/{})",
                self.worker.id, run.id, run.attempt, run.max_attempts
            );

            // Spawn the runner in the workspace directory
            let workspace_path = std::path::Path::new(&self.worker.instance_path);
            let handle = spawn_runner(&run, &self.log_dir, workspace_path).await?;

            // Update run status to running with PID
            let update = UpdateRunStatus {
                status: RunStatus::Running,
                exit_code: None,
                error_message: None,
                pid: Some(handle.pid() as i64),
            };
            db::runs::update_status(&self.global_pool, &run.id, &update).await?;

            // Track the active run
            self.active_runs.insert(run.id.clone(), handle);
        }

        Ok(())
    }

    /// Check if the workspace still exists.
    async fn workspace_exists(&self) -> bool {
        let path = std::path::Path::new(&self.worker.instance_path);
        if !path.exists() {
            return false;
        }

        // Also check if we can still query the workspace database
        let result: std::result::Result<i64, _> = sqlx::query_scalar("SELECT 1")
            .fetch_one(&self.workspace_pool)
            .await;
        result.is_ok()
    }

    /// Transition the worker to error state.
    async fn transition_to_error(&mut self, reason: &str) -> Result<()> {
        eprintln!(
            "[worker:{}] Worker entering error state: {}",
            self.worker.id, reason
        );
        self.update_worker_status(WorkerStatus::Error, Some(reason.to_string()))
            .await
    }

    /// Update the worker status in the database.
    async fn update_worker_status(
        &self,
        status: WorkerStatus,
        error_message: Option<String>,
    ) -> Result<()> {
        let pid = if status == WorkerStatus::Running {
            Some(std::process::id() as i64)
        } else {
            None
        };

        let update = UpdateWorkerStatus {
            status,
            error_message,
            pid,
        };
        db::workers::update_status(&self.global_pool, &self.worker.id, &update).await?;
        Ok(())
    }

    /// Perform graceful shutdown.
    ///
    /// This waits for active runs to complete or kills them after a timeout.
    async fn graceful_shutdown(&mut self) -> Result<()> {
        if self.active_runs.is_empty() {
            return Ok(());
        }

        eprintln!(
            "[worker:{}] Graceful shutdown: waiting for {} active runs",
            self.worker.id,
            self.active_runs.len()
        );

        // Give processes time to finish (30 seconds)
        let shutdown_timeout = Duration::from_secs(30);
        let deadline = tokio::time::Instant::now() + shutdown_timeout;

        loop {
            // Check for completed runs
            self.check_completed_runs().await?;

            if self.active_runs.is_empty() {
                break;
            }

            if tokio::time::Instant::now() >= deadline {
                // Timeout - kill remaining processes
                eprintln!(
                    "[worker:{}] Shutdown timeout: killing {} remaining processes",
                    self.worker.id,
                    self.active_runs.len()
                );

                for (run_id, mut handle) in self.active_runs.drain() {
                    if let Err(e) = handle.kill().await {
                        eprintln!(
                            "[worker:{}] Failed to kill run {}: {}",
                            self.worker.id, run_id, e
                        );
                    }

                    // Mark run as cancelled
                    let update = UpdateRunStatus {
                        status: RunStatus::Cancelled,
                        exit_code: None,
                        error_message: Some("Killed during worker shutdown".to_string()),
                        pid: None,
                    };
                    db::runs::update_status(&self.global_pool, &run_id, &update).await?;
                }
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }

    /// Get the worker ID.
    pub fn worker_id(&self) -> &str {
        &self.worker.id
    }

    /// Get the number of active runs.
    pub fn active_run_count(&self) -> usize {
        self.active_runs.len()
    }

    /// Get the log directory path.
    pub fn log_dir(&self) -> &PathBuf {
        &self.log_dir
    }
}

/// Calculate exponential backoff delay with jitter.
///
/// # Arguments
/// * `attempt` - The current attempt number (1-based)
/// * `base_delay_secs` - The base delay in seconds
///
/// # Returns
/// The delay duration with jitter applied.
pub fn calculate_backoff(attempt: i32, base_delay_secs: u64) -> Duration {
    // Exponential: base * 2^(attempt-1)
    // e.g., with base=5: 5, 10, 20, 40, 80...
    let exp = (attempt - 1).min(10) as u32; // Cap at 2^10 to prevent overflow
    let delay = base_delay_secs.saturating_mul(2u64.pow(exp));

    // Add jitter: 0-25% of the delay
    let jitter_range = delay / 4;
    let jitter = if jitter_range > 0 {
        rand::random::<u64>() % jitter_range
    } else {
        0
    };

    Duration::from_secs(delay + jitter)
}

/// Create a shutdown signal sender/receiver pair.
///
/// The sender can be used to signal shutdown to the worker runtime,
/// and the receiver is passed to the runtime.
pub fn create_shutdown_channel() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    watch::channel(false)
}

/// Start a worker runtime in a background task.
///
/// # Arguments
/// * `worker` - The worker configuration
/// * `global_pool` - Connection pool for the global database
/// * `workspace_pool` - Connection pool for the workspace database
/// * `config` - Runtime configuration
///
/// # Returns
/// A tuple of (JoinHandle, shutdown_sender) where the shutdown sender
/// can be used to stop the worker.
pub async fn start_worker_runtime(
    worker: Worker,
    global_pool: SqlitePool,
    workspace_pool: SqlitePool,
    config: WorkerRuntimeConfig,
) -> Result<(tokio::task::JoinHandle<Result<()>>, watch::Sender<bool>)> {
    let (shutdown_tx, shutdown_rx) = create_shutdown_channel();

    let mut runtime =
        WorkerRuntime::new(worker, global_pool, workspace_pool, shutdown_rx, config).await?;

    let handle = tokio::spawn(async move { runtime.run().await });

    Ok((handle, shutdown_tx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_backoff_first_attempt() {
        let delay = calculate_backoff(1, 5);
        // First attempt: 5 seconds + 0-1.25 seconds jitter
        assert!(delay.as_secs() >= 5 && delay.as_secs() <= 7);
    }

    #[test]
    fn test_calculate_backoff_second_attempt() {
        let delay = calculate_backoff(2, 5);
        // Second attempt: 10 seconds + 0-2.5 seconds jitter
        assert!(delay.as_secs() >= 10 && delay.as_secs() <= 13);
    }

    #[test]
    fn test_calculate_backoff_exponential() {
        // Verify exponential growth (ignoring jitter)
        for attempt in 1..=5 {
            let delay = calculate_backoff(attempt, 5);
            let expected_base = 5 * 2u64.pow((attempt - 1) as u32);
            // Allow for jitter
            assert!(
                delay.as_secs() >= expected_base,
                "Attempt {}: expected >= {}, got {}",
                attempt,
                expected_base,
                delay.as_secs()
            );
        }
    }

    #[test]
    fn test_calculate_backoff_capped() {
        // Very high attempt number should not overflow
        let delay = calculate_backoff(100, 5);
        // Should be capped at 2^10 = 1024 * 5 = 5120 seconds max base
        assert!(delay.as_secs() <= 6400); // 5120 + 25% jitter
    }

    #[test]
    fn test_default_config() {
        let config = WorkerRuntimeConfig::default();
        assert_eq!(config.base_delay_secs, DEFAULT_BASE_DELAY_SECS);
        assert_eq!(config.max_attempts, DEFAULT_MAX_ATTEMPTS);
        assert_eq!(
            config.poll_interval.as_millis(),
            DEFAULT_POLL_INTERVAL_MS as u128
        );
        assert!(config.log_dir.is_none());
    }
}
