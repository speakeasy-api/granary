//! DaemonClient for CLI-to-daemon communication.
//!
//! This module provides a client library that CLI commands use to communicate
//! with the daemon process. On Unix, this uses Unix domain sockets. On Windows,
//! this uses named pipes. It handles request/response serialization and error
//! handling.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[cfg(unix)]
use tokio::net::UnixStream;

#[cfg(windows)]
use tokio::net::windows::named_pipe::NamedPipeClient;

use crate::daemon::protocol::{
    AuthRequest, LogTarget, LogsRequest, LogsResponse, Operation, Request, Response,
    StartWorkerRequest, read_frame, write_frame,
};
use crate::error::{GranaryError, Result};
use crate::models::Worker;
use crate::models::run::Run;
use crate::services::global_config as global_config_service;

/// Client for communicating with the granary daemon.
///
/// The DaemonClient connects to the daemon via Unix socket (on Unix) or named
/// pipe (on Windows) and provides typed methods for each operation. It handles
/// request/response serialization and error handling.
///
/// # Example
///
/// ```ignore
/// use granary::daemon::client::DaemonClient;
///
/// let mut client = DaemonClient::connect().await?;
/// let version = client.ping().await?;
/// println!("Daemon version: {}", version);
/// ```
#[cfg(unix)]
pub struct DaemonClient {
    stream: UnixStream,
    request_id: AtomicU64,
}

#[cfg(windows)]
pub struct DaemonClient {
    pipe: NamedPipeClient,
    request_id: AtomicU64,
}

impl DaemonClient {
    /// Connect to the daemon.
    ///
    /// On Unix, this establishes a connection to the daemon's Unix domain socket at
    /// `~/.granary/daemon/granaryd.sock`.
    ///
    /// On Windows, this connects to the daemon's named pipe at
    /// `\\.\pipe\granaryd-{username}`.
    ///
    /// After establishing the connection, the client automatically authenticates
    /// using the auth token stored at `~/.granary/daemon/auth.token`.
    ///
    /// # Errors
    ///
    /// Returns `DaemonConnection` error if the daemon is not running or the
    /// socket/pipe cannot be connected to.
    /// Returns `DaemonError` if authentication fails.
    #[cfg(unix)]
    pub async fn connect() -> Result<Self> {
        let socket_path = global_config_service::daemon_socket_path()?;

        let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
            GranaryError::DaemonConnection(format!(
                "Failed to connect to daemon at {:?}: {}",
                socket_path, e
            ))
        })?;

        let mut client = Self {
            stream,
            request_id: AtomicU64::new(1),
        };

        // Authenticate with the daemon
        client.authenticate().await?;

        Ok(client)
    }

    /// Create a DaemonClient from an existing Unix stream.
    ///
    /// This is useful for testing where you want to connect to a daemon
    /// at a custom socket path rather than the default global socket.
    ///
    /// **Note:** This method does NOT authenticate automatically. The caller
    /// must call `authenticate()` manually after creating the client.
    ///
    /// # Arguments
    ///
    /// * `stream` - An already-connected UnixStream
    #[cfg(unix)]
    pub fn from_stream(stream: UnixStream) -> Self {
        Self {
            stream,
            request_id: AtomicU64::new(1),
        }
    }

    /// Authenticate with the daemon using the stored auth token.
    ///
    /// This is called automatically by `connect()`. You only need to call
    /// this manually if you created the client using `from_stream()`.
    ///
    /// # Errors
    ///
    /// Returns `DaemonError` if authentication fails (invalid token or
    /// daemon rejects the auth request).
    pub async fn authenticate(&mut self) -> Result<()> {
        let token = global_config_service::get_or_create_auth_token()?;
        self.authenticate_with_token(&token).await
    }

    /// Authenticate with the daemon using a provided token.
    ///
    /// This is useful for testing or custom authentication scenarios where
    /// the token is stored in a non-standard location.
    ///
    /// # Arguments
    ///
    /// * `token` - The authentication token to use
    ///
    /// # Errors
    ///
    /// Returns `DaemonError` if authentication fails.
    pub async fn authenticate_with_token(&mut self, token: &str) -> Result<()> {
        let response = self
            .request(Operation::Auth(AuthRequest {
                token: token.to_string(),
            }))
            .await?;

        if response.ok {
            Ok(())
        } else {
            Err(GranaryError::DaemonError(
                response
                    .error
                    .unwrap_or_else(|| "Authentication failed".to_string()),
            ))
        }
    }

    /// Connect to the daemon via named pipe (Windows).
    ///
    /// This establishes a connection to the daemon's named pipe.
    /// If the pipe is busy (all instances in use), this will retry with
    /// a short delay.
    ///
    /// After establishing the connection, the client automatically authenticates
    /// using the auth token stored at `~/.granary/daemon/auth.token`.
    ///
    /// # Errors
    ///
    /// Returns `DaemonConnection` error if the daemon is not running or the
    /// pipe cannot be connected to.
    /// Returns `DaemonError` if authentication fails.
    #[cfg(windows)]
    pub async fn connect() -> Result<Self> {
        use tokio::net::windows::named_pipe::ClientOptions;
        use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;

        let pipe_name = global_config_service::daemon_pipe_name();

        // Retry loop for handling busy pipes
        let pipe = loop {
            match ClientOptions::new().open(&pipe_name) {
                Ok(pipe) => break pipe,
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) => {
                    // Pipe is busy (all instances in use), wait and retry
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) => {
                    return Err(GranaryError::DaemonConnection(format!(
                        "Failed to connect to daemon at {}: {}",
                        pipe_name, e
                    )));
                }
            }
        };

        let mut client = Self {
            pipe,
            request_id: AtomicU64::new(1),
        };

        // Authenticate with the daemon
        client.authenticate().await?;

        Ok(client)
    }

    /// Send a request and wait for response.
    ///
    /// This is the core method that handles the request/response cycle:
    /// 1. Assigns a unique request ID
    /// 2. Serializes and sends the request
    /// 3. Reads and deserializes the response
    /// 4. Validates the response ID matches
    #[cfg(unix)]
    async fn request(&mut self, op: Operation) -> Result<Response> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = Request { id, op };

        // Send request
        let data = serde_json::to_vec(&request)?;
        write_frame(&mut self.stream, &data)
            .await
            .map_err(|e| GranaryError::DaemonProtocol(format!("Failed to send request: {}", e)))?;

        // Read response
        let response_data = read_frame(&mut self.stream)
            .await
            .map_err(|e| GranaryError::DaemonProtocol(format!("Failed to read response: {}", e)))?;
        let response: Response = serde_json::from_slice(&response_data)?;

        if response.id != id {
            return Err(GranaryError::DaemonProtocol(format!(
                "Response ID mismatch: expected {}, got {}",
                id, response.id
            )));
        }

        Ok(response)
    }

    /// Send a request and wait for response (Windows).
    #[cfg(windows)]
    async fn request(&mut self, op: Operation) -> Result<Response> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = Request { id, op };

        // Send request
        let data = serde_json::to_vec(&request)?;
        write_frame(&mut self.pipe, &data)
            .await
            .map_err(|e| GranaryError::DaemonProtocol(format!("Failed to send request: {}", e)))?;

        // Read response
        let response_data = read_frame(&mut self.pipe)
            .await
            .map_err(|e| GranaryError::DaemonProtocol(format!("Failed to read response: {}", e)))?;
        let response: Response = serde_json::from_slice(&response_data)?;

        if response.id != id {
            return Err(GranaryError::DaemonProtocol(format!(
                "Response ID mismatch: expected {}, got {}",
                id, response.id
            )));
        }

        Ok(response)
    }

    /// Ping the daemon to check if it is running.
    ///
    /// Returns the daemon version string on success.
    pub async fn ping(&mut self) -> Result<String> {
        let response = self.request(Operation::Ping).await?;
        if response.ok {
            let version = response
                .body
                .and_then(|v| v.get("version").and_then(|v| v.as_str()).map(String::from))
                .unwrap_or_default();
            Ok(version)
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Request daemon shutdown.
    ///
    /// This gracefully shuts down the daemon process.
    pub async fn shutdown(&mut self) -> Result<()> {
        let response = self.request(Operation::Shutdown).await?;
        if response.ok {
            Ok(())
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Start a new worker.
    ///
    /// Creates and starts a new worker with the given configuration.
    /// Returns the created Worker on success.
    pub async fn start_worker(&mut self, req: StartWorkerRequest) -> Result<Worker> {
        let response = self.request(Operation::StartWorker(req)).await?;
        if response.ok {
            let worker: Worker =
                serde_json::from_value(response.body.ok_or_else(|| {
                    GranaryError::DaemonProtocol("Missing response body".into())
                })?)?;
            Ok(worker)
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Stop a worker.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - The ID of the worker to stop
    /// * `stop_runs` - Whether to also stop any running runs
    pub async fn stop_worker(&mut self, worker_id: &str, stop_runs: bool) -> Result<()> {
        let response = self
            .request(Operation::StopWorker {
                worker_id: worker_id.to_string(),
                stop_runs,
            })
            .await?;
        if response.ok {
            Ok(())
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Get a worker by ID.
    ///
    /// Returns the Worker with the given ID.
    pub async fn get_worker(&mut self, worker_id: &str) -> Result<Worker> {
        let response = self
            .request(Operation::GetWorker {
                worker_id: worker_id.to_string(),
            })
            .await?;
        if response.ok {
            let worker: Worker =
                serde_json::from_value(response.body.ok_or_else(|| {
                    GranaryError::DaemonProtocol("Missing response body".into())
                })?)?;
            Ok(worker)
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// List all workers.
    ///
    /// # Arguments
    ///
    /// * `all` - If true, include stopped workers; otherwise only running workers
    pub async fn list_workers(&mut self, all: bool) -> Result<Vec<Worker>> {
        let response = self.request(Operation::ListWorkers { all }).await?;
        if response.ok {
            let workers: Vec<Worker> =
                serde_json::from_value(response.body.ok_or_else(|| {
                    GranaryError::DaemonProtocol("Missing response body".into())
                })?)?;
            Ok(workers)
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Prune stopped workers.
    ///
    /// Removes all workers that have stopped from the database.
    /// Returns the number of workers pruned.
    pub async fn prune_workers(&mut self) -> Result<i32> {
        let response = self.request(Operation::PruneWorkers).await?;
        if response.ok {
            let pruned = response
                .body
                .and_then(|v| v.get("pruned").and_then(|v| v.as_i64()))
                .unwrap_or(0) as i32;
            Ok(pruned)
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Get worker logs.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - The ID of the worker
    /// * `follow` - If true, returns LogsResponse format with has_more indicator for streaming
    /// * `lines` - Number of lines to show (from the end)
    ///
    /// When `follow=true`, the returned string contains the initial log lines and
    /// the response indicates if more logs may come (worker is still active).
    /// Use `get_logs()` or `follow_logs()` for proper streaming support.
    pub async fn worker_logs(
        &mut self,
        worker_id: &str,
        follow: bool,
        lines: i32,
    ) -> Result<String> {
        let response = self
            .request(Operation::WorkerLogs {
                worker_id: worker_id.to_string(),
                follow,
                lines,
            })
            .await?;
        if response.ok {
            if follow {
                // Follow mode returns LogsResponse format
                let logs_response: LogsResponse =
                    serde_json::from_value(response.body.ok_or_else(|| {
                        GranaryError::DaemonProtocol("Missing response body".into())
                    })?)?;
                Ok(logs_response.lines.join("\n"))
            } else {
                // Non-follow mode returns simple { logs: "..." } format
                let logs = response
                    .body
                    .and_then(|v| v.get("logs").and_then(|v| v.as_str()).map(String::from))
                    .unwrap_or_default();
                Ok(logs)
            }
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    // Run management methods

    /// Get a run by ID.
    ///
    /// Returns the Run with the given ID.
    pub async fn get_run(&mut self, run_id: &str) -> Result<Run> {
        let response = self
            .request(Operation::GetRun {
                run_id: run_id.to_string(),
            })
            .await?;
        if response.ok {
            let run: Run =
                serde_json::from_value(response.body.ok_or_else(|| {
                    GranaryError::DaemonProtocol("Missing response body".into())
                })?)?;
            Ok(run)
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// List runs.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - Optional filter by worker ID
    /// * `status` - Optional filter by status (e.g., "running", "completed")
    /// * `all` - If true, include completed/failed runs; otherwise only active runs
    pub async fn list_runs(
        &mut self,
        worker_id: Option<&str>,
        status: Option<&str>,
        all: bool,
    ) -> Result<Vec<Run>> {
        let response = self
            .request(Operation::ListRuns {
                worker_id: worker_id.map(String::from),
                status: status.map(String::from),
                all,
            })
            .await?;
        if response.ok {
            let runs: Vec<Run> =
                serde_json::from_value(response.body.ok_or_else(|| {
                    GranaryError::DaemonProtocol("Missing response body".into())
                })?)?;
            Ok(runs)
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Stop a running run.
    ///
    /// Sends a termination signal to the run's process.
    pub async fn stop_run(&mut self, run_id: &str) -> Result<()> {
        let response = self
            .request(Operation::StopRun {
                run_id: run_id.to_string(),
            })
            .await?;
        if response.ok {
            Ok(())
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Pause a running run.
    ///
    /// Pauses the run's process (SIGSTOP on Unix).
    pub async fn pause_run(&mut self, run_id: &str) -> Result<()> {
        let response = self
            .request(Operation::PauseRun {
                run_id: run_id.to_string(),
            })
            .await?;
        if response.ok {
            Ok(())
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Resume a paused run.
    ///
    /// Resumes the run's process (SIGCONT on Unix).
    pub async fn resume_run(&mut self, run_id: &str) -> Result<()> {
        let response = self
            .request(Operation::ResumeRun {
                run_id: run_id.to_string(),
            })
            .await?;
        if response.ok {
            Ok(())
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Get run logs.
    ///
    /// # Arguments
    ///
    /// * `run_id` - The ID of the run
    /// * `follow` - If true, returns LogsResponse format with has_more indicator for streaming
    /// * `lines` - Number of lines to show (from the end)
    ///
    /// When `follow=true`, the returned string contains the initial log lines and
    /// the response indicates if more logs may come (run is still active).
    /// Use `get_logs()` or `follow_logs()` for proper streaming support.
    pub async fn run_logs(&mut self, run_id: &str, follow: bool, lines: i32) -> Result<String> {
        let response = self
            .request(Operation::RunLogs {
                run_id: run_id.to_string(),
                follow,
                lines,
            })
            .await?;
        if response.ok {
            if follow {
                // Follow mode returns LogsResponse format
                let logs_response: LogsResponse =
                    serde_json::from_value(response.body.ok_or_else(|| {
                        GranaryError::DaemonProtocol("Missing response body".into())
                    })?)?;
                Ok(logs_response.lines.join("\n"))
            } else {
                // Non-follow mode returns simple { logs: "..." } format
                let logs = response
                    .body
                    .and_then(|v| v.get("logs").and_then(|v| v.as_str()).map(String::from))
                    .unwrap_or_default();
                Ok(logs)
            }
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    // ========================================================================
    // Log streaming methods
    // ========================================================================

    /// Get logs with offset-based pagination for streaming support.
    ///
    /// This is the low-level method for log streaming. Use `follow_logs` for
    /// a higher-level streaming interface.
    ///
    /// # Arguments
    ///
    /// * `target_id` - Worker ID or Run ID
    /// * `target_type` - Whether this is a worker or run
    /// * `since_line` - Return lines after this line number
    /// * `limit` - Maximum lines to return
    pub async fn get_logs(
        &mut self,
        target_id: &str,
        target_type: LogTarget,
        since_line: u64,
        limit: u64,
    ) -> Result<LogsResponse> {
        let response = self
            .request(Operation::GetLogs(LogsRequest {
                target_id: target_id.to_string(),
                target_type,
                since_line,
                limit,
            }))
            .await?;

        if response.ok {
            let logs_response: LogsResponse =
                serde_json::from_value(response.body.ok_or_else(|| {
                    GranaryError::DaemonProtocol("Missing response body".into())
                })?)?;
            Ok(logs_response)
        } else {
            Err(GranaryError::DaemonError(
                response.error.unwrap_or_default(),
            ))
        }
    }

    /// Stream logs from a worker or run, calling the callback for each batch.
    ///
    /// This method polls the daemon repeatedly to stream log lines as they arrive.
    /// The callback is invoked with each batch of new lines. Return `false` from
    /// the callback to stop following.
    ///
    /// # Arguments
    ///
    /// * `target_id` - Worker ID or Run ID
    /// * `target_type` - Whether this is a worker or run
    /// * `initial_lines` - Number of initial lines to display (from the end of existing logs)
    /// * `callback` - Called with each batch of new lines. Return `false` to stop.
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.follow_logs("worker-123", LogTarget::Worker, 50, |lines| {
    ///     for line in lines {
    ///         println!("{}", line);
    ///     }
    ///     true // Continue following
    /// }).await?;
    /// ```
    pub async fn follow_logs<F>(
        &mut self,
        target_id: &str,
        target_type: LogTarget,
        initial_lines: u64,
        mut callback: F,
    ) -> Result<()>
    where
        F: FnMut(&[String]) -> bool,
    {
        // First, get total line count by requesting with high limit
        // This is a simple approach - could be optimized with a dedicated "count lines" operation
        let initial_response = self
            .get_logs(target_id, target_type.clone(), 0, u64::MAX)
            .await?;

        // Calculate starting position to show initial_lines from the end
        let total_lines = initial_response.next_line;
        let mut since_line = total_lines.saturating_sub(initial_lines);

        // Display initial lines (if any)
        if since_line < total_lines {
            let response = self
                .get_logs(target_id, target_type.clone(), since_line, 1000)
                .await?;

            if !response.lines.is_empty() && !callback(&response.lines) {
                return Ok(());
            }
            since_line = response.next_line;
        }

        // Poll for new lines until target is no longer active or callback returns false
        loop {
            let response = self
                .get_logs(target_id, target_type.clone(), since_line, 100)
                .await?;

            if !response.lines.is_empty() && !callback(&response.lines) {
                break;
            }

            since_line = response.next_line;

            // If target is no longer active and no more lines, we're done
            if !response.has_more && response.lines.is_empty() {
                break;
            }

            // Sleep before polling again
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::io::{AsyncRead, AsyncWrite};

    // Mock stream for testing - kept for future integration tests
    #[allow(dead_code)]
    struct MockStream {
        read_data: Cursor<Vec<u8>>,
        write_data: Vec<u8>,
    }

    #[allow(dead_code)]
    impl MockStream {
        fn new(response_data: Vec<u8>) -> Self {
            Self {
                read_data: Cursor::new(response_data),
                write_data: Vec::new(),
            }
        }

        fn with_response(response: &Response) -> Self {
            let json = serde_json::to_vec(response).unwrap();
            let mut data = Vec::new();
            data.extend_from_slice(&(json.len() as u32).to_be_bytes());
            data.extend_from_slice(&json);
            Self::new(data)
        }
    }

    impl AsyncRead for MockStream {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::pin::Pin::new(&mut self.read_data).poll_read(cx, buf)
        }
    }

    impl AsyncWrite for MockStream {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            self.write_data.extend_from_slice(buf);
            std::task::Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn test_response_parsing_ok() {
        let response = Response::ok(1, serde_json::json!({"version": "0.1.0"}));
        let json = serde_json::to_string(&response).unwrap();
        let parsed: Response = serde_json::from_str(&json).unwrap();
        assert!(parsed.ok);
        assert_eq!(parsed.id, 1);
        assert!(parsed.error.is_none());
    }

    #[test]
    fn test_response_parsing_error() {
        let response = Response::err(2, "something went wrong");
        let json = serde_json::to_string(&response).unwrap();
        let parsed: Response = serde_json::from_str(&json).unwrap();
        assert!(!parsed.ok);
        assert_eq!(parsed.id, 2);
        assert_eq!(parsed.error.unwrap(), "something went wrong");
    }

    #[test]
    fn test_request_id_increment() {
        // Verify AtomicU64 behavior for request IDs
        let counter = AtomicU64::new(1);
        assert_eq!(counter.fetch_add(1, Ordering::SeqCst), 1);
        assert_eq!(counter.fetch_add(1, Ordering::SeqCst), 2);
        assert_eq!(counter.fetch_add(1, Ordering::SeqCst), 3);
    }

    #[test]
    fn test_worker_deserialization() {
        let worker_json = serde_json::json!({
            "id": "worker-12345678",
            "runner_name": null,
            "command": "claude",
            "args": "[]",
            "event_type": "task.unblocked",
            "filters": "[]",
            "concurrency": 1,
            "instance_path": "/home/user/project",
            "status": "running",
            "error_message": null,
            "pid": 12345,
            "detached": false,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z",
            "stopped_at": null,
            "last_event_id": 0
        });

        let worker: Worker = serde_json::from_value(worker_json).unwrap();
        assert_eq!(worker.id, "worker-12345678");
        assert_eq!(worker.command, "claude");
        assert!(worker.is_running());
    }

    #[test]
    fn test_run_deserialization() {
        let run_json = serde_json::json!({
            "id": "run-12345678",
            "worker_id": "worker-abcdefgh",
            "event_id": 42,
            "event_type": "task.unblocked",
            "entity_id": "my-project-task-1",
            "command": "claude",
            "args": "[\"code\", \"--task\"]",
            "status": "running",
            "exit_code": null,
            "error_message": null,
            "attempt": 1,
            "max_attempts": 3,
            "next_retry_at": null,
            "pid": 54321,
            "log_path": "/home/user/.granary/logs/run-12345678.log",
            "started_at": "2024-01-01T00:00:00Z",
            "completed_at": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        });

        let run: Run = serde_json::from_value(run_json).unwrap();
        assert_eq!(run.id, "run-12345678");
        assert_eq!(run.worker_id, "worker-abcdefgh");
        assert!(run.is_running());
    }

    #[test]
    fn test_start_worker_request_serialization() {
        let req = StartWorkerRequest {
            runner_name: Some("claude-runner".to_string()),
            command: "claude".to_string(),
            args: vec!["code".to_string(), "--task".to_string()],
            event_type: "task.unblocked".to_string(),
            filters: vec!["project=my-project".to_string()],
            concurrency: 2,
            instance_path: "/home/user/project".to_string(),
            attach: true,
            since: None,
        };

        let json = serde_json::to_string(&req).unwrap();
        let parsed: StartWorkerRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.runner_name, Some("claude-runner".to_string()));
        assert_eq!(parsed.command, "claude");
        assert_eq!(parsed.concurrency, 2);
        assert!(parsed.attach);
    }
}
