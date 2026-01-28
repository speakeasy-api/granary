//! Daemon CLI commands for managing the granary daemon process.
//!
//! The daemon is a long-running background process that manages workers and runs.
//! These commands allow users to check its status, start/stop it manually,
//! and view its logs.

use std::path::Path;

use crate::cli::args::DaemonCommand;
use crate::daemon::DaemonClient;
use crate::daemon::auto_start::{daemon_pid, is_daemon_running};
use crate::error::Result;
use crate::services::global_config as global_config_service;

/// Handle daemon commands
pub async fn daemon(command: DaemonCommand) -> Result<()> {
    match command {
        DaemonCommand::Status => daemon_status().await,
        DaemonCommand::Start => daemon_start().await,
        DaemonCommand::Stop => daemon_stop().await,
        DaemonCommand::Restart => daemon_restart().await,
        DaemonCommand::Logs { follow, lines } => daemon_logs(follow, lines).await,
    }
}

/// Show daemon status
async fn daemon_status() -> Result<()> {
    if is_daemon_running().await {
        let mut client = DaemonClient::connect().await?;
        let version = client.ping().await?;

        let pid = daemon_pid().unwrap_or(0);
        println!("Daemon status: running");
        println!("  PID: {}", pid);
        println!("  Version: {}", version);
        #[cfg(unix)]
        {
            let socket_path = global_config_service::daemon_socket_path()?;
            println!("  Socket: {}", socket_path.display());
        }
        #[cfg(windows)]
        {
            let pipe_name = global_config_service::daemon_pipe_name();
            println!("  Pipe: {}", pipe_name);
        }
    } else {
        println!("Daemon status: not running");
        println!("  Run 'granary daemon start' or any worker command to start it.");
    }

    Ok(())
}

/// Start the daemon manually
async fn daemon_start() -> Result<()> {
    if is_daemon_running().await {
        println!("Daemon is already running.");
        return Ok(());
    }

    // Use ensure_daemon which handles spawning and waiting for connection
    match crate::daemon::ensure_daemon().await {
        Ok(mut client) => {
            let version = client.ping().await.unwrap_or_default();
            println!("Daemon started successfully.");
            println!("  Version: {}", version);
            if let Some(pid) = daemon_pid() {
                println!("  PID: {}", pid);
            }
            Ok(())
        }
        Err(e) => {
            println!("Failed to start daemon: {}", e);
            let log_path = global_config_service::daemon_log_path()?;
            println!("Check logs at: {}", log_path.display());
            Err(e)
        }
    }
}

/// Stop the daemon
async fn daemon_stop() -> Result<()> {
    if !is_daemon_running().await {
        println!("Daemon is not running.");
        return Ok(());
    }

    let mut client = DaemonClient::connect().await?;
    client.shutdown().await?;

    // Wait for shutdown
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if !is_daemon_running().await {
            println!("Daemon stopped.");
            return Ok(());
        }
    }

    println!("Warning: Daemon may still be shutting down.");
    Ok(())
}

/// Restart the daemon
async fn daemon_restart() -> Result<()> {
    if is_daemon_running().await {
        println!("Stopping daemon...");
        daemon_stop().await?;
        // Give a bit more time for full shutdown
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    println!("Starting daemon...");
    daemon_start().await
}

/// Show daemon logs
async fn daemon_logs(follow: bool, lines: usize) -> Result<()> {
    let log_path = global_config_service::daemon_log_path()?;

    if !log_path.exists() {
        println!("No daemon logs found at: {}", log_path.display());
        println!("The daemon may not have been started yet.");
        return Ok(());
    }

    if follow {
        crate::cli::worker::follow_log(&log_path, lines).await
    } else {
        crate::cli::worker::print_log_tail(&log_path, lines)
    }
}

/// Helper to convert PathBuf to displayable path
#[allow(dead_code)]
fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_path() {
        use std::path::PathBuf;
        let path = PathBuf::from("/home/user/.granary/daemon/daemon.log");
        assert_eq!(display_path(&path), "/home/user/.granary/daemon/daemon.log");
    }
}
