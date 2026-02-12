//! Granary daemon - manages worker lifecycles via IPC.
//!
//! The granaryd binary is a long-running background process that:
//! - Accepts IPC connections from the CLI over Unix domain sockets
//! - Manages worker lifecycles (start, stop, query)
//! - Handles graceful shutdown on SIGTERM/SIGINT
//! - Restores workers that were running before the daemon stopped
//!
//! ## Usage
//!
//! The daemon is typically started automatically by the CLI when needed.
//! Manual start: `granaryd`
//!
//! ## Files
//!
//! - `~/.granary/daemon/granaryd.sock` - Unix socket for IPC
//! - `~/.granary/daemon/granaryd.pid` - PID file for process tracking
//! - `~/.granary/daemon/daemon.log` - Daemon log file

use std::sync::Arc;
use std::time::Duration;

use tokio::select;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};
use tracing_appender::non_blocking::WorkerGuard;

use granary::daemon::IpcConnection;
use granary::daemon::listener::IpcListener;
use granary::daemon::protocol::{Operation, Request, Response};
use granary::daemon::worker_manager::WorkerManager;
use granary::models::LogRetentionConfig;
use granary::services::global_config as global_config_service;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Ensure daemon directory exists
    let daemon_dir = global_config_service::daemon_dir()?;
    std::fs::create_dir_all(&daemon_dir)?;

    // Initialize logging to daemon log file
    let _guard = init_logging(&daemon_dir)?;

    tracing::info!("granaryd starting, version {}", env!("CARGO_PKG_VERSION"));

    // Pre-generate the auth token so clients can read it before connecting
    // The token is created at ~/.granary/daemon/auth.token with 0600 permissions
    let _auth_token = global_config_service::get_or_create_auth_token()?;
    tracing::debug!(
        "Auth token ready at {:?}",
        global_config_service::daemon_auth_token_path()?
    );

    // Write PID file
    let pid_path = global_config_service::daemon_pid_path()?;
    std::fs::write(&pid_path, std::process::id().to_string())?;

    // Open global database
    let global_pool = global_config_service::global_pool().await?;

    // Create worker manager
    let manager = Arc::new(WorkerManager::new(global_pool));

    // Restore workers that were running before daemon stopped
    if let Err(e) = manager.restore_workers().await {
        tracing::warn!("Failed to restore workers: {}", e);
    }

    // Start IPC listener
    #[cfg(unix)]
    #[allow(unused_mut)] // Windows needs mut for accept(), Unix doesn't
    let mut listener = {
        let socket_path = global_config_service::daemon_socket_path()?;
        let listener = IpcListener::bind(&socket_path).await?;
        tracing::info!("granaryd listening on {:?}", listener.socket_path());
        listener
    };

    #[cfg(windows)]
    let mut listener = {
        let pipe_name = global_config_service::daemon_pipe_name();
        let listener = IpcListener::bind(&pipe_name).await?;
        tracing::info!("granaryd listening on {}", listener.pipe_name());
        listener
    };

    // Set up signal handlers
    #[cfg(unix)]
    let mut sigterm = signal(SignalKind::terminate())?;
    #[cfg(unix)]
    let mut sigint = signal(SignalKind::interrupt())?;

    // Flag to track shutdown request from IPC
    let shutdown_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Set up log cleanup interval (every hour)
    let mut cleanup_interval = tokio::time::interval(Duration::from_secs(3600));
    // Skip the first immediate tick
    cleanup_interval.tick().await;

    // Run initial log cleanup on startup
    let log_retention_config = LogRetentionConfig::default();
    match manager.cleanup_old_logs(&log_retention_config) {
        Ok(deleted) if deleted > 0 => {
            tracing::info!("Initial log cleanup: deleted {} old log files", deleted);
        }
        Err(e) => {
            tracing::warn!("Initial log cleanup failed: {}", e);
        }
        _ => {}
    }

    // Main loop - Unix version with SIGTERM/SIGINT handling
    #[cfg(unix)]
    loop {
        // Check if shutdown was requested via IPC
        if shutdown_flag.load(std::sync::atomic::Ordering::SeqCst) {
            tracing::info!("Shutdown requested via IPC");
            break;
        }

        select! {
            // Handle shutdown signals
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, shutting down...");
                break;
            }
            _ = sigint.recv() => {
                tracing::info!("Received SIGINT, shutting down...");
                break;
            }

            // Periodic log cleanup
            _ = cleanup_interval.tick() => {
                match manager.cleanup_old_logs(&log_retention_config) {
                    Ok(deleted) if deleted > 0 => {
                        tracing::info!("Periodic log cleanup: deleted {} old log files", deleted);
                    }
                    Err(e) => {
                        tracing::warn!("Periodic log cleanup failed: {}", e);
                    }
                    _ => {}
                }
            }

            // Accept new connections
            result = listener.accept() => {
                match result {
                    Ok(conn) => {
                        let manager = Arc::clone(&manager);
                        let shutdown_flag = Arc::clone(&shutdown_flag);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(conn, &manager, &shutdown_flag).await {
                                tracing::error!("Connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Accept error: {}", e);
                    }
                }
            }
        }
    }

    // Main loop - Windows version with Ctrl+C handling
    #[cfg(windows)]
    loop {
        // Check if shutdown was requested via IPC
        if shutdown_flag.load(std::sync::atomic::Ordering::SeqCst) {
            tracing::info!("Shutdown requested via IPC");
            break;
        }

        select! {
            // Handle Ctrl+C
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received Ctrl+C, shutting down...");
                break;
            }

            // Periodic log cleanup
            _ = cleanup_interval.tick() => {
                match manager.cleanup_old_logs(&log_retention_config) {
                    Ok(deleted) if deleted > 0 => {
                        tracing::info!("Periodic log cleanup: deleted {} old log files", deleted);
                    }
                    Err(e) => {
                        tracing::warn!("Periodic log cleanup failed: {}", e);
                    }
                    _ => {}
                }
            }

            // Accept new connections
            result = listener.accept() => {
                match result {
                    Ok(conn) => {
                        let manager = Arc::clone(&manager);
                        let shutdown_flag = Arc::clone(&shutdown_flag);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(conn, &manager, &shutdown_flag).await {
                                tracing::error!("Connection error: {}", e);
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("Accept error: {}", e);
                    }
                }
            }
        }
    }

    // Graceful shutdown
    tracing::info!("Shutting down workers...");
    manager.shutdown_all().await?;

    // Clean up PID file
    let _ = std::fs::remove_file(&pid_path);

    tracing::info!("granaryd shutdown complete");
    Ok(())
}

/// Handle a single client connection.
///
/// Processes requests in a loop until the connection is closed or
/// a shutdown operation is received.
///
/// The first message must be an Auth operation with a valid token.
/// Connections that fail authentication are rejected.
async fn handle_connection(
    mut conn: IpcConnection,
    manager: &WorkerManager,
    shutdown_flag: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<()> {
    // First message must be authentication
    let auth_request = match conn.recv_request().await {
        Ok(req) => req,
        Err(_) => return Ok(()), // Connection closed before auth
    };

    match &auth_request.op {
        Operation::Auth(auth) => {
            let expected = global_config_service::get_or_create_auth_token()?;
            if auth.token != expected {
                tracing::warn!("Authentication failed: invalid token");
                conn.send_response(&Response::err(auth_request.id, "Authentication failed"))
                    .await?;
                return Ok(());
            }
            conn.send_response(&Response::ok_empty(auth_request.id))
                .await?;
            tracing::debug!("Client authenticated successfully");
        }
        _ => {
            tracing::warn!("First message was not Auth, rejecting connection");
            conn.send_response(&Response::err(
                auth_request.id,
                "First message must be Auth",
            ))
            .await?;
            return Ok(());
        }
    }

    // Continue with normal request loop after successful authentication
    loop {
        let request = match conn.recv_request().await {
            Ok(req) => req,
            Err(_) => break, // Connection closed
        };

        let (response, should_shutdown) = dispatch_request(request, manager).await;
        conn.send_response(&response).await?;

        if should_shutdown {
            // Signal the main loop to shutdown
            shutdown_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            break;
        }
    }
    Ok(())
}

/// Dispatch a request to the appropriate handler.
///
/// Returns the response and a flag indicating if the daemon should shutdown.
async fn dispatch_request(request: Request, manager: &WorkerManager) -> (Response, bool) {
    let id = request.id;

    match request.op {
        // Auth is only valid as the first message; reject if sent later
        Operation::Auth(_) => {
            let response = Response::err(id, "Auth is only valid as the first message");
            (response, false)
        }

        Operation::Ping => {
            let response = Response::ok(
                id,
                serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "status": "running"
                }),
            );
            (response, false)
        }

        Operation::Shutdown => {
            // Return acknowledgment, then daemon will shutdown
            let response = Response::ok(id, "shutdown_ack");
            (response, true)
        }

        Operation::StartWorker(req) => {
            let create = granary::models::CreateWorker {
                runner_name: req.runner_name,
                command: req.command,
                args: req.args,
                event_type: req.event_type,
                filters: req.filters,
                concurrency: req.concurrency,
                instance_path: req.instance_path,
                detached: !req.attach,
                since: req.since,
            };

            match manager.start_worker(create).await {
                Ok(worker) => {
                    let response = Response::ok(id, &worker);
                    (response, false)
                }
                Err(e) => {
                    let response = Response::err(id, e.to_string());
                    (response, false)
                }
            }
        }

        Operation::StopWorker {
            worker_id,
            stop_runs,
        } => match manager.stop_worker(&worker_id, stop_runs).await {
            Ok(_) => (Response::ok_empty(id), false),
            Err(e) => (Response::err(id, e.to_string()), false),
        },

        Operation::GetWorker { worker_id } => match manager.get_worker(&worker_id).await {
            Ok(Some(worker)) => (Response::ok(id, &worker), false),
            Ok(None) => (
                Response::err(id, format!("Worker {} not found", worker_id)),
                false,
            ),
            Err(e) => (Response::err(id, e.to_string()), false),
        },

        Operation::ListWorkers { all } => match manager.list_workers(all).await {
            Ok(workers) => (Response::ok(id, &workers), false),
            Err(e) => (Response::err(id, e.to_string()), false),
        },

        Operation::PruneWorkers => match manager.prune_workers().await {
            Ok(pruned) => (
                Response::ok(id, serde_json::json!({ "pruned": pruned })),
                false,
            ),
            Err(e) => (Response::err(id, e.to_string()), false),
        },

        Operation::WorkerLogs {
            worker_id,
            follow,
            lines,
        } => {
            if follow {
                // For follow mode, use LogsResponse format with has_more indicator
                // This allows the client to poll for new lines and know when the worker stops
                match manager
                    .get_logs(
                        &worker_id,
                        granary::daemon::protocol::LogTarget::Worker,
                        0,
                        lines as u64,
                    )
                    .await
                {
                    Ok(response) => (Response::ok(id, &response), false),
                    Err(e) => (Response::err(id, e.to_string()), false),
                }
            } else {
                // Non-follow mode: get worker log path and read logs (simple string response)
                match manager.get_worker_log_path(&worker_id) {
                    Ok(path) => {
                        if path.exists() {
                            match read_log_tail(&path, lines as usize) {
                                Ok(logs) => {
                                    (Response::ok(id, serde_json::json!({ "logs": logs })), false)
                                }
                                Err(e) => (
                                    Response::err(id, format!("Failed to read logs: {}", e)),
                                    false,
                                ),
                            }
                        } else {
                            (
                                Response::ok(
                                    id,
                                    serde_json::json!({ "logs": "", "message": "No log file found" }),
                                ),
                                false,
                            )
                        }
                    }
                    Err(e) => (Response::err(id, e.to_string()), false),
                }
            }
        }

        Operation::GetRun { run_id } => match manager.get_run(&run_id).await {
            Ok(Some(run)) => (Response::ok(id, &run), false),
            Ok(None) => (
                Response::err(id, format!("Run {} not found", run_id)),
                false,
            ),
            Err(e) => (Response::err(id, e.to_string()), false),
        },

        Operation::ListRuns {
            worker_id,
            status,
            all,
        } => {
            match manager
                .list_runs(worker_id.as_deref(), status.as_deref(), all)
                .await
            {
                Ok(runs) => (Response::ok(id, &runs), false),
                Err(e) => (Response::err(id, e.to_string()), false),
            }
        }

        Operation::StopRun { run_id } => match manager.stop_run(&run_id).await {
            Ok(()) => (Response::ok_empty(id), false),
            Err(e) => (Response::err(id, e.to_string()), false),
        },

        Operation::PauseRun { run_id } => match manager.pause_run(&run_id).await {
            Ok(()) => (Response::ok_empty(id), false),
            Err(e) => (Response::err(id, e.to_string()), false),
        },

        Operation::ResumeRun { run_id } => match manager.resume_run(&run_id).await {
            Ok(()) => (Response::ok_empty(id), false),
            Err(e) => (Response::err(id, e.to_string()), false),
        },

        Operation::RunLogs {
            run_id,
            follow,
            lines,
        } => {
            if follow {
                // For follow mode, use LogsResponse format with has_more indicator
                // This allows the client to poll for new lines and know when the run completes
                match manager
                    .get_logs(
                        &run_id,
                        granary::daemon::protocol::LogTarget::Run,
                        0,
                        lines as u64,
                    )
                    .await
                {
                    Ok(response) => (Response::ok(id, &response), false),
                    Err(e) => (Response::err(id, e.to_string()), false),
                }
            } else {
                // Non-follow mode: get log path and read logs (simple string response)
                match manager.get_run_log_path(&run_id).await {
                    Ok(Some(path)) => {
                        // Read last N lines from log file
                        match read_log_tail(&path, lines as usize) {
                            Ok(logs) => {
                                (Response::ok(id, serde_json::json!({ "logs": logs })), false)
                            }
                            Err(e) => (
                                Response::err(id, format!("Failed to read logs: {}", e)),
                                false,
                            ),
                        }
                    }
                    Ok(None) => (
                        Response::ok(
                            id,
                            serde_json::json!({ "logs": "", "message": "No log file found" }),
                        ),
                        false,
                    ),
                    Err(e) => (Response::err(id, e.to_string()), false),
                }
            }
        }

        Operation::GetLogs(req) => {
            match manager
                .get_logs(&req.target_id, req.target_type, req.since_line, req.limit)
                .await
            {
                Ok(response) => (Response::ok(id, &response), false),
                Err(e) => (Response::err(id, e.to_string()), false),
            }
        }
    }
}

/// Read the last N lines from a log file
fn read_log_tail(path: &std::path::Path, lines: usize) -> std::io::Result<String> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().collect::<std::io::Result<_>>()?;

    let start = if all_lines.len() > lines {
        all_lines.len() - lines
    } else {
        0
    };

    Ok(all_lines[start..].join("\n"))
}

/// Initialize file-based logging for the daemon with daily rotation.
///
/// Sets up tracing-subscriber with a non-blocking file appender that writes to
/// `daemon.log` in the specified daemon directory. Log files are automatically
/// rotated daily with timestamps appended to the filename.
///
/// The returned `WorkerGuard` must be kept alive for the duration of the program
/// to ensure all logs are flushed.
///
/// # Log Rotation
///
/// Files are named with the pattern `daemon.log.YYYY-MM-DD` for rotated files,
/// keeping logs organized and preventing unbounded growth of a single log file.
fn init_logging(daemon_dir: &std::path::Path) -> anyhow::Result<WorkerGuard> {
    use tracing_subscriber::fmt::format::FmtSpan;

    // Create a file appender with daily rotation
    // This creates files like: daemon.log.2026-01-20
    let file_appender = tracing_appender::rolling::daily(daemon_dir, "daemon.log");

    // Make it non-blocking for better performance
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Initialize the subscriber
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    Ok(guard)
}
