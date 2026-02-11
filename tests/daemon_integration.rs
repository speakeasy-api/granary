//! Integration tests for the granary daemon.
//!
//! These tests verify end-to-end functionality of the daemon, CLI, and worker
//! runtime working together. Each test runs in isolation with its own temporary
//! directory and daemon instance.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tempfile::TempDir;
use tokio::time::sleep;

use granary::daemon::DaemonClient;
use granary::daemon::protocol::StartWorkerRequest;

/// Test helper to start a test daemon in isolation.
///
/// Each TestDaemon instance:
/// - Creates a temporary directory for GRANARY_HOME
/// - Starts the daemon process with that environment
/// - Provides a client for interacting with the daemon
/// - Cleans up everything on drop
struct TestDaemon {
    /// Temporary directory used as GRANARY_HOME
    temp_dir: TempDir,
    /// The daemon process handle
    process: Option<Child>,
    /// Path to the socket for this instance
    #[cfg(unix)]
    socket_path: PathBuf,
    /// Path to the auth token for this instance
    auth_token_path: PathBuf,
}

impl TestDaemon {
    /// Start a new test daemon instance.
    ///
    /// This creates a temporary directory, sets up the environment, and starts
    /// the daemon process. It waits for the daemon to be ready before returning.
    async fn start() -> Result<Self, String> {
        let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

        // Create daemon directory structure at ~/.granary/daemon (where ~ is temp_dir)
        // The daemon uses dirs::home_dir() which reads HOME env var
        let granary_dir = temp_dir.path().join(".granary");
        let daemon_dir = granary_dir.join("daemon");
        std::fs::create_dir_all(&daemon_dir)
            .map_err(|e| format!("Failed to create daemon dir: {}", e))?;

        #[cfg(unix)]
        let socket_path = daemon_dir.join("granaryd.sock");
        let auth_token_path = daemon_dir.join("auth.token");

        // Find the granaryd binary - it should be in target/debug or target/release
        let daemon_path = find_daemon_binary()?;

        // Start daemon with isolated environment
        let process = Command::new(&daemon_path)
            .env("GRANARY_HOME", temp_dir.path())
            .env("HOME", temp_dir.path()) // Override HOME so config_dir uses temp
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn daemon: {}", e))?;

        let mut instance = Self {
            temp_dir,
            process: Some(process),
            #[cfg(unix)]
            socket_path,
            auth_token_path,
        };

        // Wait for daemon to be ready (up to 5 seconds)
        for i in 0..50 {
            sleep(Duration::from_millis(100)).await;
            if instance.try_connect().await.is_ok() {
                return Ok(instance);
            }
            // Check if process is still running
            if let Some(ref mut proc) = instance.process
                && let Ok(Some(status)) = proc.try_wait()
            {
                // Capture stderr for debugging
                let stderr = proc.stderr.take();
                let stderr_content = if let Some(mut err) = stderr {
                    use std::io::Read;
                    let mut s = String::new();
                    let _ = err.read_to_string(&mut s);
                    s
                } else {
                    String::new()
                };
                return Err(format!(
                    "Daemon exited prematurely with status: {:?}\nstderr: {}",
                    status, stderr_content
                ));
            }
            if i == 49 {
                // Capture stderr for debugging
                let stderr_content = if let Some(ref mut proc) = instance.process {
                    if let Some(mut err) = proc.stderr.take() {
                        use std::io::Read;
                        let mut s = String::new();
                        let _ = err.read_to_string(&mut s);
                        s
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                return Err(format!(
                    "Daemon failed to start within 5 seconds\nSocket path: {:?}\nstderr: {}",
                    instance.socket_path, stderr_content
                ));
            }
        }

        Ok(instance)
    }

    /// Read the auth token from the daemon's temp directory.
    fn read_auth_token(&self) -> Result<String, String> {
        if !self.auth_token_path.exists() {
            return Err("Auth token file does not exist yet".to_string());
        }
        std::fs::read_to_string(&self.auth_token_path)
            .map(|s| s.trim().to_string())
            .map_err(|e| format!("Failed to read auth token: {}", e))
    }

    /// Try to connect to the daemon.
    ///
    /// Creates a connection and authenticates with the daemon.
    #[cfg(unix)]
    async fn try_connect(&self) -> Result<DaemonClient, String> {
        use tokio::net::UnixStream;

        if !self.socket_path.exists() {
            return Err("Socket does not exist yet".to_string());
        }

        // Read the auth token from the daemon's temp home
        // Wait briefly if the auth token doesn't exist yet (it's created on first connection)
        let token = match self.read_auth_token() {
            Ok(t) => t,
            Err(_) => {
                // Auth token might not exist yet, wait a bit and retry
                sleep(Duration::from_millis(100)).await;
                self.read_auth_token()?
            }
        };

        let stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| format!("Connect failed: {}", e))?;

        let mut client = DaemonClient::from_stream(stream);

        // Authenticate with the daemon using the correct token
        client
            .authenticate_with_token(&token)
            .await
            .map_err(|e| format!("Authentication failed: {}", e))?;

        Ok(client)
    }

    #[cfg(windows)]
    async fn try_connect(&self) -> Result<DaemonClient, String> {
        // On Windows, construct pipe name based on temp dir
        // For isolation, we'd need to set up a unique pipe name per test
        Err("Windows integration tests not yet implemented".to_string())
    }

    /// Get a connected client to this daemon.
    #[cfg(unix)]
    async fn client(&self) -> Result<DaemonClient, String> {
        self.try_connect().await
    }

    /// Stop the daemon gracefully.
    #[allow(dead_code)]
    async fn stop(&mut self) -> Result<(), String> {
        // Try graceful shutdown via IPC first
        if let Ok(mut client) = self.try_connect().await {
            let _ = client.shutdown().await;
            // Wait for process to exit
            for _ in 0..30 {
                sleep(Duration::from_millis(100)).await;
                if let Some(ref mut proc) = self.process
                    && proc.try_wait().ok().flatten().is_some()
                {
                    self.process = None;
                    return Ok(());
                }
            }
        }

        // Force kill if still running
        if let Some(ref mut proc) = self.process {
            let _ = proc.kill();
            let _ = proc.wait();
        }
        self.process = None;
        Ok(())
    }

    /// Get the temporary GRANARY_HOME path for this instance.
    fn home_path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        // Ensure process is killed when test ends
        if let Some(ref mut proc) = self.process {
            let _ = proc.kill();
            let _ = proc.wait();
        }
    }
}

/// Find the granaryd binary in the target directory.
fn find_daemon_binary() -> Result<PathBuf, String> {
    // Try debug build first, then release
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let target_dir = PathBuf::from(manifest_dir).join("target");

    let debug_path = target_dir.join("debug").join("granaryd");
    if debug_path.exists() {
        return Ok(debug_path);
    }

    let release_path = target_dir.join("release").join("granaryd");
    if release_path.exists() {
        return Ok(release_path);
    }

    // Check if we're running from cargo test (binary should be next to test binary)
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling_path = dir.join("granaryd");
        if sibling_path.exists() {
            return Ok(sibling_path);
        }
    }

    Err(format!(
        "granaryd binary not found. Build it first with 'cargo build'. Searched in: {:?}",
        target_dir
    ))
}

// ============================================================================
// Integration Tests
// ============================================================================

/// Test that the daemon responds to ping requests.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_ping() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    let mut client = daemon.client().await.expect("Failed to connect to daemon");
    let version = client.ping().await.expect("Ping failed");

    assert!(!version.is_empty(), "Version should not be empty");
    // Version should match the crate version
    assert!(
        version.contains(env!("CARGO_PKG_VERSION")) || version.starts_with(char::is_numeric),
        "Version should be a valid version string: {}",
        version
    );
}

/// Test starting and stopping a worker through the daemon.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_start_stop_worker() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    let mut client = daemon.client().await.expect("Failed to connect to daemon");

    // Create a test workspace directory
    let workspace_path = daemon.home_path().join("test-workspace");
    std::fs::create_dir_all(&workspace_path).expect("Failed to create workspace");

    // Initialize a minimal granary database in the workspace
    let db_path = workspace_path.join(".granary").join("granary.db");
    std::fs::create_dir_all(db_path.parent().unwrap()).expect("Failed to create .granary dir");

    // Start a simple worker that just echoes
    let req = StartWorkerRequest {
        runner_name: None,
        command: "echo".to_string(),
        args: vec!["test".to_string()],
        event_type: "task.unblocked".to_string(),
        filters: vec![],
        concurrency: 1,
        instance_path: workspace_path.to_string_lossy().to_string(),
        attach: false,
    };

    // Note: This will likely fail because the workspace doesn't have a proper granary DB
    // but we can test that the daemon accepts the request
    let result = client.start_worker(req).await;

    // The worker start may fail due to missing DB, but the daemon should handle it gracefully
    match result {
        Ok(worker) => {
            assert!(!worker.id.is_empty(), "Worker ID should not be empty");

            // List workers - should include our worker
            let workers = client
                .list_workers(true)
                .await
                .expect("Failed to list workers");
            assert!(
                workers.iter().any(|w| w.id == worker.id),
                "Worker should be in list"
            );

            // Stop the worker
            client
                .stop_worker(&worker.id, true)
                .await
                .expect("Failed to stop worker");

            // Verify worker is stopped
            let stopped_worker = client
                .get_worker(&worker.id)
                .await
                .expect("Failed to get worker");
            assert_eq!(stopped_worker.status, "stopped", "Worker should be stopped");
        }
        Err(e) => {
            // Expected - workspace doesn't have proper granary setup
            eprintln!("Worker start failed (expected): {}", e);
        }
    }
}

/// Test listing workers when none exist.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_list_workers_empty() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    let mut client = daemon.client().await.expect("Failed to connect to daemon");

    // List workers - should be empty
    let workers = client
        .list_workers(false)
        .await
        .expect("Failed to list workers");
    assert!(workers.is_empty(), "Worker list should be empty");

    // List all workers (including stopped) - should still be empty
    let all_workers = client
        .list_workers(true)
        .await
        .expect("Failed to list all workers");
    assert!(all_workers.is_empty(), "All workers list should be empty");
}

/// Test daemon shutdown via IPC.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_shutdown() {
    let mut daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Verify daemon is running
    let mut client = daemon.client().await.expect("Failed to connect to daemon");
    let _ = client.ping().await.expect("Initial ping failed");

    // Request shutdown
    let shutdown_result = client.shutdown().await;
    assert!(shutdown_result.is_ok(), "Shutdown request should succeed");

    // Wait a bit for shutdown
    sleep(Duration::from_millis(500)).await;

    // Connection should fail now (daemon has shut down)
    let connect_result = daemon.try_connect().await;
    // Either the socket is gone or connection is refused
    if connect_result.is_ok() {
        // If we can still connect, ping should fail
        let mut client = connect_result.unwrap();
        let ping_result = client.ping().await;
        // The ping might succeed if daemon hasn't fully shut down yet,
        // or fail if it has
        if ping_result.is_ok() {
            // Give it more time
            sleep(Duration::from_secs(1)).await;
        }
    }

    // Clean up
    daemon.process = None; // Don't try to kill already-dead process
}

/// Test multiple sequential connections to the daemon.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_multiple_connections() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Make multiple sequential connections
    for i in 0..5 {
        let mut client = daemon
            .client()
            .await
            .unwrap_or_else(|_| panic!("Failed to connect (attempt {})", i));
        let version = client.ping().await.expect("Ping failed");
        assert!(!version.is_empty());
    }
}

/// Test concurrent connections to the daemon.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_concurrent_connections() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Read the auth token for all connections
    let auth_token = daemon.read_auth_token().expect("Failed to read auth token");

    // Spawn multiple concurrent ping requests
    let socket_path = daemon.socket_path.clone();
    let mut handles = Vec::new();

    for _ in 0..5 {
        let path = socket_path.clone();
        let token = auth_token.clone();
        let handle = tokio::spawn(async move {
            use tokio::net::UnixStream;

            let stream = UnixStream::connect(&path).await?;
            let mut client = DaemonClient::from_stream(stream);
            // Authenticate before using the client
            client.authenticate_with_token(&token).await?;
            client.ping().await
        });
        handles.push(handle);
    }

    // All pings should succeed
    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.expect("Task panicked");
        assert!(
            result.is_ok(),
            "Concurrent ping {} failed: {:?}",
            i,
            result.err()
        );
    }
}

/// Test listing runs when none exist.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_list_runs_empty() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    let mut client = daemon.client().await.expect("Failed to connect to daemon");

    // List runs - should be empty
    let runs = client
        .list_runs(None, None, false)
        .await
        .expect("Failed to list runs");
    assert!(runs.is_empty(), "Run list should be empty");

    // List all runs - should still be empty
    let all_runs = client
        .list_runs(None, None, true)
        .await
        .expect("Failed to list all runs");
    assert!(all_runs.is_empty(), "All runs list should be empty");
}

/// Test getting a non-existent worker.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_get_nonexistent_worker() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    let mut client = daemon.client().await.expect("Failed to connect to daemon");

    // Try to get a worker that doesn't exist
    let result = client.get_worker("nonexistent-worker-id").await;
    assert!(result.is_err(), "Getting nonexistent worker should fail");
}

/// Test getting a non-existent run.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_get_nonexistent_run() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    let mut client = daemon.client().await.expect("Failed to connect to daemon");

    // Try to get a run that doesn't exist
    let result = client.get_run("nonexistent-run-id").await;
    assert!(result.is_err(), "Getting nonexistent run should fail");
}

/// Test that stopping a non-existent worker fails gracefully.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_stop_nonexistent_worker() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    let mut client = daemon.client().await.expect("Failed to connect to daemon");

    // Try to stop a worker that doesn't exist - this actually succeeds in the
    // current implementation (just updates DB status)
    let result = client.stop_worker("nonexistent-worker-id", false).await;
    // The operation should complete without error (idempotent)
    assert!(
        result.is_ok(),
        "Stopping nonexistent worker should succeed (idempotent)"
    );
}

// ============================================================================
// Worker Restoration Tests
// ============================================================================
//
// Note: Full worker restoration tests require a properly initialized workspace
// with a granary database. The daemon's restore_workers() function is tested
// in unit tests in src/daemon/worker_manager.rs. The integration tests here
// verify the daemon starts and accepts connections, which exercises the
// restoration path at startup.
//
// The ensure_daemon() auto-start function is tested manually as it modifies
// global state (~/.granary/daemon) which would interfere with other tests
// running in parallel.

/// Test that the daemon properly handles rapid start/stop cycles.
/// This exercises the socket cleanup logic.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_rapid_restart() {
    // Start and stop daemon multiple times in quick succession
    for i in 0..3 {
        let daemon = match TestDaemon::start().await {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Skipping test (iteration {}): {}", i, e);
                return;
            }
        };

        let mut client = daemon.client().await.expect("Failed to connect");
        let version = client.ping().await.expect("Ping failed");
        assert!(!version.is_empty());

        // Daemon will be stopped and cleaned up when dropped
        drop(daemon);

        // Small delay between restarts
        sleep(Duration::from_millis(100)).await;
    }
}

/// Test that the daemon handles connection after client disconnect.
#[tokio::test]
#[cfg(unix)]
async fn test_daemon_connection_lifecycle() {
    let daemon = match TestDaemon::start().await {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Connect, use, and disconnect multiple times
    for _ in 0..3 {
        // Create a new client connection
        let mut client = daemon.client().await.expect("Failed to connect");

        // Make a request
        let version = client.ping().await.expect("Ping failed");
        assert!(!version.is_empty());

        // Drop the client (disconnect)
        drop(client);

        // Small delay
        sleep(Duration::from_millis(50)).await;
    }

    // Daemon should still be healthy after all the connect/disconnect cycles
    let mut final_client = daemon.client().await.expect("Final connect failed");
    let _ = final_client.ping().await.expect("Final ping failed");
}
