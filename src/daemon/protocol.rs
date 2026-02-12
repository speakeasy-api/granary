//! IPC protocol types and framing for daemon communication.
//!
//! This module defines the Request/Response types and the length-delimited
//! JSON protocol used for CLI-daemon communication over Unix domain sockets.
//!
//! ## Protocol Format
//!
//! Messages are framed using a simple length-delimited format:
//! - 4 bytes: message length (big-endian u32)
//! - N bytes: JSON-encoded message
//!
//! This allows for efficient parsing and streaming of messages.

use serde::{Deserialize, Serialize};
use std::io;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Maximum message size (16 MB) to prevent memory exhaustion attacks
pub const MAX_MESSAGE_SIZE: u32 = 16 * 1024 * 1024;

/// IPC Request envelope sent from CLI to daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique request identifier for correlating responses
    pub id: u64,
    /// The operation to perform
    pub op: Operation,
}

impl Request {
    /// Create a new request with the given ID and operation
    pub fn new(id: u64, op: Operation) -> Self {
        Self { id, op }
    }
}

/// IPC Response envelope sent from daemon to CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Request ID this response corresponds to
    pub id: u64,
    /// Whether the operation succeeded
    pub ok: bool,
    /// Response body (operation-specific data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
    /// Error message if ok is false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    /// Create a successful response with a body
    pub fn ok(id: u64, body: impl Serialize) -> Self {
        Self {
            id,
            ok: true,
            body: Some(serde_json::to_value(body).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }

    /// Create a successful response with no body
    pub fn ok_empty(id: u64) -> Self {
        Self {
            id,
            ok: true,
            body: None,
            error: None,
        }
    }

    /// Create an error response
    pub fn err(id: u64, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            body: None,
            error: Some(error.into()),
        }
    }
}

/// Authentication request sent as the first message on connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    /// The authentication token (should match ~/.granary/daemon/auth.token)
    pub token: String,
}

/// Operations supported by the daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Operation {
    // Authentication
    /// Authenticate the connection (must be first message)
    Auth(AuthRequest),

    // Daemon control
    /// Check if daemon is alive
    Ping,
    /// Request daemon shutdown
    Shutdown,

    // Worker management
    /// Start a new worker
    StartWorker(StartWorkerRequest),
    /// Stop a running worker
    StopWorker {
        worker_id: String,
        /// Whether to also stop running runs
        stop_runs: bool,
    },
    /// Get worker details
    GetWorker { worker_id: String },
    /// List all workers
    ListWorkers {
        /// Include stopped workers
        all: bool,
    },
    /// Remove stopped workers
    PruneWorkers,
    /// Get worker logs
    WorkerLogs {
        worker_id: String,
        /// Follow log output
        follow: bool,
        /// Number of lines to show
        lines: i32,
    },

    // Run management
    /// Get run details
    GetRun { run_id: String },
    /// List runs
    ListRuns {
        /// Filter by worker
        worker_id: Option<String>,
        /// Filter by status
        status: Option<String>,
        /// Include completed/failed runs
        all: bool,
    },
    /// Stop a running run
    StopRun { run_id: String },
    /// Pause a running run
    PauseRun { run_id: String },
    /// Resume a paused run
    ResumeRun { run_id: String },
    /// Get run logs
    RunLogs {
        run_id: String,
        /// Follow log output
        follow: bool,
        /// Number of lines to show
        lines: i32,
    },

    /// Get logs with offset-based pagination (for streaming support)
    GetLogs(LogsRequest),
}

/// Target type for log requests
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LogTarget {
    /// Worker logs (worker.log)
    Worker,
    /// Individual run logs
    Run,
}

/// Request payload for getting logs with offset-based pagination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsRequest {
    /// Target ID - either worker_id or run_id
    pub target_id: String,
    /// Type of target (worker or run)
    pub target_type: LogTarget,
    /// Return lines after this line number (0-indexed)
    pub since_line: u64,
    /// Maximum number of lines to return
    pub limit: u64,
}

/// Response payload for log requests with streaming support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsResponse {
    /// Log lines returned
    pub lines: Vec<String>,
    /// Line number to use as since_line for the next request
    pub next_line: u64,
    /// True if more logs might be coming (target is still active)
    pub has_more: bool,
    /// Path to the log file (for debugging/direct access)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_path: Option<PathBuf>,
}

/// Request payload for starting a new worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartWorkerRequest {
    /// Optional reference to a configured runner
    pub runner_name: Option<String>,
    /// Command to execute
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Event type to subscribe to (e.g., "task.unblocked")
    pub event_type: String,
    /// Filter expressions (e.g., ["status!=draft"])
    pub filters: Vec<String>,
    /// Maximum concurrent runs
    pub concurrency: i32,
    /// Workspace root path
    pub instance_path: String,
    /// Whether to attach to the worker's output stream
    pub attach: bool,
    /// Resolved ISO timestamp to start processing events from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

impl Default for StartWorkerRequest {
    fn default() -> Self {
        Self {
            runner_name: None,
            command: String::new(),
            args: Vec::new(),
            event_type: String::new(),
            filters: Vec::new(),
            concurrency: 1,
            instance_path: String::new(),
            attach: false,
            since: None,
        }
    }
}

/// Write a length-delimited frame to an async writer.
///
/// Frame format:
/// - 4 bytes: length (big-endian u32)
/// - N bytes: data
///
/// # Errors
///
/// Returns an error if the data exceeds MAX_MESSAGE_SIZE or if writing fails.
pub async fn write_frame<W: AsyncWriteExt + Unpin>(writer: &mut W, data: &[u8]) -> io::Result<()> {
    if data.len() > MAX_MESSAGE_SIZE as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "message too large: {} bytes (max {})",
                data.len(),
                MAX_MESSAGE_SIZE
            ),
        ));
    }

    let len = data.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(data).await?;
    writer.flush().await?;
    Ok(())
}

/// Read a length-delimited frame from an async reader.
///
/// Frame format:
/// - 4 bytes: length (big-endian u32)
/// - N bytes: data
///
/// # Errors
///
/// Returns an error if:
/// - The connection is closed (EOF when reading length)
/// - The message size exceeds MAX_MESSAGE_SIZE
/// - Reading fails
pub async fn read_frame<R: AsyncReadExt + Unpin>(reader: &mut R) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;

    let len = u32::from_be_bytes(len_buf);

    if len > MAX_MESSAGE_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "message too large: {} bytes (max {})",
                len, MAX_MESSAGE_SIZE
            ),
        ));
    }

    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).await?;
    Ok(buf)
}

/// Serialize and write a request to an async writer.
pub async fn write_request<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    request: &Request,
) -> io::Result<()> {
    let json =
        serde_json::to_vec(request).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    write_frame(writer, &json).await
}

/// Read and deserialize a request from an async reader.
pub async fn read_request<R: AsyncReadExt + Unpin>(reader: &mut R) -> io::Result<Request> {
    let data = read_frame(reader).await?;
    serde_json::from_slice(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Serialize and write a response to an async writer.
pub async fn write_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    response: &Response,
) -> io::Result<()> {
    let json =
        serde_json::to_vec(response).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    write_frame(writer, &json).await
}

/// Read and deserialize a response from an async reader.
pub async fn read_response<R: AsyncReadExt + Unpin>(reader: &mut R) -> io::Result<Response> {
    let data = read_frame(reader).await?;
    serde_json::from_slice(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_request_serialization_roundtrip() {
        let request = Request::new(42, Operation::Ping);
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: Request = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, 42);
        assert!(matches!(deserialized.op, Operation::Ping));
    }

    #[test]
    fn test_response_ok_serialization() {
        let response = Response::ok(1, "hello");
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        assert!(deserialized.ok);
        assert_eq!(deserialized.id, 1);
        assert_eq!(deserialized.body.unwrap().as_str().unwrap(), "hello");
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn test_response_err_serialization() {
        let response = Response::err(2, "something went wrong");
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.ok);
        assert_eq!(deserialized.id, 2);
        assert!(deserialized.body.is_none());
        assert_eq!(deserialized.error.unwrap(), "something went wrong");
    }

    #[test]
    fn test_response_ok_empty_serialization() {
        let response = Response::ok_empty(3);
        let json = serde_json::to_string(&response).unwrap();
        // Verify that None fields are skipped
        assert!(!json.contains("body"));
        assert!(!json.contains("error"));
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        assert!(deserialized.ok);
        assert!(deserialized.body.is_none());
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn test_operation_tagged_serialization() {
        // Test that operations serialize with type tags
        let op = Operation::StartWorker(StartWorkerRequest {
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            event_type: "task.created".to_string(),
            ..Default::default()
        });
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains(r#""type":"StartWorker""#));
        assert!(json.contains(r#""data""#));

        // Verify roundtrip
        let deserialized: Operation = serde_json::from_str(&json).unwrap();
        if let Operation::StartWorker(req) = deserialized {
            assert_eq!(req.command, "echo");
            assert_eq!(req.args, vec!["hello"]);
            assert_eq!(req.event_type, "task.created");
        } else {
            panic!("Expected StartWorker operation");
        }
    }

    #[test]
    fn test_operation_unit_variant_serialization() {
        // Test unit variants (no data)
        let op = Operation::Ping;
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains(r#""type":"Ping""#));
        assert!(!json.contains(r#""data""#));

        let deserialized: Operation = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, Operation::Ping));
    }

    #[test]
    fn test_operation_struct_variant_serialization() {
        // Test struct variants (inline fields)
        let op = Operation::StopWorker {
            worker_id: "worker-abc123".to_string(),
            stop_runs: true,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Operation = serde_json::from_str(&json).unwrap();
        if let Operation::StopWorker {
            worker_id,
            stop_runs,
        } = deserialized
        {
            assert_eq!(worker_id, "worker-abc123");
            assert!(stop_runs);
        } else {
            panic!("Expected StopWorker operation");
        }
    }

    #[test]
    fn test_auth_request_serialization() {
        let auth = AuthRequest {
            token: "test-token-123".to_string(),
        };
        let json = serde_json::to_string(&auth).unwrap();
        assert!(json.contains("test-token-123"));

        let deserialized: AuthRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.token, "test-token-123");
    }

    #[test]
    fn test_auth_operation_serialization() {
        let op = Operation::Auth(AuthRequest {
            token: "my-secret-token".to_string(),
        });
        let json = serde_json::to_string(&op).unwrap();
        assert!(json.contains(r#""type":"Auth""#));
        assert!(json.contains("my-secret-token"));

        let deserialized: Operation = serde_json::from_str(&json).unwrap();
        if let Operation::Auth(auth) = deserialized {
            assert_eq!(auth.token, "my-secret-token");
        } else {
            panic!("Expected Auth operation");
        }
    }

    #[test]
    fn test_all_operations_serialize() {
        // Ensure all operation variants serialize without error
        let operations = vec![
            Operation::Auth(AuthRequest {
                token: "test".to_string(),
            }),
            Operation::Ping,
            Operation::Shutdown,
            Operation::StartWorker(StartWorkerRequest::default()),
            Operation::StopWorker {
                worker_id: "w1".to_string(),
                stop_runs: false,
            },
            Operation::GetWorker {
                worker_id: "w1".to_string(),
            },
            Operation::ListWorkers { all: true },
            Operation::PruneWorkers,
            Operation::WorkerLogs {
                worker_id: "w1".to_string(),
                follow: true,
                lines: 100,
            },
            Operation::GetRun {
                run_id: "r1".to_string(),
            },
            Operation::ListRuns {
                worker_id: Some("w1".to_string()),
                status: Some("running".to_string()),
                all: false,
            },
            Operation::StopRun {
                run_id: "r1".to_string(),
            },
            Operation::PauseRun {
                run_id: "r1".to_string(),
            },
            Operation::ResumeRun {
                run_id: "r1".to_string(),
            },
            Operation::RunLogs {
                run_id: "r1".to_string(),
                follow: false,
                lines: 50,
            },
            Operation::GetLogs(LogsRequest {
                target_id: "w1".to_string(),
                target_type: LogTarget::Worker,
                since_line: 0,
                limit: 100,
            }),
        ];

        for op in operations {
            let json = serde_json::to_string(&op).unwrap();
            let _: Operation = serde_json::from_str(&json).unwrap();
        }
    }

    #[tokio::test]
    async fn test_frame_roundtrip() {
        let data = b"hello, world!";

        // Write frame to buffer
        let mut buf = Vec::new();
        write_frame(&mut buf, data).await.unwrap();

        // Verify frame format: 4-byte length prefix + data
        assert_eq!(buf.len(), 4 + data.len());
        let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        assert_eq!(len as usize, data.len());

        // Read frame back
        let mut reader = Cursor::new(buf);
        let read_data = read_frame(&mut reader).await.unwrap();
        assert_eq!(read_data, data);
    }

    #[tokio::test]
    async fn test_request_response_roundtrip() {
        // Test request roundtrip
        let request = Request::new(
            123,
            Operation::StartWorker(StartWorkerRequest {
                command: "claude".to_string(),
                args: vec!["code".to_string(), "--task".to_string()],
                event_type: "task.unblocked".to_string(),
                filters: vec!["project=my-project".to_string()],
                concurrency: 2,
                instance_path: "/home/user/project".to_string(),
                attach: true,
                ..Default::default()
            }),
        );

        let mut buf = Vec::new();
        write_request(&mut buf, &request).await.unwrap();

        let mut reader = Cursor::new(buf);
        let read_request = read_request(&mut reader).await.unwrap();

        assert_eq!(read_request.id, 123);
        if let Operation::StartWorker(req) = read_request.op {
            assert_eq!(req.command, "claude");
            assert_eq!(req.concurrency, 2);
            assert!(req.attach);
        } else {
            panic!("Expected StartWorker");
        }
    }

    #[tokio::test]
    async fn test_response_roundtrip() {
        let response = Response::ok(
            456,
            serde_json::json!({
                "worker_id": "worker-12345678",
                "status": "running"
            }),
        );

        let mut buf = Vec::new();
        write_response(&mut buf, &response).await.unwrap();

        let mut reader = Cursor::new(buf);
        let read_response = read_response(&mut reader).await.unwrap();

        assert_eq!(read_response.id, 456);
        assert!(read_response.ok);
        let body = read_response.body.unwrap();
        assert_eq!(body["worker_id"], "worker-12345678");
    }

    #[tokio::test]
    async fn test_frame_size_limit() {
        // Test that writing oversized data fails
        let oversized = vec![0u8; (MAX_MESSAGE_SIZE + 1) as usize];
        let mut buf = Vec::new();
        let result = write_frame(&mut buf, &oversized).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("message too large")
        );
    }

    #[tokio::test]
    async fn test_read_frame_size_limit() {
        // Craft a frame header claiming an oversized message
        let mut buf = Vec::new();
        let oversized_len = MAX_MESSAGE_SIZE + 1;
        buf.extend_from_slice(&oversized_len.to_be_bytes());
        buf.extend_from_slice(b"some data");

        let mut reader = Cursor::new(buf);
        let result = read_frame(&mut reader).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("message too large")
        );
    }

    #[tokio::test]
    async fn test_multiple_frames() {
        // Test reading multiple frames from one stream
        let mut buf = Vec::new();
        write_frame(&mut buf, b"first").await.unwrap();
        write_frame(&mut buf, b"second").await.unwrap();
        write_frame(&mut buf, b"third").await.unwrap();

        let mut reader = Cursor::new(buf);
        assert_eq!(read_frame(&mut reader).await.unwrap(), b"first");
        assert_eq!(read_frame(&mut reader).await.unwrap(), b"second");
        assert_eq!(read_frame(&mut reader).await.unwrap(), b"third");
    }
}
