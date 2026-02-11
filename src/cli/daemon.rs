//! Daemon CLI commands for managing the granary daemon process.
//!
//! The daemon is a long-running background process that manages workers and runs.
//! These commands allow users to check its status, start/stop it manually,
//! and view its logs.

use serde::Serialize;

use crate::cli::args::{CliOutputFormat, DaemonCommand};
use crate::daemon::DaemonClient;
use crate::daemon::auto_start::{daemon_pid, is_daemon_running};
use crate::error::Result;
use crate::output::Output;
use crate::services::global_config as global_config_service;

// =============================================================================
// Output Types
// =============================================================================

/// Output for daemon status
pub struct DaemonStatusOutput {
    pub running: bool,
    pub pid: Option<u32>,
    pub version: Option<String>,
    pub endpoint: Option<String>,
}

#[derive(Serialize)]
struct DaemonStatusJson {
    running: bool,
    pid: Option<u32>,
    version: Option<String>,
    endpoint: Option<String>,
}

impl Output for DaemonStatusOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&DaemonStatusJson {
            running: self.running,
            pid: self.pid,
            version: self.version.clone(),
            endpoint: self.endpoint.clone(),
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        if self.running {
            let mut parts = vec![format!("Daemon: running")];
            if let Some(pid) = self.pid {
                parts.push(format!("PID: {}", pid));
            }
            if let Some(ref version) = self.version {
                parts.push(format!("Version: {}", version));
            }
            if let Some(ref endpoint) = self.endpoint {
                parts.push(format!("Endpoint: {}", endpoint));
            }
            parts.join("\n")
        } else {
            "Daemon: not running".to_string()
        }
    }

    fn to_text(&self) -> String {
        if self.running {
            let mut lines = vec!["Daemon status: running".to_string()];
            if let Some(pid) = self.pid {
                lines.push(format!("  PID: {}", pid));
            }
            if let Some(ref version) = self.version {
                lines.push(format!("  Version: {}", version));
            }
            if let Some(ref endpoint) = self.endpoint {
                lines.push(format!("  {}", endpoint));
            }
            lines.join("\n")
        } else {
            "Daemon status: not running\n  Run 'granary daemon start' or any worker command to start it.".to_string()
        }
    }
}

/// Output for daemon start
pub struct DaemonStartOutput {
    pub success: bool,
    pub version: Option<String>,
    pub pid: Option<u32>,
    pub error: Option<String>,
}

#[derive(Serialize)]
struct DaemonStartJson {
    success: bool,
    version: Option<String>,
    pid: Option<u32>,
    error: Option<String>,
}

impl Output for DaemonStartOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&DaemonStartJson {
            success: self.success,
            version: self.version.clone(),
            pid: self.pid,
            error: self.error.clone(),
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        if self.success {
            let mut parts = vec!["Daemon started".to_string()];
            if let Some(ref version) = self.version {
                parts.push(format!("Version: {}", version));
            }
            if let Some(pid) = self.pid {
                parts.push(format!("PID: {}", pid));
            }
            parts.join("\n")
        } else {
            let msg = self.error.as_deref().unwrap_or("unknown error");
            format!("Daemon start failed: {}", msg)
        }
    }

    fn to_text(&self) -> String {
        if self.success {
            let mut lines = vec!["Daemon started successfully.".to_string()];
            if let Some(ref version) = self.version {
                lines.push(format!("  Version: {}", version));
            }
            if let Some(pid) = self.pid {
                lines.push(format!("  PID: {}", pid));
            }
            lines.join("\n")
        } else {
            let msg = self.error.as_deref().unwrap_or("unknown error");
            format!("Failed to start daemon: {}", msg)
        }
    }
}

/// Output for daemon stop
pub struct DaemonStopOutput {
    pub stopped: bool,
    pub warning: Option<String>,
}

#[derive(Serialize)]
struct DaemonStopJson {
    stopped: bool,
    warning: Option<String>,
}

impl Output for DaemonStopOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&DaemonStopJson {
            stopped: self.stopped,
            warning: self.warning.clone(),
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        if self.stopped {
            "Daemon stopped".to_string()
        } else if let Some(ref warning) = self.warning {
            format!("Daemon stop: {}", warning)
        } else {
            "Daemon is not running".to_string()
        }
    }

    fn to_text(&self) -> String {
        if self.stopped {
            "Daemon stopped.".to_string()
        } else if let Some(ref warning) = self.warning {
            format!("Warning: {}", warning)
        } else {
            "Daemon is not running.".to_string()
        }
    }
}

/// Output for daemon restart
pub struct DaemonRestartOutput {
    pub stop: DaemonStopOutput,
    pub start: DaemonStartOutput,
}

impl Output for DaemonRestartOutput {
    fn to_json(&self) -> String {
        format!(
            "{{\"stop\":{},\"start\":{}}}",
            self.stop.to_json(),
            self.start.to_json()
        )
    }

    fn to_prompt(&self) -> String {
        format!("{}\n{}", self.stop.to_prompt(), self.start.to_prompt())
    }

    fn to_text(&self) -> String {
        let mut lines = Vec::new();
        // Only show stop info if daemon was previously running
        if self.stop.stopped {
            lines.push("Stopping daemon...".to_string());
            lines.push(self.stop.to_text());
        }
        lines.push("Starting daemon...".to_string());
        lines.push(self.start.to_text());
        lines.join("\n")
    }
}

/// Output for daemon logs (non-follow mode only)
pub struct DaemonLogsOutput {
    pub logs: String,
    pub log_path: String,
}

#[derive(Serialize)]
struct DaemonLogsJson {
    logs: String,
    log_path: String,
}

impl Output for DaemonLogsOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&DaemonLogsJson {
            logs: self.logs.clone(),
            log_path: self.log_path.clone(),
        })
        .unwrap_or_default()
    }

    fn to_prompt(&self) -> String {
        self.logs.clone()
    }

    fn to_text(&self) -> String {
        self.logs.clone()
    }
}

// =============================================================================
// Command Handlers
// =============================================================================

/// Handle daemon commands
pub async fn daemon(command: DaemonCommand, cli_format: Option<CliOutputFormat>) -> Result<()> {
    match command {
        DaemonCommand::Status => daemon_status(cli_format).await,
        DaemonCommand::Start => daemon_start(cli_format).await,
        DaemonCommand::Stop => daemon_stop(cli_format).await,
        DaemonCommand::Restart => daemon_restart(cli_format).await,
        DaemonCommand::Logs { follow, lines } => daemon_logs(follow, lines, cli_format).await,
    }
}

/// Show daemon status
async fn daemon_status(cli_format: Option<CliOutputFormat>) -> Result<()> {
    let output = if is_daemon_running().await {
        let mut client = DaemonClient::connect().await?;
        let version = client.ping().await?;
        let pid = daemon_pid().unwrap_or(0);

        let endpoint = get_daemon_endpoint();

        DaemonStatusOutput {
            running: true,
            pid: Some(pid),
            version: Some(version),
            endpoint,
        }
    } else {
        DaemonStatusOutput {
            running: false,
            pid: None,
            version: None,
            endpoint: None,
        }
    };

    println!("{}", output.format(cli_format));
    Ok(())
}

/// Start the daemon manually
async fn daemon_start(cli_format: Option<CliOutputFormat>) -> Result<()> {
    if is_daemon_running().await {
        let output = DaemonStartOutput {
            success: true,
            version: None,
            pid: daemon_pid(),
            error: Some("Daemon is already running.".to_string()),
        };
        println!("{}", output.format(cli_format));
        return Ok(());
    }

    match crate::daemon::ensure_daemon().await {
        Ok(mut client) => {
            let version = client.ping().await.unwrap_or_default();
            let output = DaemonStartOutput {
                success: true,
                version: Some(version),
                pid: daemon_pid(),
                error: None,
            };
            println!("{}", output.format(cli_format));
            Ok(())
        }
        Err(e) => {
            let log_path = global_config_service::daemon_log_path()?;
            let output = DaemonStartOutput {
                success: false,
                version: None,
                pid: None,
                error: Some(format!("{}\nCheck logs at: {}", e, log_path.display())),
            };
            println!("{}", output.format(cli_format));
            Err(e)
        }
    }
}

/// Stop the daemon
async fn daemon_stop(cli_format: Option<CliOutputFormat>) -> Result<()> {
    if !is_daemon_running().await {
        let output = DaemonStopOutput {
            stopped: false,
            warning: None,
        };
        println!("{}", output.format(cli_format));
        return Ok(());
    }

    let mut client = DaemonClient::connect().await?;
    client.shutdown().await?;

    // Wait for shutdown
    for _ in 0..50 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if !is_daemon_running().await {
            let output = DaemonStopOutput {
                stopped: true,
                warning: None,
            };
            println!("{}", output.format(cli_format));
            return Ok(());
        }
    }

    let output = DaemonStopOutput {
        stopped: false,
        warning: Some("Daemon may still be shutting down.".to_string()),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

/// Restart the daemon
async fn daemon_restart(cli_format: Option<CliOutputFormat>) -> Result<()> {
    let was_running = is_daemon_running().await;

    let stop_output = if was_running {
        // Print progress for text mode
        if cli_format.is_none() {
            println!("Stopping daemon...");
        }
        let mut client = DaemonClient::connect().await?;
        client.shutdown().await?;

        let mut stopped = false;
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if !is_daemon_running().await {
                stopped = true;
                break;
            }
        }

        // Give a bit more time for full shutdown
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        DaemonStopOutput {
            stopped,
            warning: if stopped {
                None
            } else {
                Some("Daemon may still be shutting down.".to_string())
            },
        }
    } else {
        DaemonStopOutput {
            stopped: false,
            warning: None,
        }
    };

    if cli_format.is_none() {
        println!("Starting daemon...");
    }

    let start_output = match crate::daemon::ensure_daemon().await {
        Ok(mut client) => {
            let version = client.ping().await.unwrap_or_default();
            DaemonStartOutput {
                success: true,
                version: Some(version),
                pid: daemon_pid(),
                error: None,
            }
        }
        Err(e) => {
            let log_path = global_config_service::daemon_log_path()?;
            let start_output = DaemonStartOutput {
                success: false,
                version: None,
                pid: None,
                error: Some(format!("{}\nCheck logs at: {}", e, log_path.display())),
            };
            let output = DaemonRestartOutput {
                stop: stop_output,
                start: start_output,
            };
            // For structured formats, output the combined result
            if cli_format.is_some() {
                println!("{}", output.format(cli_format));
            } else {
                println!("{}", output.start.to_text());
            }
            return Err(e);
        }
    };

    let output = DaemonRestartOutput {
        stop: stop_output,
        start: start_output,
    };

    if cli_format.is_some() {
        // For structured formats, output the combined result
        println!("{}", output.format(cli_format));
    } else {
        // For text mode, just print the start result (stop/start progress already printed)
        println!("{}", output.start.to_text());
    }

    Ok(())
}

/// Show daemon logs
async fn daemon_logs(
    follow: bool,
    lines: usize,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let log_path = global_config_service::daemon_log_path()?;

    if !log_path.exists() {
        let output = DaemonLogsOutput {
            logs: String::new(),
            log_path: log_path.display().to_string(),
        };
        if cli_format.is_some() {
            println!("{}", output.format(cli_format));
        } else {
            println!("No daemon logs found at: {}", log_path.display());
            println!("The daemon may not have been started yet.");
        }
        return Ok(());
    }

    // Follow mode is inherently streaming - print as you go
    if follow {
        return crate::cli::worker::follow_log(&log_path, lines).await;
    }

    // Non-follow mode: read log tail and format via Output
    let log_content = read_log_tail(&log_path, lines);
    let output = DaemonLogsOutput {
        logs: log_content,
        log_path: log_path.display().to_string(),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

// =============================================================================
// Helpers
// =============================================================================

/// Get the daemon endpoint (socket path on Unix, pipe name on Windows)
fn get_daemon_endpoint() -> Option<String> {
    #[cfg(unix)]
    {
        global_config_service::daemon_socket_path()
            .ok()
            .map(|p| format!("Socket: {}", p.display()))
    }
    #[cfg(windows)]
    {
        Some(format!(
            "Pipe: {}",
            global_config_service::daemon_pipe_name()
        ))
    }
}

/// Read the last N lines of a log file as a string
fn read_log_tail(path: &std::path::Path, lines: usize) -> String {
    use std::io::{BufRead, BufReader};

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return String::new(),
    };
    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader.lines().map_while(|l| l.ok()).collect();
    let start = all_lines.len().saturating_sub(lines);
    all_lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_output_running_text() {
        let output = DaemonStatusOutput {
            running: true,
            pid: Some(1234),
            version: Some("0.1.0".to_string()),
            endpoint: Some("Socket: /tmp/granary.sock".to_string()),
        };
        let text = output.to_text();
        assert!(text.contains("running"));
        assert!(text.contains("1234"));
        assert!(text.contains("0.1.0"));
        assert!(text.contains("Socket:"));
    }

    #[test]
    fn test_status_output_not_running_text() {
        let output = DaemonStatusOutput {
            running: false,
            pid: None,
            version: None,
            endpoint: None,
        };
        let text = output.to_text();
        assert!(text.contains("not running"));
    }

    #[test]
    fn test_status_output_json() {
        let output = DaemonStatusOutput {
            running: true,
            pid: Some(1234),
            version: Some("0.1.0".to_string()),
            endpoint: Some("Socket: /tmp/granary.sock".to_string()),
        };
        let json = output.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["running"], true);
        assert_eq!(parsed["pid"], 1234);
        assert_eq!(parsed["version"], "0.1.0");
    }

    #[test]
    fn test_start_output_success_json() {
        let output = DaemonStartOutput {
            success: true,
            version: Some("0.1.0".to_string()),
            pid: Some(5678),
            error: None,
        };
        let json = output.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["pid"], 5678);
    }

    #[test]
    fn test_start_output_failure_text() {
        let output = DaemonStartOutput {
            success: false,
            version: None,
            pid: None,
            error: Some("connection refused".to_string()),
        };
        let text = output.to_text();
        assert!(text.contains("Failed to start daemon"));
        assert!(text.contains("connection refused"));
    }

    #[test]
    fn test_stop_output_stopped_json() {
        let output = DaemonStopOutput {
            stopped: true,
            warning: None,
        };
        let json = output.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["stopped"], true);
    }

    #[test]
    fn test_stop_output_warning_text() {
        let output = DaemonStopOutput {
            stopped: false,
            warning: Some("Daemon may still be shutting down.".to_string()),
        };
        let text = output.to_text();
        assert!(text.contains("Warning:"));
    }

    #[test]
    fn test_restart_output_json() {
        let output = DaemonRestartOutput {
            stop: DaemonStopOutput {
                stopped: true,
                warning: None,
            },
            start: DaemonStartOutput {
                success: true,
                version: Some("0.1.0".to_string()),
                pid: Some(9999),
                error: None,
            },
        };
        let json = output.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["stop"]["stopped"], true);
        assert_eq!(parsed["start"]["success"], true);
    }

    #[test]
    fn test_logs_output_json() {
        let output = DaemonLogsOutput {
            logs: "line 1\nline 2".to_string(),
            log_path: "/tmp/daemon.log".to_string(),
        };
        let json = output.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed["logs"].as_str().unwrap().contains("line 1"));
        assert_eq!(parsed["log_path"], "/tmp/daemon.log");
    }

    #[test]
    fn test_logs_output_text() {
        let output = DaemonLogsOutput {
            logs: "line 1\nline 2".to_string(),
            log_path: "/tmp/daemon.log".to_string(),
        };
        let text = output.to_text();
        assert_eq!(text, "line 1\nline 2");
    }

    #[test]
    fn test_status_output_prompt() {
        let output = DaemonStatusOutput {
            running: true,
            pid: Some(1234),
            version: Some("0.1.0".to_string()),
            endpoint: Some("Socket: /tmp/granary.sock".to_string()),
        };
        let prompt = output.to_prompt();
        assert!(prompt.contains("Daemon: running"));
        assert!(prompt.contains("PID: 1234"));
    }
}
