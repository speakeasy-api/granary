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
use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::watch;

use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::Event;
use crate::models::run::{CreateRun, RunStatus, ScheduleRetry, UpdateRunStatus};
use crate::models::{OnError, StepConfig, UpdateWorkerStatus, Worker, WorkerStatus};
use crate::services::event_poller::{EventPoller, EventPollerConfig, create_poller_for_worker};

use crate::services::global_config;
use crate::services::runner::{RunnerHandle, spawn_runner_piped, spawn_runner_with_env};
use crate::services::template::{self, PipelineContext, StepOutput};

/// Wraps either a simple runner process or a spawned pipeline task,
/// providing a uniform interface for concurrency tracking.
enum ActiveRun {
    /// A single spawned child process (simple action).
    Simple(RunnerHandle),
    /// A pipeline executing as a spawned tokio task.
    /// The receiver yields `(exit_code, Option<error_message>)` when done.
    /// The `cancel_tx` signals the pipeline to kill the in-flight child
    /// process before the tokio task is aborted.
    Pipeline {
        result_rx: tokio::sync::oneshot::Receiver<(i32, Option<String>)>,
        join_handle: tokio::task::JoinHandle<()>,
        cancel_tx: watch::Sender<bool>,
    },
}

impl ActiveRun {
    /// Check if the run has completed without blocking.
    ///
    /// Returns `Some((exit_code, error_message))` if done, `None` if still running.
    fn try_wait(&mut self) -> Result<Option<(i32, Option<String>)>> {
        match self {
            ActiveRun::Simple(handle) => handle.try_wait(),
            ActiveRun::Pipeline { result_rx, .. } => match result_rx.try_recv() {
                Ok(result) => Ok(Some(result)),
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => Ok(None),
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => Ok(Some((
                    1,
                    Some("Pipeline task terminated unexpectedly".to_string()),
                ))),
            },
        }
    }

    /// Kill the active run.
    ///
    /// For pipelines, signals cancellation so the in-flight child process
    /// is killed before aborting the tokio task.
    async fn kill(&mut self) -> Result<()> {
        match self {
            ActiveRun::Simple(handle) => handle.kill().await,
            ActiveRun::Pipeline {
                join_handle,
                cancel_tx,
                ..
            } => {
                // Signal the pipeline task to kill the current child process.
                let _ = cancel_tx.send(true);
                // Give the task a brief moment to kill the child, then abort.
                tokio::time::sleep(Duration::from_millis(50)).await;
                join_handle.abort();
                Ok(())
            }
        }
    }
}

/// Default base delay for exponential backoff (in seconds)
const DEFAULT_BASE_DELAY_SECS: u64 = 5;

/// Default maximum retry attempts
const DEFAULT_MAX_ATTEMPTS: i32 = 3;

/// Default poll interval (in milliseconds)
const DEFAULT_POLL_INTERVAL_MS: u64 = 1000;

/// A fully resolved pipeline step, ready for execution.
///
/// Created by [`resolve_pipeline_steps`] which merges action references
/// with step-level overrides.
#[derive(Debug, Clone)]
struct ResolvedStep {
    /// Step name for logging and output references.
    name: String,
    /// Command to execute.
    command: String,
    /// Arguments to pass to the command.
    args: Vec<String>,
    /// Environment variables for this step.
    env: Vec<(String, String)>,
    /// Optional working directory override (may contain templates).
    cwd: Option<String>,
    /// Error handling strategy.
    on_error: OnError,
}

/// Resolve pipeline steps into fully-resolved steps ready for execution.
///
/// For each step:
/// - If `action` is set, loads the action config and merges step overrides
/// - If `command` is set, uses the inline command directly
/// - Pipeline-level env vars are inherited by all steps (step wins on conflict)
fn resolve_pipeline_steps(
    steps: &[StepConfig],
    pipeline_env: &HashMap<String, String>,
) -> Result<Vec<ResolvedStep>> {
    let mut resolved = Vec::with_capacity(steps.len());

    for (i, step) in steps.iter().enumerate() {
        let name = step.resolved_name(i);

        let (command, base_args, base_env) = if let Some(action_name) = &step.action {
            // Load the referenced action
            let action = global_config::get_action(action_name)?.ok_or_else(|| {
                GranaryError::Conflict(format!(
                    "Step '{}' references unknown action '{}'",
                    name, action_name
                ))
            })?;

            let cmd = step
                .command
                .clone()
                .or(action.command.clone())
                .ok_or_else(|| {
                    GranaryError::Conflict(format!(
                        "Step '{}' references action '{}' which has no command defined (it may be a pipeline action)",
                        name, action_name
                    ))
                })?;
            let args = action.args.clone();
            let env = action.env.clone();
            (cmd, args, env)
        } else if let Some(cmd) = &step.command {
            (cmd.clone(), Vec::new(), HashMap::new())
        } else {
            return Err(GranaryError::Conflict(format!(
                "Step '{}' has neither 'action' nor 'command' set",
                name
            )));
        };

        // Step args override action args; step command overrides action command
        let final_command = step.command.clone().unwrap_or(command);
        let final_args = step.args.clone().unwrap_or(base_args);

        // Merge env: pipeline_env < action_env < step_env
        let mut merged_env = pipeline_env.clone();
        merged_env.extend(base_env);
        if let Some(step_env) = &step.env {
            merged_env.extend(step_env.clone());
        }

        let env_vec: Vec<(String, String)> = merged_env.into_iter().collect();

        resolved.push(ResolvedStep {
            name,
            command: final_command,
            args: final_args,
            env: env_vec,
            cwd: step.cwd.clone(),
            on_error: step.on_error.clone().unwrap_or_default(),
        });
    }

    Ok(resolved)
}

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
    /// Resolved ISO timestamp to start processing events from (ephemeral)
    pub since: Option<String>,
}

impl Default for WorkerRuntimeConfig {
    fn default() -> Self {
        Self {
            base_delay_secs: DEFAULT_BASE_DELAY_SECS,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            log_dir: None,
            since: None,
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
    /// Currently active runs (simple processes or pipeline tasks).
    active_runs: HashMap<String, ActiveRun>,
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

        let poller = create_poller_for_worker(
            &worker,
            workspace_pool.clone(),
            poller_config,
            config.since.clone(),
        )
        .await?;

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
        // Only claim as many events as we have available concurrency slots.
        // Events are claimed atomically during poll(), so claiming more than
        // we can process would permanently consume them without handling them.
        let available = self.worker.concurrency as usize - self.active_runs.len();
        if available == 0 {
            return Ok(());
        }

        let events = self.poller.poll(Some(available)).await?;

        for event in events {
            if let Err(e) = self.handle_event(event).await {
                eprintln!("[worker:{}] Error handling event: {}", self.worker.id, e);
            }
        }

        Ok(())
    }

    /// Handle a single event by creating and spawning a run.
    ///
    /// Detects whether the worker is a pipeline and dispatches accordingly:
    /// - Pipeline workers call [`execute_pipeline`] which runs steps serially
    /// - Simple workers use the existing single-process code path
    async fn handle_event(&mut self, event: Event) -> Result<()> {
        // Check concurrency limit
        if self.active_runs.len() >= self.worker.concurrency as usize {
            // Don't acknowledge the event - it will be picked up on next poll
            return Ok(());
        }

        if self.worker.is_pipeline() {
            self.handle_pipeline_event(event).await
        } else {
            self.handle_simple_event(event).await
        }
    }

    /// Handle an event for a simple (non-pipeline) worker.
    ///
    /// This is the original code path: substitute templates, spawn a single
    /// process, and track it for async completion.
    async fn handle_simple_event(&mut self, event: Event) -> Result<()> {
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

        // Associate the run with the originating task (if applicable)
        self.associate_run_with_task(&event, &run.id).await;

        // Update log path with actual run ID
        let log_path = self.log_dir.join(format!("{}.log", run.id));
        sqlx::query("UPDATE runs SET log_path = ? WHERE id = ?")
            .bind(log_path.to_string_lossy().to_string())
            .bind(&run.id)
            .execute(&self.global_pool)
            .await?;

        // Spawn the runner in the workspace directory with env vars
        let workspace_path = std::path::Path::new(&self.worker.instance_path);
        let mut env_vars = self.worker.env_vec();
        env_vars.push(("GRANARY_WORKER_ID".to_string(), self.worker.id.clone()));
        env_vars.push(("GRANARY_RUN_ID".to_string(), run.id.clone()));
        let handle = spawn_runner_with_env(&run, &self.log_dir, workspace_path, &env_vars).await?;

        // Update run status to running with PID
        let update = UpdateRunStatus {
            status: RunStatus::Running,
            exit_code: None,
            error_message: None,
            pid: Some(handle.pid() as i64),
        };
        db::runs::update_status(&self.global_pool, &run.id, &update).await?;

        // Track the active run
        self.active_runs
            .insert(run.id.clone(), ActiveRun::Simple(handle));

        // Acknowledge the event
        self.poller.acknowledge(event.id).await?;

        eprintln!(
            "[worker:{}] Started run {} for event {} ({})",
            self.worker.id, run.id, event.id, event.event_type
        );

        Ok(())
    }

    /// Handle an event for a pipeline worker.
    ///
    /// Creates a run record, spawns the pipeline as a background tokio task,
    /// and inserts it into `active_runs` so it counts toward the concurrency
    /// limit. The spawned task updates the run status on completion.
    async fn handle_pipeline_event(&mut self, event: Event) -> Result<()> {
        let steps = self.worker.pipeline_steps_vec().ok_or_else(|| {
            GranaryError::Conflict(format!(
                "Worker {} is marked as pipeline but has no pipeline_steps",
                self.worker.id
            ))
        })?;

        // Resolve steps (load referenced actions, merge overrides)
        let pipeline_env: HashMap<String, String> = self.worker.env_vec().into_iter().collect();
        let resolved_steps = resolve_pipeline_steps(&steps, &pipeline_env)?;

        // Create run record
        let create_run = CreateRun {
            worker_id: self.worker.id.clone(),
            event_id: event.id,
            event_type: event.event_type.clone(),
            entity_id: event.entity_id.clone(),
            command: format!("pipeline({} steps)", resolved_steps.len()),
            args: resolved_steps.iter().map(|s| s.name.clone()).collect(),
            max_attempts: self.config.max_attempts,
            log_path: Some(
                self.log_dir
                    .join("run-placeholder.log")
                    .to_string_lossy()
                    .to_string(),
            ),
        };

        let run = db::runs::create(&self.global_pool, &create_run).await?;

        // Associate the run with the originating task (if applicable)
        self.associate_run_with_task(&event, &run.id).await;

        // Update log path with actual run ID
        let log_path = self.log_dir.join(format!("{}.log", run.id));
        sqlx::query("UPDATE runs SET log_path = ? WHERE id = ?")
            .bind(log_path.to_string_lossy().to_string())
            .bind(&run.id)
            .execute(&self.global_pool)
            .await?;

        // Mark run as running
        let update = UpdateRunStatus {
            status: RunStatus::Running,
            exit_code: None,
            error_message: None,
            pid: Some(std::process::id() as i64),
        };
        db::runs::update_status(&self.global_pool, &run.id, &update).await?;

        // Acknowledge the event before executing (pipeline may take a while)
        self.poller.acknowledge(event.id).await?;

        eprintln!(
            "[worker:{}] Started pipeline run {} for event {} ({}) with {} steps",
            self.worker.id,
            run.id,
            event.id,
            event.event_type,
            resolved_steps.len()
        );

        // Spawn the pipeline as a background task so it doesn't block the
        // event loop. This allows concurrent pipeline runs up to the
        // worker's concurrency limit.
        //
        // The task returns (exit_code, error_message) so that
        // check_completed_runs → handle_run_completion can update
        // the DB status and handle retries uniformly.
        let worker_id = self.worker.id.clone();
        let run_id = run.id.clone();
        let workspace_path = PathBuf::from(&self.worker.instance_path);
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);

        let join_handle = tokio::spawn(async move {
            let result = execute_pipeline(
                &worker_id,
                &run_id,
                &workspace_path,
                &event,
                &log_path,
                &resolved_steps,
                cancel_rx,
            )
            .await;

            let outcome = match result {
                Ok(()) => (0i32, None),
                Err(e) => (1i32, Some(e.to_string())),
            };
            let _ = result_tx.send(outcome);
        });

        // Track the pipeline task as an active run so it counts toward
        // the concurrency limit.
        self.active_runs.insert(
            run.id.clone(),
            ActiveRun::Pipeline {
                result_rx,
                join_handle,
                cancel_tx,
            },
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

            if self.worker.is_pipeline() {
                self.retry_pipeline_run(&run).await?;
            } else {
                self.retry_simple_run(&run).await?;
            }
        }

        Ok(())
    }

    /// Retry a simple (non-pipeline) run by re-spawning the stored command.
    async fn retry_simple_run(&mut self, run: &crate::models::run::Run) -> Result<()> {
        let workspace_path = std::path::Path::new(&self.worker.instance_path);
        let env_vars = self.worker.env_vec();
        let handle = spawn_runner_with_env(run, &self.log_dir, workspace_path, &env_vars).await?;

        let update = UpdateRunStatus {
            status: RunStatus::Running,
            exit_code: None,
            error_message: None,
            pid: Some(handle.pid() as i64),
        };
        db::runs::update_status(&self.global_pool, &run.id, &update).await?;

        self.active_runs
            .insert(run.id.clone(), ActiveRun::Simple(handle));
        Ok(())
    }

    /// Retry a pipeline run by re-resolving steps and re-executing the pipeline.
    ///
    /// Pipeline runs store a placeholder command (`pipeline(N steps)`) which
    /// cannot be executed directly. Instead, we fetch the original event from
    /// the workspace DB, re-resolve the pipeline steps, and spawn the pipeline
    /// executor — the same path used by [`handle_pipeline_event`].
    async fn retry_pipeline_run(&mut self, run: &crate::models::run::Run) -> Result<()> {
        // Fetch the original event from the workspace DB for template expansion
        let event = db::events::get_by_id(&self.workspace_pool, run.event_id)
            .await?
            .ok_or_else(|| {
                GranaryError::Conflict(format!(
                    "Cannot retry pipeline run {}: original event {} not found in workspace DB",
                    run.id, run.event_id
                ))
            })?;

        // Re-resolve pipeline steps (load referenced actions, merge overrides)
        let steps = self.worker.pipeline_steps_vec().ok_or_else(|| {
            GranaryError::Conflict(format!(
                "Worker {} is marked as pipeline but has no pipeline_steps",
                self.worker.id
            ))
        })?;
        let pipeline_env: HashMap<String, String> = self.worker.env_vec().into_iter().collect();
        let resolved_steps = resolve_pipeline_steps(&steps, &pipeline_env)?;

        // Use the existing log path for this run (appends retry output)
        let log_path = self.log_dir.join(format!("{}.log", run.id));

        // Mark run as running
        let update = UpdateRunStatus {
            status: RunStatus::Running,
            exit_code: None,
            error_message: None,
            pid: Some(std::process::id() as i64),
        };
        db::runs::update_status(&self.global_pool, &run.id, &update).await?;

        // Spawn the pipeline as a background task (same as handle_pipeline_event)
        let worker_id = self.worker.id.clone();
        let run_id = run.id.clone();
        let workspace_path = PathBuf::from(&self.worker.instance_path);
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);

        let join_handle = tokio::spawn(async move {
            let result = execute_pipeline(
                &worker_id,
                &run_id,
                &workspace_path,
                &event,
                &log_path,
                &resolved_steps,
                cancel_rx,
            )
            .await;

            let outcome = match result {
                Ok(()) => (0i32, None),
                Err(e) => (1i32, Some(e.to_string())),
            };
            let _ = result_tx.send(outcome);
        });

        self.active_runs.insert(
            run.id.clone(),
            ActiveRun::Pipeline {
                result_rx,
                join_handle,
                cancel_tx,
            },
        );

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

    /// Associate a run with its originating task by updating the task's
    /// `worker_ids`, `run_ids`, and `owner` fields.
    ///
    /// - `worker_ids`: appended with this worker's ID (deduplicated)
    /// - `run_ids`: always appended with the new run ID
    /// - `owner`: set to the run ID only when currently unset
    ///
    /// Only applies when the event's `entity_type` is "task". Errors are
    /// logged but do not fail the run.
    async fn associate_run_with_task(&self, event: &Event, run_id: &str) {
        if event.entity_type != "task" {
            return;
        }

        let task_id = &event.entity_id;

        let result: Result<()> = async {
            // Fetch the task from the workspace DB
            let mut task = db::tasks::get(&self.workspace_pool, task_id)
                .await?
                .ok_or_else(|| {
                    GranaryError::Conflict(format!("Task {} not found in workspace DB", task_id))
                })?;

            // Append worker_id (deduplicated)
            let mut worker_ids = task.worker_ids_vec();
            if !worker_ids.contains(&self.worker.id) {
                worker_ids.push(self.worker.id.clone());
            }
            task.worker_ids = Some(serde_json::to_string(&worker_ids)?);

            // Append run_id
            let mut run_ids = task.run_ids_vec();
            run_ids.push(run_id.to_string());
            task.run_ids = Some(serde_json::to_string(&run_ids)?);

            // Set owner to run_id only when unset
            if task.owner.as_ref().is_none_or(|o| o.is_empty()) {
                task.owner = Some(run_id.to_string());
            }

            let updated = db::tasks::update(&self.workspace_pool, &task).await?;
            if !updated {
                eprintln!(
                    "[worker:{}] Version conflict: run {} association with task {} was not persisted (task was concurrently modified)",
                    self.worker.id, run_id, task_id
                );
            }

            Ok(())
        }
        .await;

        if let Err(e) = result {
            eprintln!(
                "[worker:{}] Failed to associate run {} with task {}: {}",
                self.worker.id, run_id, task_id, e
            );
        }
    }
}

/// Execute a pipeline: run resolved steps serially with output passing.
///
/// This is a free function (not a method) so it can be spawned as a
/// tokio task for concurrent pipeline execution.
///
/// Each step's stdout is captured and stored in a [`PipelineContext`],
/// making it available to subsequent steps via `{steps.<name>.stdout}`
/// and `{prev.stdout}` template variables.
///
/// The `cancel_rx` channel allows the caller to request cancellation.
/// When signalled, the in-flight child process is killed and the
/// pipeline returns a `Cancelled` error.
async fn execute_pipeline(
    worker_id: &str,
    run_id: &str,
    workspace_path: &Path,
    event: &Event,
    log_path: &Path,
    steps: &[ResolvedStep],
    mut cancel_rx: watch::Receiver<bool>,
) -> Result<()> {
    let mut pipeline_ctx = PipelineContext::new();

    // Ensure log directory exists
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    for (i, step) in steps.iter().enumerate() {
        // 1. Expand templates in args and cwd using event + pipeline context
        let resolved_args =
            template::substitute_all_with_context(&step.args, event, Some(&pipeline_ctx))?;

        let resolved_cwd = if let Some(cwd_template) = &step.cwd {
            let resolved =
                template::substitute_with_context(cwd_template, event, Some(&pipeline_ctx))?;
            PathBuf::from(resolved)
        } else {
            workspace_path.to_path_buf()
        };

        // 2. Write step start delimiter to log
        let step_header = if step.cwd.is_some() {
            format!(
                "=== [step:{}] started cwd={} ===\n",
                step.name,
                resolved_cwd.display()
            )
        } else {
            format!("=== [step:{}] started ===\n", step.name)
        };
        append_to_log(log_path, &step_header)?;

        eprintln!(
            "[worker:{}] Pipeline step {}/{}: {} ({})",
            worker_id,
            i + 1,
            steps.len(),
            step.name,
            step.command
        );

        // 3. Resolve templates in env var values
        let resolved_env: Vec<(String, String)> = step
            .env
            .iter()
            .map(|(k, v)| {
                let resolved_v = template::substitute_with_context(v, event, Some(&pipeline_ctx))?;
                Ok((k.clone(), resolved_v))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut resolved_env = resolved_env;
        resolved_env.push(("GRANARY_WORKER_ID".to_string(), worker_id.to_string()));
        resolved_env.push(("GRANARY_RUN_ID".to_string(), run_id.to_string()));

        // 4. Spawn piped process
        // Use a per-step stderr log so spawn_runner_piped can direct stderr there
        let step_stderr_path = log_path.with_extension(format!("step-{}.stderr", i));
        let handle = spawn_runner_piped(
            &step.command,
            &resolved_args,
            &resolved_cwd,
            &resolved_env,
            &step_stderr_path,
        )
        .await?;

        // 5. Wait for completion (or cancellation)
        let result = handle.wait_or_cancel(&mut cancel_rx).await?;

        // 6. Write stdout to main log file (prefixed with step name)
        if !result.stdout.is_empty() {
            append_to_log(log_path, &result.stdout)?;
            append_to_log(log_path, "\n")?;
        }

        // Append stderr from the step's stderr file into the main log
        if step_stderr_path.exists() {
            if let Ok(stderr_content) = std::fs::read_to_string(&step_stderr_path)
                && !stderr_content.is_empty()
            {
                append_to_log(log_path, &stderr_content)?;
                if !stderr_content.ends_with('\n') {
                    append_to_log(log_path, "\n")?;
                }
            }
            // Clean up per-step stderr file
            let _ = std::fs::remove_file(&step_stderr_path);
        }

        // Write exit code delimiter
        append_to_log(
            log_path,
            &format!(
                "=== [step:{}] exit_code={} ===\n\n",
                step.name, result.exit_code
            ),
        )?;

        // 7. Store in pipeline context
        pipeline_ctx.add_step(
            step.name.clone(),
            StepOutput {
                stdout: result.stdout.clone(),
                exit_code: result.exit_code,
            },
        );

        // 8. Check exit code vs on_error
        if result.exit_code != 0 {
            match step.on_error {
                OnError::Stop => {
                    return Err(GranaryError::Conflict(format!(
                        "Pipeline step '{}' failed with exit code {} (on_error=stop)",
                        step.name, result.exit_code
                    )));
                }
                OnError::Continue => {
                    eprintln!(
                        "[worker:{}] Pipeline step '{}' failed with exit code {} (on_error=continue, continuing)",
                        worker_id, step.name, result.exit_code
                    );
                }
            }
        }
    }

    Ok(())
}

/// Append text to a log file (creating it if necessary).
fn append_to_log(path: &Path, content: &str) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
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

    // ==========================================
    // Pipeline Integration Tests
    // ==========================================

    fn create_test_event(payload: &str) -> Event {
        Event {
            id: 1,
            event_type: "task.next".to_string(),
            entity_type: "task".to_string(),
            entity_id: "test-task-1".to_string(),
            actor: Some("test".to_string()),
            session_id: None,
            payload: payload.to_string(),
            created_at: "2026-02-15T10:00:00Z".to_string(),
        }
    }

    /// Multi-step pipeline end-to-end: step 1 outputs a value, step 2 consumes it
    /// via `{prev.stdout}`, step 3 consumes step 1's output via `{steps.step1.stdout}`.
    #[tokio::test]
    async fn test_pipeline_multi_step_output_passing() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("test.log");
        let event = create_test_event(r#"{"id": "task-42"}"#);

        let steps = vec![
            ResolvedStep {
                name: "step1".to_string(),
                command: "echo".to_string(),
                args: vec!["hello-from-step1".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "step2".to_string(),
                command: "echo".to_string(),
                args: vec!["got:{prev.stdout}".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "step3".to_string(),
                command: "echo".to_string(),
                args: vec!["from-step1:{steps.step1.stdout}".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(result.is_ok(), "Pipeline should succeed: {:?}", result);

        let log = std::fs::read_to_string(&log_path).unwrap();

        // Step 1 output captured
        assert!(
            log.contains("hello-from-step1"),
            "Log should contain step1 output"
        );
        // Step 2 received prev.stdout
        assert!(
            log.contains("got:hello-from-step1"),
            "Step2 should have received step1 output via prev.stdout"
        );
        // Step 3 received step1 output by name
        assert!(
            log.contains("from-step1:hello-from-step1"),
            "Step3 should have received step1 output via steps.step1.stdout"
        );
        // Log delimiters present
        assert!(log.contains("=== [step:step1] started ==="));
        assert!(log.contains("=== [step:step1] exit_code=0 ==="));
        assert!(log.contains("=== [step:step2] started ==="));
        assert!(log.contains("=== [step:step3] exit_code=0 ==="));
    }

    /// Template resolution in env vars: step env values should resolve pipeline templates.
    #[tokio::test]
    async fn test_pipeline_template_resolution_in_env_vars() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("env-test.log");
        let event = create_test_event(r#"{"id": "proj-abc-task-5"}"#);

        let steps = vec![
            ResolvedStep {
                name: "produce".to_string(),
                command: "echo".to_string(),
                args: vec!["/tmp/worktree-path".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "consume".to_string(),
                command: "sh".to_string(),
                args: vec![
                    "-c".to_string(),
                    "echo \"WORKDIR=$STEP_OUTPUT TASK=$TASK_ID\"".to_string(),
                ],
                env: vec![
                    (
                        "STEP_OUTPUT".to_string(),
                        "{steps.produce.stdout}".to_string(),
                    ),
                    ("TASK_ID".to_string(), "{id}".to_string()),
                ],
                cwd: None,
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(result.is_ok(), "Pipeline should succeed: {:?}", result);

        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains("WORKDIR=/tmp/worktree-path TASK=proj-abc-task-5"),
            "Env vars should have templates resolved. Log:\n{}",
            log
        );
    }

    /// Step error handling: on_error=stop causes early exit on first failure.
    #[tokio::test]
    async fn test_pipeline_error_handling_stop() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("error-stop.log");
        let event = create_test_event(r#"{}"#);

        let steps = vec![
            ResolvedStep {
                name: "ok-step".to_string(),
                command: "echo".to_string(),
                args: vec!["step1-ok".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "fail-step".to_string(),
                command: "sh".to_string(),
                args: vec!["-c".to_string(), "exit 1".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "never-reached".to_string(),
                command: "echo".to_string(),
                args: vec!["should-not-run".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(result.is_err(), "Pipeline should fail on non-zero exit");

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("fail-step"),
            "Error should mention the failing step"
        );
        assert!(
            err_msg.contains("exit code 1"),
            "Error should mention exit code"
        );

        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(log.contains("step1-ok"), "First step should have run");
        assert!(
            !log.contains("should-not-run"),
            "Third step should NOT have run after failure with on_error=stop"
        );
    }

    /// Step error handling: on_error=continue allows pipeline to proceed past failures.
    #[tokio::test]
    async fn test_pipeline_error_handling_continue() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("error-continue.log");
        let event = create_test_event(r#"{}"#);

        let steps = vec![
            ResolvedStep {
                name: "fail-step".to_string(),
                command: "sh".to_string(),
                args: vec![
                    "-c".to_string(),
                    "echo partial-output && exit 42".to_string(),
                ],
                env: vec![],
                cwd: None,
                on_error: OnError::Continue,
            },
            ResolvedStep {
                name: "after-fail".to_string(),
                command: "echo".to_string(),
                args: vec![
                    "prev_exit={prev.exit_code}".to_string(),
                    "prev_out={prev.stdout}".to_string(),
                ],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(
            result.is_ok(),
            "Pipeline should succeed despite failed step with on_error=continue: {:?}",
            result
        );

        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains("=== [step:fail-step] exit_code=42 ==="),
            "Failed step exit code should be logged"
        );
        assert!(
            log.contains("prev_exit=42"),
            "Subsequent step should see failed step's exit code via prev.exit_code"
        );
        assert!(
            log.contains("prev_out=partial-output"),
            "Subsequent step should see failed step's stdout via prev.stdout"
        );
    }

    /// Concurrency behavior: steps run serially, not concurrently. Output from
    /// step N is available to step N+1 because they execute sequentially.
    #[tokio::test]
    async fn test_pipeline_steps_run_serially() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("serial.log");
        let marker_file = tmp.path().join("marker.txt");
        let event = create_test_event(r#"{}"#);

        // Step 1: write a marker file
        // Step 2: read the marker file (only works if step 1 completed first)
        let steps = vec![
            ResolvedStep {
                name: "writer".to_string(),
                command: "sh".to_string(),
                args: vec![
                    "-c".to_string(),
                    format!("echo serial-proof > {}", marker_file.display()),
                ],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "reader".to_string(),
                command: "sh".to_string(),
                args: vec!["-c".to_string(), format!("cat {}", marker_file.display())],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(
            result.is_ok(),
            "Pipeline should succeed (proves serial execution): {:?}",
            result
        );

        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains("serial-proof"),
            "Reader step should have read the file written by writer step"
        );
    }

    /// Pipeline with cwd override: step cwd is resolved from template.
    #[tokio::test]
    async fn test_pipeline_cwd_template_resolution() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("cwd-test.log");
        let subdir = tmp.path().join("my-subdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let event = create_test_event(r#"{}"#);

        let steps = vec![
            ResolvedStep {
                name: "emit-path".to_string(),
                command: "echo".to_string(),
                args: vec![subdir.to_string_lossy().to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "use-path".to_string(),
                command: "pwd".to_string(),
                args: vec![],
                env: vec![],
                cwd: Some("{prev.stdout}".to_string()),
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(result.is_ok(), "Pipeline should succeed: {:?}", result);

        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains(&subdir.to_string_lossy().to_string()),
            "Step with cwd template should have run in the correct directory. Log:\n{}",
            log
        );
    }

    /// Namespaced step names (containing `/`) work correctly for output references.
    #[tokio::test]
    async fn test_pipeline_namespaced_step_names() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("namespaced.log");
        let event = create_test_event(r#"{}"#);

        let steps = vec![
            ResolvedStep {
                name: "git/worktree-create".to_string(),
                command: "echo".to_string(),
                args: vec!["/tmp/worktree-abc".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "agents/claude-work".to_string(),
                command: "echo".to_string(),
                args: vec!["working-in:{steps.git/worktree-create.stdout}".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(result.is_ok(), "Pipeline should succeed: {:?}", result);

        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains("working-in:/tmp/worktree-abc"),
            "Namespaced step name should resolve correctly. Log:\n{}",
            log
        );
    }

    /// Event payload templates resolve alongside pipeline templates.
    #[tokio::test]
    async fn test_pipeline_event_and_pipeline_templates_mixed() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("mixed.log");
        let event = create_test_event(r#"{"id": "task-99", "title": "Fix bug"}"#);

        let steps = vec![
            ResolvedStep {
                name: "setup".to_string(),
                command: "echo".to_string(),
                args: vec!["setup-done".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "work".to_string(),
                command: "echo".to_string(),
                args: vec![
                    "task={id}".to_string(),
                    "title={title}".to_string(),
                    "event={event.type}".to_string(),
                    "prev={prev.stdout}".to_string(),
                ],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(result.is_ok(), "Pipeline should succeed: {:?}", result);

        let log = std::fs::read_to_string(&log_path).unwrap();
        assert!(
            log.contains("task=task-99"),
            "Event payload {{id}} should resolve"
        );
        assert!(
            log.contains("title=Fix bug"),
            "Event payload {{title}} should resolve"
        );
        assert!(
            log.contains("event=task.next"),
            "Event meta {{event.type}} should resolve"
        );
        assert!(
            log.contains("prev=setup-done"),
            "Pipeline template {{prev.stdout}} should resolve"
        );
    }

    /// Empty pipeline (zero steps) succeeds immediately.
    #[tokio::test]
    async fn test_pipeline_empty_steps() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("empty.log");
        let event = create_test_event(r#"{}"#);

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &[],
            watch::channel(false).1,
        )
        .await;
        assert!(result.is_ok(), "Empty pipeline should succeed");
    }

    /// resolve_pipeline_steps: inline command steps resolve correctly.
    #[test]
    fn test_resolve_pipeline_steps_inline_command() {
        let steps = vec![
            StepConfig {
                name: Some("greet".to_string()),
                action: None,
                command: Some("echo".to_string()),
                args: Some(vec!["hello".to_string()]),
                env: None,
                cwd: None,
                on_error: None,
            },
            StepConfig {
                name: None,
                action: None,
                command: Some("ls".to_string()),
                args: None,
                env: Some(HashMap::from([("FOO".to_string(), "bar".to_string())])),
                cwd: Some("/tmp".to_string()),
                on_error: Some(OnError::Continue),
            },
        ];

        let pipeline_env = HashMap::from([("GLOBAL".to_string(), "val".to_string())]);
        let resolved = resolve_pipeline_steps(&steps, &pipeline_env).unwrap();

        assert_eq!(resolved.len(), 2);

        // First step: explicit name, explicit args
        assert_eq!(resolved[0].name, "greet");
        assert_eq!(resolved[0].command, "echo");
        assert_eq!(resolved[0].args, vec!["hello"]);
        // Pipeline env inherited
        assert!(
            resolved[0]
                .env
                .iter()
                .any(|(k, v)| k == "GLOBAL" && v == "val")
        );

        // Second step: auto-generated name (step_2), no explicit args → empty
        assert_eq!(resolved[1].name, "step_2");
        assert_eq!(resolved[1].command, "ls");
        assert!(resolved[1].args.is_empty());
        assert_eq!(resolved[1].cwd, Some("/tmp".to_string()));
        // Step env overrides pipeline env
        assert!(
            resolved[1]
                .env
                .iter()
                .any(|(k, v)| k == "FOO" && v == "bar")
        );
        // Pipeline env also present
        assert!(
            resolved[1]
                .env
                .iter()
                .any(|(k, v)| k == "GLOBAL" && v == "val")
        );
        // on_error propagated
        assert!(matches!(resolved[1].on_error, OnError::Continue));
    }

    /// resolve_pipeline_steps: step without action or command is an error.
    #[test]
    fn test_resolve_pipeline_steps_missing_command_and_action() {
        let steps = vec![StepConfig {
            name: Some("bad".to_string()),
            action: None,
            command: None,
            args: None,
            env: None,
            cwd: None,
            on_error: None,
        }];

        let result = resolve_pipeline_steps(&steps, &HashMap::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("bad"));
    }

    /// resolve_pipeline_steps: step referencing a non-existent action produces an error.
    #[test]
    fn test_resolve_pipeline_steps_action_reference_unknown() {
        let steps = vec![StepConfig {
            name: None,
            action: Some("nonexistent/action".to_string()),
            command: None,
            args: None,
            env: None,
            cwd: None,
            on_error: None,
        }];

        let result = resolve_pipeline_steps(&steps, &HashMap::new());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("nonexistent/action"),
            "Error should mention the missing action: {}",
            err
        );
    }

    /// Log file contains proper step delimiters for all steps.
    #[tokio::test]
    async fn test_pipeline_log_delimiters() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("delimiters.log");
        let event = create_test_event(r#"{}"#);

        let steps = vec![
            ResolvedStep {
                name: "alpha".to_string(),
                command: "echo".to_string(),
                args: vec!["a-output".to_string()],
                env: vec![],
                cwd: None,
                on_error: OnError::Stop,
            },
            ResolvedStep {
                name: "beta".to_string(),
                command: "echo".to_string(),
                args: vec!["b-output".to_string()],
                env: vec![],
                cwd: Some("/tmp".to_string()),
                on_error: OnError::Stop,
            },
        ];

        let result = execute_pipeline(
            "test-worker",
            "test-run",
            tmp.path(),
            &event,
            &log_path,
            &steps,
            watch::channel(false).1,
        )
        .await;
        assert!(result.is_ok());

        let log = std::fs::read_to_string(&log_path).unwrap();

        // Step without cwd: plain started delimiter
        assert!(log.contains("=== [step:alpha] started ==="));
        assert!(log.contains("=== [step:alpha] exit_code=0 ==="));

        // Step with cwd: started delimiter includes cwd
        assert!(log.contains("=== [step:beta] started cwd=/tmp ==="));
        assert!(log.contains("=== [step:beta] exit_code=0 ==="));
    }

    /// Cancelling a pipeline kills the in-flight child process.
    #[tokio::test]
    async fn test_pipeline_cancellation_kills_child() {
        let tmp = tempfile::tempdir().unwrap();
        let log_path = tmp.path().join("logs").join("cancel.log");
        let event = create_test_event(r#"{}"#);

        // A step that sleeps long enough for us to cancel it
        let steps = vec![ResolvedStep {
            name: "sleeper".to_string(),
            command: "sleep".to_string(),
            args: vec!["60".to_string()],
            env: vec![],
            cwd: None,
            on_error: OnError::Stop,
        }];

        let (cancel_tx, cancel_rx) = watch::channel(false);

        let handle = tokio::spawn({
            let tmp_path = tmp.path().to_path_buf();
            let event = event.clone();
            let log_path = log_path.clone();
            let steps = steps.clone();
            async move {
                execute_pipeline(
                    "test-worker",
                    "test-run",
                    &tmp_path,
                    &event,
                    &log_path,
                    &steps,
                    cancel_rx,
                )
                .await
            }
        });

        // Give the pipeline a moment to spawn the child process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Signal cancellation
        cancel_tx.send(true).unwrap();

        // The pipeline should return a Cancelled error
        let result = handle.await.unwrap();
        assert!(result.is_err(), "Pipeline should fail on cancellation");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("cancelled") || err_msg.contains("Cancelled"),
            "Error should indicate cancellation: {}",
            err_msg
        );
    }
}
