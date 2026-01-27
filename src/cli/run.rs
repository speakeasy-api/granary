//! Run CLI commands for managing individual runner executions.
//!
//! Runs are individual executions spawned by workers. Users can monitor run status,
//! view logs, and control run lifecycle (stop, pause, resume).

use std::time::Duration;

use crate::cli::args::RunCommand;
use crate::daemon::{LogTarget, ensure_daemon};
use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::run::{RunStatus, UpdateRunStatus};
use crate::output::{Formatter, OutputFormat};
use crate::services::global_config_service;

/// List all runs with optional filters
pub async fn list_runs(
    worker_id: Option<String>,
    status: Option<String>,
    all: bool,
    limit: u32,
    format: OutputFormat,
) -> Result<()> {
    let global_pool = global_config_service::global_pool().await?;

    // Parse status filter if provided
    let status_filter: Option<RunStatus> = status.as_ref().and_then(|s| s.parse().ok());

    // Get runs based on filters
    let runs = if let Some(worker) = &worker_id {
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

    if runs.is_empty() {
        if all {
            println!("No runs found.");
        } else {
            println!("No active runs found.");
            println!("Use --all to include completed/failed/cancelled runs.");
        }
        return Ok(());
    }

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_runs(&runs));

    Ok(())
}

/// Handle run subcommands
pub async fn run(command: RunCommand, format: OutputFormat) -> Result<()> {
    match command {
        RunCommand::Status { run_id } => show_status(&run_id, format).await,
        RunCommand::Logs {
            run_id,
            follow,
            lines,
        } => show_logs(&run_id, follow, lines).await,
        RunCommand::Stop { run_id } => stop_run(&run_id, format).await,
        RunCommand::Pause { run_id } => pause_run(&run_id, format).await,
        RunCommand::Resume { run_id } => resume_run(&run_id, format).await,
    }
}

/// Show run status and details
async fn show_status(run_id: &str, format: OutputFormat) -> Result<()> {
    let global_pool = global_config_service::global_pool().await?;

    let run = db::runs::get(&global_pool, run_id)
        .await?
        .ok_or_else(|| GranaryError::RunNotFound(run_id.to_string()))?;

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_run(&run));

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
async fn stop_run(run_id: &str, format: OutputFormat) -> Result<()> {
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
        println!(
            "Run {} is already finished (status: {})",
            run_id, run.status
        );
        return Ok(());
    }

    // If run has a PID and is running, try to send SIGTERM
    if let Some(pid) = run.pid
        && (run_status == RunStatus::Running || run_status == RunStatus::Paused)
        && is_process_alive(pid as u32)
    {
        println!("Sending SIGTERM to process {}...", pid);
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
    let formatter = Formatter::new(format);
    println!("Run stopped.");
    println!("{}", formatter.format_run(&run));

    Ok(())
}

/// Pause a running run (sends SIGSTOP)
async fn pause_run(run_id: &str, format: OutputFormat) -> Result<()> {
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
            println!("Sending SIGSTOP to process {}...", pid);
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
    let formatter = Formatter::new(format);
    println!("Run paused.");
    println!("{}", formatter.format_run(&run));

    Ok(())
}

/// Resume a paused run (sends SIGCONT)
async fn resume_run(run_id: &str, format: OutputFormat) -> Result<()> {
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
            println!("Sending SIGCONT to process {}...", pid);
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
    let formatter = Formatter::new(format);
    println!("Run resumed.");
    println!("{}", formatter.format_run(&run));

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
