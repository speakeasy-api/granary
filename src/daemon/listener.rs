//! IPC listener for daemon communication.
//!
//! This module provides the IPC listener that the daemon uses to accept
//! connections from CLI clients. On Unix, this uses Unix domain sockets.
//! On Windows, this uses named pipes.
//!
//! ## Security
//!
//! On Unix, the socket file is created with mode 0600 (owner only) to prevent
//! unauthorized access. The socket file is automatically cleaned up
//! when the listener is dropped.
//!
//! On Windows, the named pipe includes the username for per-user isolation.
//!
//! ## Usage
//!
//! ```ignore
//! use granary::daemon::listener::IpcListener;
//!
//! // Unix: pass socket path
//! // Windows: pass pipe name
//! let listener = IpcListener::bind(socket_path).await?;
//! loop {
//!     let mut conn = listener.accept().await?;
//!     let request = conn.recv_request().await?;
//!     // ... handle request ...
//!     conn.send_response(&response).await?;
//! }
//! ```

use crate::daemon::protocol::{Request, Response, read_request, write_response};
use crate::error::Result;

#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

// ============================================================================
// Unix Implementation
// ============================================================================

#[cfg(unix)]
mod unix_impl {
    use super::*;

    /// Unix socket listener for accepting IPC connections from CLI clients.
    ///
    /// The listener binds to a Unix domain socket and accepts incoming connections.
    /// Each connection is represented by an `IpcConnection` that can be used to
    /// send and receive messages.
    pub struct IpcListener {
        listener: UnixListener,
        socket_path: PathBuf,
    }

    impl IpcListener {
        /// Bind to a Unix domain socket at the given path.
        ///
        /// This will:
        /// 1. Create the parent directory if it doesn't exist
        /// 2. Remove any existing socket file at the path
        /// 3. Bind to the socket
        /// 4. Set socket permissions to 0600 (owner only)
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - The parent directory cannot be created
        /// - The existing socket file cannot be removed
        /// - The socket cannot be bound
        /// - Permissions cannot be set
        pub async fn bind(socket_path: impl AsRef<Path>) -> Result<Self> {
            let socket_path = socket_path.as_ref().to_path_buf();

            // Ensure parent directory exists
            if let Some(parent) = socket_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Remove existing socket file if present (stale from previous run)
            if socket_path.exists() {
                std::fs::remove_file(&socket_path)?;
            }

            let listener = UnixListener::bind(&socket_path)?;

            // Set socket permissions to 0600 for security (owner only)
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
            }

            Ok(Self {
                listener,
                socket_path,
            })
        }

        /// Accept a new incoming connection.
        ///
        /// This method blocks until a new client connects to the socket.
        ///
        /// # Errors
        ///
        /// Returns an error if accepting the connection fails.
        pub async fn accept(&self) -> Result<IpcConnection> {
            let (stream, _addr) = self.listener.accept().await?;
            Ok(IpcConnection::new(stream))
        }

        /// Get the path to the socket file.
        pub fn socket_path(&self) -> &Path {
            &self.socket_path
        }
    }

    impl Drop for IpcListener {
        fn drop(&mut self) {
            // Clean up socket file on shutdown
            // Ignore errors since we're in drop
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }

    /// A connection to a CLI client over the Unix socket.
    ///
    /// Each connection represents a single CLI invocation and supports
    /// request/response communication using the IPC protocol.
    pub struct IpcConnection {
        stream: UnixStream,
    }

    impl IpcConnection {
        /// Create a new connection from a Unix stream.
        pub fn new(stream: UnixStream) -> Self {
            Self { stream }
        }

        /// Receive a request from the client.
        ///
        /// Reads a length-delimited JSON frame from the socket and deserializes
        /// it as a Request.
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - Reading from the socket fails
        /// - The frame cannot be deserialized as a Request
        pub async fn recv_request(&mut self) -> Result<Request> {
            let request = read_request(&mut self.stream).await?;
            Ok(request)
        }

        /// Send a response to the client.
        ///
        /// Serializes the response as JSON and writes it as a length-delimited
        /// frame to the socket.
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - The response cannot be serialized
        /// - Writing to the socket fails
        pub async fn send_response(&mut self, response: &Response) -> Result<()> {
            write_response(&mut self.stream, response).await?;
            Ok(())
        }

        /// Get a reference to the underlying Unix stream.
        ///
        /// This is useful for advanced operations like setting socket options
        /// or implementing custom protocols.
        pub fn stream(&self) -> &UnixStream {
            &self.stream
        }

        /// Get a mutable reference to the underlying Unix stream.
        pub fn stream_mut(&mut self) -> &mut UnixStream {
            &mut self.stream
        }
    }
}

#[cfg(unix)]
pub use unix_impl::*;

// ============================================================================
// Windows Implementation
// ============================================================================

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::io;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    /// Named pipe listener for accepting IPC connections from CLI clients on Windows.
    ///
    /// The listener creates a named pipe and accepts incoming connections.
    /// Each connection is represented by an `IpcConnection` that can be used to
    /// send and receive messages.
    pub struct IpcListener {
        pipe_name: String,
        /// The current pipe server instance waiting for a connection
        server: NamedPipeServer,
    }

    impl IpcListener {
        /// Bind to a named pipe with the given name.
        ///
        /// The pipe name should be in the format `\\.\pipe\{name}`.
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - The pipe cannot be created
        /// - Another process already owns a pipe with this name
        pub async fn bind(pipe_name: impl Into<String>) -> Result<Self> {
            let pipe_name = pipe_name.into();

            // Create the first pipe instance
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&pipe_name)
                .map_err(|e| io::Error::new(io::ErrorKind::AddrInUse, e))?;

            Ok(Self { pipe_name, server })
        }

        /// Accept a new incoming connection.
        ///
        /// This method blocks until a new client connects to the pipe.
        /// After accepting, a new pipe instance is created for the next connection.
        ///
        /// # Errors
        ///
        /// Returns an error if accepting the connection fails or if creating
        /// a new pipe instance fails.
        pub async fn accept(&mut self) -> Result<IpcConnection> {
            // Wait for a client to connect to the current pipe instance
            self.server.connect().await?;

            // Take the connected pipe and create a new one for the next client
            let connected_pipe = std::mem::replace(
                &mut self.server,
                ServerOptions::new()
                    .create(&self.pipe_name)
                    .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?,
            );

            Ok(IpcConnection::new(connected_pipe))
        }

        /// Get the pipe name.
        pub fn pipe_name(&self) -> &str {
            &self.pipe_name
        }
    }

    /// A connection to a CLI client over a Windows named pipe.
    ///
    /// Each connection represents a single CLI invocation and supports
    /// request/response communication using the IPC protocol.
    pub struct IpcConnection {
        pipe: NamedPipeServer,
    }

    impl IpcConnection {
        /// Create a new connection from a named pipe server.
        pub fn new(pipe: NamedPipeServer) -> Self {
            Self { pipe }
        }

        /// Receive a request from the client.
        ///
        /// Reads a length-delimited JSON frame from the pipe and deserializes
        /// it as a Request.
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - Reading from the pipe fails
        /// - The frame cannot be deserialized as a Request
        pub async fn recv_request(&mut self) -> Result<Request> {
            let request = read_request(&mut self.pipe).await?;
            Ok(request)
        }

        /// Send a response to the client.
        ///
        /// Serializes the response as JSON and writes it as a length-delimited
        /// frame to the pipe.
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - The response cannot be serialized
        /// - Writing to the pipe fails
        pub async fn send_response(&mut self, response: &Response) -> Result<()> {
            write_response(&mut self.pipe, response).await?;
            Ok(())
        }

        /// Get a reference to the underlying named pipe.
        pub fn pipe(&self) -> &NamedPipeServer {
            &self.pipe
        }

        /// Get a mutable reference to the underlying named pipe.
        pub fn pipe_mut(&mut self) -> &mut NamedPipeServer {
            &mut self.pipe
        }
    }
}

#[cfg(windows)]
pub use windows_impl::*;

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::daemon::protocol::{Operation, Request, Response};
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::net::UnixStream;
    use tokio::time::timeout;

    /// Helper to create a temporary socket path
    fn temp_socket_path() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.sock");
        (dir, path)
    }

    #[tokio::test]
    async fn test_listener_bind_creates_socket() {
        let (_dir, socket_path) = temp_socket_path();

        let listener = IpcListener::bind(&socket_path).await.unwrap();

        assert!(socket_path.exists());
        assert_eq!(listener.socket_path(), socket_path);
    }

    #[tokio::test]
    async fn test_listener_creates_parent_directory() {
        let dir = TempDir::new().unwrap();
        let socket_path = dir.path().join("nested").join("dir").join("test.sock");

        let _listener = IpcListener::bind(&socket_path).await.unwrap();

        assert!(socket_path.exists());
    }

    #[tokio::test]
    async fn test_listener_removes_existing_socket() {
        let (_dir, socket_path) = temp_socket_path();

        // Create first listener
        let listener1 = IpcListener::bind(&socket_path).await.unwrap();
        drop(listener1); // This removes the socket

        // Socket should be gone
        assert!(!socket_path.exists());

        // Create a stale socket file manually
        std::fs::write(&socket_path, b"stale").unwrap();
        assert!(socket_path.exists());

        // Second listener should succeed by removing the stale file
        let _listener2 = IpcListener::bind(&socket_path).await.unwrap();
        assert!(socket_path.exists());
    }

    #[tokio::test]
    async fn test_listener_drop_cleans_up_socket() {
        let (_dir, socket_path) = temp_socket_path();

        {
            let _listener = IpcListener::bind(&socket_path).await.unwrap();
            assert!(socket_path.exists());
        }
        // Listener dropped here

        assert!(!socket_path.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_socket_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, socket_path) = temp_socket_path();

        let _listener = IpcListener::bind(&socket_path).await.unwrap();

        let metadata = std::fs::metadata(&socket_path).unwrap();
        let mode = metadata.permissions().mode();
        // Check that mode is 0600 (only owner can read/write)
        // The actual mode includes the file type bits, so we mask them
        assert_eq!(mode & 0o777, 0o600);
    }

    #[tokio::test]
    async fn test_accept_connection() {
        let (_dir, socket_path) = temp_socket_path();
        let socket_path_clone = socket_path.clone();

        let listener = IpcListener::bind(&socket_path).await.unwrap();

        // Spawn a client that connects
        let client_handle =
            tokio::spawn(async move { UnixStream::connect(&socket_path_clone).await.unwrap() });

        // Accept the connection with a timeout
        let conn = timeout(Duration::from_secs(1), listener.accept())
            .await
            .unwrap()
            .unwrap();

        assert!(conn.stream().peer_addr().is_ok());
        client_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_request_response_roundtrip() {
        let (_dir, socket_path) = temp_socket_path();
        let socket_path_clone = socket_path.clone();

        let listener = IpcListener::bind(&socket_path).await.unwrap();

        // Spawn server handler
        let server_handle = tokio::spawn(async move {
            let mut conn = listener.accept().await.unwrap();
            let request = conn.recv_request().await.unwrap();
            assert_eq!(request.id, 42);
            assert!(matches!(request.op, Operation::Ping));

            let response = Response::ok_empty(request.id);
            conn.send_response(&response).await.unwrap();
        });

        // Client side
        let client_handle = tokio::spawn(async move {
            let mut stream = UnixStream::connect(&socket_path_clone).await.unwrap();

            // Send request
            let request = Request::new(42, Operation::Ping);
            crate::daemon::protocol::write_request(&mut stream, &request)
                .await
                .unwrap();

            // Read response
            let response = crate::daemon::protocol::read_response(&mut stream)
                .await
                .unwrap();
            assert_eq!(response.id, 42);
            assert!(response.ok);
        });

        // Wait for both with timeout
        timeout(Duration::from_secs(5), async {
            server_handle.await.unwrap();
            client_handle.await.unwrap();
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_multiple_requests_on_same_connection() {
        let (_dir, socket_path) = temp_socket_path();
        let socket_path_clone = socket_path.clone();

        let listener = IpcListener::bind(&socket_path).await.unwrap();

        // Spawn server handler
        let server_handle = tokio::spawn(async move {
            let mut conn = listener.accept().await.unwrap();

            // Handle 3 requests on the same connection
            for expected_id in 1..=3u64 {
                let request = conn.recv_request().await.unwrap();
                assert_eq!(request.id, expected_id);
                let response = Response::ok(request.id, format!("response-{}", expected_id));
                conn.send_response(&response).await.unwrap();
            }
        });

        // Client side
        let client_handle = tokio::spawn(async move {
            let mut stream = UnixStream::connect(&socket_path_clone).await.unwrap();

            for id in 1..=3u64 {
                let request = Request::new(id, Operation::Ping);
                crate::daemon::protocol::write_request(&mut stream, &request)
                    .await
                    .unwrap();

                let response = crate::daemon::protocol::read_response(&mut stream)
                    .await
                    .unwrap();
                assert_eq!(response.id, id);
                assert!(response.ok);
            }
        });

        timeout(Duration::from_secs(5), async {
            server_handle.await.unwrap();
            client_handle.await.unwrap();
        })
        .await
        .unwrap();
    }
}
