//! Run CLI commands for managing individual runner executions.
//!
//! Runs are individual executions spawned by workers. Users can monitor run status,
//! view logs, and control run lifecycle (stop, pause, resume).

use std::time::Duration;

use serde::Serialize;

use crate::cli::args::{CliOutputFormat, RunCommand};
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::daemon::{LogTarget, ensure_daemon};
use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::run::{Run, RunStatus, UpdateRunStatus};
use crate::output::{Output, json, table};
use crate::services::global_config_service;

// =============================================================================
// Output Types
// =============================================================================

/// Output for a list of runs
pub struct RunsOutput {
    pub runs: Vec<Run>,
    pub show_all_hint: bool,
}

impl Output for RunsOutput {
    fn to_json(&self) -> String {
        json::format_runs(&self.runs)
    }

    fn to_prompt(&self) -> String {
        if self.runs.is_empty() {
            return "No runs found.".to_string();
        }
        // Runs don't have a prompt formatter, use table format
        table::format_runs(&self.runs)
    }

    fn to_text(&self) -> String {
        if self.runs.is_empty() {
            let mut msg = "No active runs found.".to_string();
            if self.show_all_hint {
                msg.push_str("\nUse --all to include completed/failed/cancelled runs.");
            }
            return msg;
        }
        table::format_runs(&self.runs)
    }
}

/// Output for a single run
pub struct RunOutput {
    pub run: Run,
}

impl Output for RunOutput {
    fn to_json(&self) -> String {
        json::format_run(&self.run)
    }

    fn to_prompt(&self) -> String {
        // Runs don't have a prompt formatter, use table format
        table::format_run(&self.run)
    }

    fn to_text(&self) -> String {
        table::format_run(&self.run)
    }
}

/// Output for run stop
pub struct RunStopOutput {
    pub run: Run,
}

#[derive(Serialize)]
struct RunStopJson {
    stopped: bool,
    run: serde_json::Value,
}

impl Output for RunStopOutput {
    fn to_json(&self) -> String {
        let run_val = serde_json::to_value(&self.run)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        serde_json::to_string_pretty(&RunStopJson {
            stopped: true,
            run: run_val,
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        let run_out = RunOutput {
            run: self.run.clone(),
        };
        format!("Run stopped\n{}", run_out.to_prompt())
    }

    fn to_text(&self) -> String {
        let run_out = RunOutput {
            run: self.run.clone(),
        };
        format!("Run stopped.\n{}", run_out.to_text())
    }
}

/// Output for run pause
pub struct RunPauseOutput {
    pub run: Run,
}

#[derive(Serialize)]
struct RunPauseJson {
    paused: bool,
    run: serde_json::Value,
}

impl Output for RunPauseOutput {
    fn to_json(&self) -> String {
        let run_val = serde_json::to_value(&self.run)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        serde_json::to_string_pretty(&RunPauseJson {
            paused: true,
            run: run_val,
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        let run_out = RunOutput {
            run: self.run.clone(),
        };
        format!("Run paused\n{}", run_out.to_prompt())
    }

    fn to_text(&self) -> String {
        let run_out = RunOutput {
            run: self.run.clone(),
        };
        format!("Run paused.\n{}", run_out.to_text())
    }
}

/// Output for run resume
pub struct RunResumeOutput {
    pub run: Run,
}

#[derive(Serialize)]
struct RunResumeJson {
    resumed: bool,
    run: serde_json::Value,
}

impl Output for RunResumeOutput {
    fn to_json(&self) -> String {
        let run_val = serde_json::to_value(&self.run)
            .unwrap_or(serde_json::Value::Object(Default::default()));
        serde_json::to_string_pretty(&RunResumeJson {
            resumed: true,
            run: run_val,
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        let run_out = RunOutput {
            run: self.run.clone(),
        };
        format!("Run resumed\n{}", run_out.to_prompt())
    }

    fn to_text(&self) -> String {
        let run_out = RunOutput {
            run: self.run.clone(),
        };
        format!("Run resumed.\n{}", run_out.to_text())
    }
}

/// List all runs with optional filters
pub async fn list_runs(
    worker_id: Option<String>,
    status: Option<String>,
    all: bool,
    limit: u32,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    if watch {
        let interval_duration = Duration::from_secs(interval);
        watch_loop(interval_duration, || {
            let worker_id = worker_id.clone();
            let status = status.clone();
            async move {
                let output = fetch_and_format_runs(
                    worker_id.as_deref(),
                    status.as_deref(),
                    all,
                    limit,
                    cli_format,
                )
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(format!(
                    "{}\n{}",
                    watch_status_line(interval_duration),
                    output
                ))
            }
        })
        .await?;
    } else {
        let output = fetch_and_format_runs(
            worker_id.as_deref(),
            status.as_deref(),
            all,
            limit,
            cli_format,
        )
        .await?;
        print!("{}", output);
    }
    Ok(())
}

/// Fetch and format runs for display
async fn fetch_and_format_runs(
    worker_id: Option<&str>,
    status: Option<&str>,
    all: bool,
    limit: u32,
    cli_format: Option<CliOutputFormat>,
) -> Result<String> {
    let global_pool = global_config_service::global_pool().await?;

    // Parse status filter if provided
    let status_filter: Option<RunStatus> = status.and_then(|s| s.parse().ok());

    // Get runs based on filters
    let runs = if let Some(worker) = worker_id {
        db::runs::list_by_worker(&global_pool, worker).await?
    } else {
        db::runs::list_all(&global_pool).await?
    };

    // Filter runs
    let mut runs: Vec<_> = runs
        .into_iter()
        .filter(|r| {
            // Filter by status if specified
            if let Some(ref status) = status_filter
                && r.status_enum() != *status
            {
                return false;
            }

            // By default, only show pending/running/paused unless --all
            if !all {
                let run_status = r.status_enum();
                if matches!(
                    run_status,
                    RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
                ) {
                    return false;
                }
            }

            true
        })
        .take(limit as usize)
        .collect();

    // Sort by created_at descending (most recent first)
    runs.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    let output = RunsOutput {
        runs,
        show_all_hint: !all,
    };
    Ok(format!("{}\n", output.format(cli_format)))
}

/// Handle run subcommands
#[allow(clippy::too_many_arguments)]
pub async fn run(
    id: Option<String>,
    command: Option<RunCommand>,
    worker: Option<String>,
    status: Option<String>,
    all: bool,
    limit: u32,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    let require_id = || -> Result<String> {
        id.clone()
            .ok_or_else(|| GranaryError::InvalidArgument("Run ID is required".to_string()))
    };

    match command {
        Some(RunCommand::Status) => show_status(&require_id()?, cli_format).await,
        Some(RunCommand::Logs { follow, lines }) => show_logs(&require_id()?, follow, lines).await,
        Some(RunCommand::Stop) => stop_run(&require_id()?, cli_format).await,
        Some(RunCommand::Pause) => pause_run(&require_id()?, cli_format).await,
        Some(RunCommand::Resume) => resume_run(&require_id()?, cli_format).await,
        None => match id {
            Some(run_id) => show_status(&run_id, cli_format).await,
            None => list_runs(worker, status, all, limit, cli_format, watch, interval).await,
        },
    }
}

/// Show run status and details
async fn show_status(run_id: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let global_pool = global_config_service::global_pool().await?;

    let run = db::runs::get(&global_pool, run_id)
        .await?
        .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

    let output = RunOutput { run: run.clone() };
    println!("{}", output.format(cli_format));

    // Check if process is actually running (if run says it's running)
    if run.status_enum() == RunStatus::Running
        && let Some(pid) = run.pid
    {
        let is_alive = is_process_alive(pid as u32);
        if !is_alive {
            println!();
            println!(
                "WARNING: Run is marked as running but process {} is not alive.",
                pid
            );
        }
    }

    Ok(())
}

/// Show run logs
async fn show_logs(run_id: &str, follow: bool, lines: usize) -> Result<()> {
    // Connect to daemon (auto-starts if needed)
    let mut client = ensure_daemon().await?;

    // Verify run exists
    let _run = client.get_run(run_id).await?;

    if follow {
        // Use daemon-based log streaming for follow mode
        println!("--- Following run logs via daemon (Ctrl+C to stop) ---");

        // Use Ctrl+C to break out of the follow loop
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        let _ctrlc_task = tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = tx.send(()).await;
        });

        // Stream logs via daemon with polling

        // Get initial lines from the end
        let initial_response = client.get_logs(run_id, LogTarget::Run, 0, u64::MAX).await?;
        let total_lines = initial_response.next_line;
        let mut since_line = total_lines.saturating_sub(lines as u64);

        // Print initial lines
        if since_line < total_lines {
            let response = client
                .get_logs(run_id, LogTarget::Run, since_line, 1000)
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
                .get_logs(run_id, LogTarget::Run, since_line, 100)
                .await?;

            for line in &response.lines {
                println!("{}", line);
            }

            since_line = response.next_line;

            // If run is no longer active and no more lines, we're done
            if !response.has_more && response.lines.is_empty() {
                println!("--- Run is no longer active ---");
                break;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    } else {
        // Non-follow mode: get logs via daemon
        let response = client.get_logs(run_id, LogTarget::Run, 0, u64::MAX).await?;

        let total_lines = response.next_line;
        let start_line = total_lines.saturating_sub(lines as u64);

        // Get the last N lines
        let response = client
            .get_logs(run_id, LogTarget::Run, start_line, lines as u64)
            .await?;

        if response.lines.is_empty() {
            println!("No log lines found for run {}", run_id);
            if let Some(path) = response.log_path {
                println!("Log path: {}", path.display());
            }
            println!();
            println!("The log file may not exist yet if the run hasn't started,");
            println!("or it may have been cleaned up.");
        } else {
            for line in &response.lines {
                println!("{}", line);
            }
        }
    }

    Ok(())
}

/// Stop a running run
async fn stop_run(run_id: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let global_pool = global_config_service::global_pool().await?;

    let run = db::runs::get(&global_pool, run_id)
        .await?
        .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

    let run_status = run.status_enum();

    // Check if run can be stopped
    if run_status == RunStatus::Completed
        || run_status == RunStatus::Failed
        || run_status == RunStatus::Cancelled
    {
        let output = RunStopOutput { run };
        println!("{}", output.format(cli_format));
        return Ok(());
    }

    // If run has a PID and is running, try to send SIGTERM
    if let Some(pid) = run.pid
        && (run_status == RunStatus::Running || run_status == RunStatus::Paused)
        && is_process_alive(pid as u32)
    {
        send_signal(pid as u32, Signal::Term);

        // Wait a bit for the process to exit
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    // Update run status to cancelled
    let update = UpdateRunStatus {
        status: RunStatus::Cancelled,
        exit_code: None,
        error_message: Some("Manually cancelled".to_string()),
        pid: None,
    };
    db::runs::update_status(&global_pool, run_id, &update).await?;

    let run = db::runs::get(&global_pool, run_id)
        .await?
        .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;
    let output = RunStopOutput { run };
    println!("{}", output.format(cli_format));

    Ok(())
}

/// Pause a running run (sends SIGSTOP)
async fn pause_run(run_id: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let global_pool = global_config_service::global_pool().await?;

    let run = db::runs::get(&global_pool, run_id)
        .await?
        .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

    // Check if run is running
    if run.status_enum() != RunStatus::Running {
        return Err(GranaryError::InvalidArgument(format!(
            "Cannot pause run: status is '{}', must be 'running'",
            run.status
        )));
    }

    // Send SIGSTOP to process
    if let Some(pid) = run.pid {
        if is_process_alive(pid as u32) {
            send_signal(pid as u32, Signal::Stop);
        } else {
            return Err(GranaryError::InvalidArgument(format!(
                "Process {} is not alive",
                pid
            )));
        }
    } else {
        return Err(GranaryError::InvalidArgument(
            "Run has no PID, cannot pause".to_string(),
        ));
    }

    // Update run status to paused
    let update = UpdateRunStatus {
        status: RunStatus::Paused,
        exit_code: None,
        error_message: None,
        pid: run.pid,
    };
    db::runs::update_status(&global_pool, run_id, &update).await?;

    let run = db::runs::get(&global_pool, run_id)
        .await?
        .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;
    let output = RunPauseOutput { run };
    println!("{}", output.format(cli_format));

    Ok(())
}

/// Resume a paused run (sends SIGCONT)
async fn resume_run(run_id: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let global_pool = global_config_service::global_pool().await?;

    let run = db::runs::get(&global_pool, run_id)
        .await?
        .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

    // Check if run is paused
    if run.status_enum() != RunStatus::Paused {
        return Err(GranaryError::InvalidArgument(format!(
            "Cannot resume run: status is '{}', must be 'paused'",
            run.status
        )));
    }

    // Send SIGCONT to process
    if let Some(pid) = run.pid {
        if is_process_alive(pid as u32) {
            send_signal(pid as u32, Signal::Cont);
        } else {
            return Err(GranaryError::InvalidArgument(format!(
                "Process {} is not alive",
                pid
            )));
        }
    } else {
        return Err(GranaryError::InvalidArgument(
            "Run has no PID, cannot resume".to_string(),
        ));
    }

    // Update run status to running
    let update = UpdateRunStatus {
        status: RunStatus::Running,
        exit_code: None,
        error_message: None,
        pid: run.pid,
    };
    db::runs::update_status(&global_pool, run_id, &update).await?;

    let run = db::runs::get(&global_pool, run_id)
        .await?
        .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;
    let output = RunResumeOutput { run };
    println!("{}", output.format(cli_format));

    Ok(())
}

/// Signal types for process control
enum Signal {
    Term,
    Stop,
    Cont,
}

/// Check if a process is alive by using kill -0
#[cfg_attr(not(unix), allow(unused_variables))]
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Use kill -0 to check if process exists
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        // On non-unix platforms, assume it's alive if we can't check
        true
    }
}

/// Send a signal to a process
#[cfg_attr(not(unix), allow(unused_variables))]
fn send_signal(pid: u32, signal: Signal) {
    #[cfg(unix)]
    {
        let sig = match signal {
            Signal::Term => "-TERM",
            Signal::Stop => "-STOP",
            Signal::Cont => "-CONT",
        };
        let _ = std::process::Command::new("kill")
            .args([sig, &pid.to_string()])
            .output();
    }

    #[cfg(not(unix))]
    {
        eprintln!("Warning: Cannot send signals on this platform.");
    }
}
