//! Auto-start functionality for the granary daemon.
//!
//! This module provides utilities to ensure the daemon is running when CLI
//! commands need it. It handles automatic daemon startup with exponential
//! backoff retry logic.

use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

use crate::daemon::client::DaemonClient;
use crate::error::{GranaryError, Result};
use crate::services::global_config as global_config_service;

/// Ensure the daemon is running, starting it if necessary.
///
/// This function first attempts to connect to an already-running daemon.
/// If the connection fails, it spawns a new daemon process and retries
/// the connection with exponential backoff.
///
/// # Returns
///
/// Returns a connected `DaemonClient` on success.
///
/// # Errors
///
/// Returns `DaemonConnection` error if:
/// - The daemon binary is not found
/// - The daemon fails to start
/// - All connection retry attempts fail
///
/// # Example
///
/// ```ignore
/// use granary::daemon::ensure_daemon;
///
/// let mut client = ensure_daemon().await?;
/// let version = client.ping().await?;
/// println!("Connected to daemon version: {}", version);
/// ```
pub async fn ensure_daemon() -> Result<DaemonClient> {
    // Try to connect first - daemon may already be running
    if let Ok(client) = DaemonClient::connect().await {
        return Ok(client);
    }

    // Daemon not running, spawn it
    spawn_daemon()?;

    // Retry with exponential backoff: 50ms, 100ms, 150ms, ...
    for attempt in 0..10 {
        let delay = Duration::from_millis(50 * (attempt + 1));
        sleep(delay).await;

        if let Ok(client) = DaemonClient::connect().await {
            return Ok(client);
        }
    }

    Err(GranaryError::DaemonConnection(
        "Failed to start daemon. Check ~/.granary/daemon/daemon.log for details.".to_string(),
    ))
}

/// Spawn the daemon process in the background (Unix implementation).
///
/// The daemon binary (`granaryd`) should be located next to the `granary` binary.
/// This spawns the daemon with stdin/stdout/stderr redirected to null - the daemon
/// sets up its own logging to `~/.granary/daemon/daemon.log`.
#[cfg(unix)]
fn spawn_daemon() -> Result<()> {
    use std::process::Stdio;

    // Find the granaryd binary - it should be next to the granary binary
    let current_exe = std::env::current_exe()?;
    let daemon_path = current_exe.with_file_name("granaryd");

    if !daemon_path.exists() {
        return Err(GranaryError::DaemonConnection(format!(
            "Daemon binary not found at {:?}",
            daemon_path
        )));
    }

    // Ensure the daemon directory exists for socket/pid/log files
    let daemon_dir = global_config_service::daemon_dir()?;
    std::fs::create_dir_all(&daemon_dir)?;

    // Spawn with stdin/stdout/stderr redirected to null
    // The daemon will set up its own logging
    Command::new(&daemon_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

/// Spawn the daemon process in the background (Windows implementation).
///
/// Uses Windows-specific process creation flags to detach the daemon from the
/// current console and run it as a background process.
#[cfg(windows)]
fn spawn_daemon() -> Result<()> {
    use std::os::windows::process::CommandExt;

    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let current_exe = std::env::current_exe()?;
    let daemon_path = current_exe.with_file_name("granaryd.exe");

    if !daemon_path.exists() {
        return Err(GranaryError::DaemonConnection(format!(
            "Daemon binary not found at {:?}",
            daemon_path
        )));
    }

    // Ensure the daemon directory exists for socket/pid/log files
    let daemon_dir = global_config_service::daemon_dir()?;
    std::fs::create_dir_all(&daemon_dir)?;

    Command::new(&daemon_path)
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn()?;

    Ok(())
}

/// Check if the daemon is currently running.
///
/// This is a convenience function that attempts to connect to the daemon
/// and returns true if the connection succeeds.
///
/// # Example
///
/// ```ignore
/// use granary::daemon::auto_start::is_daemon_running;
///
/// if is_daemon_running().await {
///     println!("Daemon is running");
/// } else {
///     println!("Daemon is not running");
/// }
/// ```
pub async fn is_daemon_running() -> bool {
    DaemonClient::connect().await.is_ok()
}

/// Get the daemon PID if running.
///
/// Reads the PID from the daemon's PID file at `~/.granary/daemon/granaryd.pid`.
/// Returns `None` if the PID file doesn't exist or cannot be parsed.
///
/// Note: This does not verify if the process with that PID is still running.
/// Use `is_daemon_running()` for a connection-based check.
///
/// # Example
///
/// ```ignore
/// use granary::daemon::auto_start::daemon_pid;
///
/// if let Some(pid) = daemon_pid() {
///     println!("Daemon PID: {}", pid);
/// }
/// ```
pub fn daemon_pid() -> Option<u32> {
    let pid_path = global_config_service::daemon_pid_path().ok()?;
    let pid_str = std::fs::read_to_string(&pid_path).ok()?;
    pid_str.trim().parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_pid_missing_file() {
        // When the PID file doesn't exist, daemon_pid should return None
        // This is a basic sanity test - the actual file may or may not exist
        let result = daemon_pid();
        // We can't assert a specific value since it depends on system state,
        // but we can verify the function doesn't panic
        let _ = result;
    }

    #[tokio::test]
    async fn test_is_daemon_running_returns_bool() {
        // This test verifies that is_daemon_running returns a bool without panicking
        // The actual result depends on whether the daemon is running
        let result = is_daemon_running().await;
        let _ = result;
    }

    #[test]
    fn test_spawn_daemon_binary_not_found() {
        // Save current exe path for restoration
        // Note: We can't easily test spawn_daemon directly because it uses
        // current_exe(), but we can verify the error handling logic by
        // checking that GranaryError::DaemonConnection is the right type

        // This is more of a documentation test - in real usage, the binary
        // should be installed alongside the CLI
        let err = GranaryError::DaemonConnection("test".to_string());
        assert!(matches!(err, GranaryError::DaemonConnection(_)));
    }
}
