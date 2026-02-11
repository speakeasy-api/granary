//! Worker CLI commands for managing worker processes.
//!
//! Workers are long-running processes that subscribe to granary events and spawn
//! runners to execute commands. They are managed by the daemon and can be
//! queried across all workspaces.

use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;

use crate::cli::args::{CliOutputFormat, WorkerCommand};
use crate::cli::workers;
use crate::cli::workers::WorkerOutput;
use crate::daemon::{LogTarget, StartWorkerRequest, ensure_daemon};
use crate::error::{GranaryError, Result};
use crate::models::Worker;
use crate::output::Output;
use crate::services::{Workspace, global_config_service};

// =============================================================================
// Output Types
// =============================================================================

/// Output for worker status with run statistics
pub struct WorkerStatusOutput {
    pub worker: Worker,
    pub running: usize,
    pub pending: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Serialize)]
struct WorkerStatusJson {
    worker: serde_json::Value,
    run_statistics: RunStatisticsJson,
}

#[derive(Serialize)]
struct RunStatisticsJson {
    running: usize,
    pending: usize,
    completed: usize,
    failed: usize,
}

impl Output for WorkerStatusOutput {
    fn to_json(&self) -> String {
        let worker_val = serde_json::to_value(&self.worker)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        serde_json::to_string_pretty(&WorkerStatusJson {
            worker: worker_val,
            run_statistics: RunStatisticsJson {
                running: self.running,
                pending: self.pending,
                completed: self.completed,
                failed: self.failed,
            },
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        let worker_out = WorkerOutput {
            worker: self.worker.clone(),
        };
        format!(
            "{}\nRun Statistics: running={}, pending={}, completed={}, failed={}",
            worker_out.to_prompt(),
            self.running,
            self.pending,
            self.completed,
            self.failed,
        )
    }

    fn to_text(&self) -> String {
        let worker_out = WorkerOutput {
            worker: self.worker.clone(),
        };
        format!(
            "{}\nRun Statistics:\n  Running:   {}\n  Pending:   {}\n  Completed: {}\n  Failed:    {}",
            worker_out.to_text(),
            self.running,
            self.pending,
            self.completed,
            self.failed,
        )
    }
}

/// Output for worker stop
pub struct WorkerStopOutput {
    pub worker: Worker,
}

#[derive(Serialize)]
struct WorkerStopJson {
    stopped: bool,
    worker: serde_json::Value,
}

impl Output for WorkerStopOutput {
    fn to_json(&self) -> String {
        let worker_val = serde_json::to_value(&self.worker)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        serde_json::to_string_pretty(&WorkerStopJson {
            stopped: true,
            worker: worker_val,
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        let worker_out = WorkerOutput {
            worker: self.worker.clone(),
        };
        format!("Worker stopped\n{}", worker_out.to_prompt())
    }

    fn to_text(&self) -> String {
        let worker_out = WorkerOutput {
            worker: self.worker.clone(),
        };
        format!("Worker stopped.\n{}", worker_out.to_text())
    }
}

/// Output for worker prune
pub struct WorkerPruneOutput {
    pub pruned_count: i32,
}

#[derive(Serialize)]
struct WorkerPruneJson {
    pruned_count: i32,
}

impl Output for WorkerPruneOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&WorkerPruneJson {
            pruned_count: self.pruned_count,
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        if self.pruned_count == 0 {
            "No workers pruned".to_string()
        } else {
            format!("Pruned {} worker(s)", self.pruned_count)
        }
    }

    fn to_text(&self) -> String {
        if self.pruned_count == 0 {
            "No workers to prune.".to_string()
        } else {
            format!("Pruned {} worker(s).", self.pruned_count)
        }
    }
}

/// Handle worker commands
pub async fn worker(
    id: Option<String>,
    command: Option<WorkerCommand>,
    all: bool,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    match command {
        Some(WorkerCommand::Start {
            runner,
            command,
            args,
            on,
            filters,
            detached,
            concurrency,
            poll_cooldown,
        }) => {
            start_worker(StartWorkerArgs {
                runner_name: runner,
                inline_command: command,
                args,
                event_type: on,
                filters,
                detached,
                concurrency,
                poll_cooldown_secs: poll_cooldown,
                cli_format,
            })
            .await
        }
        Some(WorkerCommand::Prune) => prune_workers(cli_format).await,
        Some(WorkerCommand::Status) => {
            let worker_id = id.ok_or_else(|| {
                GranaryError::InvalidArgument("Worker ID is required".to_string())
            })?;
            show_status(&worker_id, cli_format).await
        }
        Some(WorkerCommand::Logs { follow, lines }) => {
            let worker_id = id.ok_or_else(|| {
                GranaryError::InvalidArgument("Worker ID is required".to_string())
            })?;
            show_logs(&worker_id, follow, lines).await
        }
        Some(WorkerCommand::Stop { runs }) => {
            let worker_id = id.ok_or_else(|| {
                GranaryError::InvalidArgument("Worker ID is required".to_string())
            })?;
            stop_worker(&worker_id, runs, cli_format).await
        }
        None => match id {
            Some(worker_id) => show_status(&worker_id, cli_format).await,
            None => {
                workers::list_workers(all, cli_format, watch, interval).await?;
                Ok(())
            }
        },
    }
}

struct StartWorkerArgs {
    runner_name: Option<String>,
    inline_command: Option<String>,
    args: Vec<String>,
    event_type: Option<String>,
    filters: Vec<String>,
    detached: bool,
    concurrency: u32,
    poll_cooldown_secs: i64,
    cli_format: Option<CliOutputFormat>,
}

/// Start a new worker via the daemon
async fn start_worker(args: StartWorkerArgs) -> Result<()> {
    let StartWorkerArgs {
        runner_name,
        inline_command,
        args: cli_args,
        event_type,
        filters,
        detached,
        concurrency,
        poll_cooldown_secs,
        cli_format,
    } = args;

    // Validate we have either a runner or an inline command
    let (command, final_args, final_concurrency, final_event_type) =
        match (&runner_name, &inline_command) {
            (Some(name), None) => {
                // Load runner from config
                let runner = global_config_service::get_runner(name)?
                    .ok_or_else(|| GranaryError::RunnerNotFound(name.clone()))?;

                let concurrency = if concurrency == 1 {
                    runner.concurrency.unwrap_or(1)
                } else {
                    concurrency
                };

                // Resolve event type: CLI arg takes precedence, then runner config
                let resolved_event_type = event_type.or(runner.on.clone()).ok_or_else(|| {
                    GranaryError::InvalidArgument(format!(
                        "Must specify --on event type (runner '{}' has no default 'on' configured)",
                        name
                    ))
                })?;

                // Merge args: runner args first, then CLI args
                let mut merged_args = runner.expand_env_in_args();
                merged_args.extend(cli_args);

                (
                    runner.command,
                    merged_args,
                    concurrency,
                    resolved_event_type,
                )
            }
            (None, Some(cmd)) => {
                // Inline command requires --on
                let resolved_event_type = event_type.ok_or_else(|| {
                    GranaryError::InvalidArgument(
                        "Must specify --on event type when using inline --command".to_string(),
                    )
                })?;
                (cmd.clone(), cli_args, concurrency, resolved_event_type)
            }
            (Some(_), Some(_)) => {
                return Err(GranaryError::InvalidArgument(
                    "Cannot specify both --runner and --command".to_string(),
                ));
            }
            (None, None) => {
                return Err(GranaryError::InvalidArgument(
                    "Must specify either --runner or --command".to_string(),
                ));
            }
        };

    // Get workspace path
    let workspace = Workspace::find()?;
    let instance_path = workspace.root.to_string_lossy().to_string();

    // Connect to daemon (auto-starts if needed)
    let mut client = ensure_daemon().await?;

    // Start worker via daemon
    let req = StartWorkerRequest {
        runner_name,
        command,
        args: final_args,
        event_type: final_event_type,
        filters,
        concurrency: final_concurrency as i32,
        instance_path,
        attach: !detached,
        poll_cooldown_secs: Some(poll_cooldown_secs),
    };

    let worker = client.start_worker(req).await?;

    let output = WorkerOutput {
        worker: worker.clone(),
    };
    println!("{}", output.format(cli_format));

    if !detached {
        // Stream logs until Ctrl+C
        println!();
        println!("Streaming worker logs (Ctrl+C to detach)...");
        println!();

        // Stream logs via file-based tailing
        let log_dir = global_config_service::worker_logs_dir(&worker.id)?;
        let worker_log = log_dir.join("worker.log");

        // Wait for log file to be created
        for _ in 0..50 {
            if worker_log.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if worker_log.exists() {
            follow_log(&worker_log, 0).await?;
        } else {
            println!("Waiting for worker logs...");
            // Keep waiting and checking
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        println!();
                        println!("Detached from worker {}. Worker continues running in background.", worker.id);
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {
                        if worker_log.exists() {
                            follow_log(&worker_log, 0).await?;
                            break;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Show worker status via the daemon
async fn show_status(worker_id: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    // Connect to daemon (auto-starts if needed)
    let mut client = ensure_daemon().await?;

    // Get worker from daemon
    let worker = client.get_worker(worker_id).await?;

    // Get run statistics via daemon
    let runs = client.list_runs(Some(worker_id), None, true).await?;

    let output = WorkerStatusOutput {
        worker,
        running: runs.iter().filter(|r| r.is_running()).count(),
        pending: runs.iter().filter(|r| r.status == "pending").count(),
        completed: runs.iter().filter(|r| r.status == "completed").count(),
        failed: runs.iter().filter(|r| r.status == "failed").count(),
    };
    println!("{}", output.format(cli_format));

    Ok(())
}

/// Show worker logs via the daemon
async fn show_logs(worker_id: &str, follow: bool, lines: usize) -> Result<()> {
    // Connect to daemon (auto-starts if needed)
    let mut client = ensure_daemon().await?;

    // Verify worker exists by getting it from daemon
    let worker = client.get_worker(worker_id).await?;

    if follow {
        // Use daemon-based log streaming for follow mode
        println!("--- Following worker logs via daemon (Ctrl+C to stop) ---");

        // Use Ctrl+C to break out of the follow loop
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        let _ctrlc_task = tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(()).await;
        });

        // Stream logs via daemon with polling

        // Get initial lines from the end
        let initial_response = client
            .get_logs(&worker.id, LogTarget::Worker, 0, u64::MAX)
            .await?;
        let total_lines = initial_response.next_line;
        let mut since_line = total_lines.saturating_sub(lines as u64);

        // Print initial lines
        if since_line < total_lines {
            let response = client
                .get_logs(&worker.id, LogTarget::Worker, since_line, 1000)
                .await?;
            for line in &response.lines {
                println!("{}", line);
            }
            since_line = response.next_line;
        }

        // Poll for new lines
        loop {
            // Check for Ctrl+C
            match rx.try_recv() {
                Ok(()) | Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    println!();
                    println!("Stopped following logs.");
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
            }

            let response = client
                .get_logs(&worker.id, LogTarget::Worker, since_line, 100)
                .await?;

            for line in &response.lines {
                println!("{}", line);
            }

            since_line = response.next_line;

            // If worker is no longer active and no more lines, we're done
            if !response.has_more && response.lines.is_empty() {
                println!("--- Worker is no longer active ---");
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    } else {
        // Non-follow mode: get logs via daemon
        let response = client
            .get_logs(&worker.id, LogTarget::Worker, 0, u64::MAX)
            .await?;

        let total_lines = response.next_line;
        let start_line = total_lines.saturating_sub(lines as u64);

        // Get the last N lines
        let response = client
            .get_logs(&worker.id, LogTarget::Worker, start_line, lines as u64)
            .await?;

        if response.lines.is_empty() {
            println!("No log lines found for worker {}", worker_id);
            if let Some(path) = response.log_path {
                println!("Log path: {}", path.display());
            }
        } else {
            for line in &response.lines {
                println!("{}", line);
            }
        }
    }

    Ok(())
}

/// Print the last N lines of a log file
pub fn print_log_tail(path: &PathBuf, lines: usize) -> Result<()> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().collect::<std::io::Result<_>>()?;

    let start = if all_lines.len() > lines {
        all_lines.len() - lines
    } else {
        0
    };

    for line in &all_lines[start..] {
        println!("{}", line);
    }

    Ok(())
}

/// Follow a log file like tail -f
pub async fn follow_log(path: &PathBuf, initial_lines: usize) -> Result<()> {
    // Print initial lines
    print_log_tail(path, initial_lines)?;

    // Open file for following
    let mut file = std::fs::File::open(path)?;
    file.seek(SeekFrom::End(0))?;

    let mut reader = BufReader::new(file);
    let mut line = String::new();

    println!("--- Following log (Ctrl+C to stop) ---");

    loop {
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new data, wait a bit
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            Ok(_) => {
                print!("{}", line);
                line.clear();
            }
            Err(e) => {
                eprintln!("Error reading log: {}", e);
                break;
            }
        }

        // Check for shutdown signal via select
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                break;
            }
            _ = tokio::time::sleep(Duration::from_millis(10)) => {
                // Continue reading
            }
        }
    }

    Ok(())
}

/// Stop a worker via the daemon
async fn stop_worker(
    worker_id: &str,
    stop_runs: bool,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    // Connect to daemon (auto-starts if needed)
    let mut client = ensure_daemon().await?;

    // Stop worker via daemon
    client.stop_worker(worker_id, stop_runs).await?;

    // Get updated worker status from daemon
    let worker = client.get_worker(worker_id).await?;

    let output = WorkerStopOutput { worker };
    println!("{}", output.format(cli_format));

    Ok(())
}

/// Prune stopped/errored workers via the daemon
async fn prune_workers(cli_format: Option<CliOutputFormat>) -> Result<()> {
    // Connect to daemon (auto-starts if needed)
    let mut client = ensure_daemon().await?;

    // Prune workers via daemon
    let pruned = client.prune_workers().await?;

    let output = WorkerPruneOutput {
        pruned_count: pruned,
    };
    println!("{}", output.format(cli_format));

    Ok(())
}
