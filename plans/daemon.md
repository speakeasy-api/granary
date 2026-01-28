# Daemon Architecture Plan

## Goal

Implement `granaryd`, a per-user daemon that owns all worker and run lifecycles. The CLI becomes a thin client that communicates via IPC.

```
┌─────────────────────────────────────────────────────┐
│                    granaryd                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐         │
│  │ Worker A │  │ Worker B │  │ Worker C │  (tasks) │
│  └──────────┘  └──────────┘  └──────────┘         │
│                      │                              │
│              ┌───────┴───────┐                      │
│              │  Run Manager  │                      │
│              └───────────────┘                      │
└──────────────────────┬──────────────────────────────┘
                       │ IPC (Unix socket / named pipe)
                       │
              ┌────────┴────────┐
              │   granary CLI   │
              └─────────────────┘
```

---

## Current State

### What Exists and Can Be Reused

| Component | Status | Reuse Plan |
|-----------|--------|------------|
| `Worker` model | ✅ Complete | Use as-is in daemon |
| `Run` model | ✅ Complete | Use as-is in daemon |
| `~/.granary/workers.db` | ✅ Complete | Daemon reads/writes this |
| `WorkerRuntime` | ✅ Complete | Move into daemon as tokio task |
| `EventPoller` | ✅ Complete | Used by WorkerRuntime |
| `spawn_runner()` | ✅ Complete | Used by WorkerRuntime |
| Retry logic (exponential backoff) | ✅ Complete | Part of WorkerRuntime |
| Concurrency control | ✅ Complete | Part of WorkerRuntime |
| Log capture to files | ✅ Complete | Keep file-based logging |
| CLI commands | ✅ Complete | Rewire to IPC calls |

### What's Broken / Incomplete

**`--detached` mode is a noop:**
```rust
// src/cli/worker.rs:130-139
if detached {
    // For detached mode, we need to fork and run in background
    // For now, we'll just print instructions to run with a process manager
    println!("Worker {} created in detached mode.", worker.id);
    println!("To start the worker, run:");
    println!("  granary worker run {} &");
}
```

This creates a database record but doesn't start anything. Users must manually run the worker or use systemd/launchd.

**Foreground mode ties up terminal:**
- Worker runs in current process
- Ctrl+C required to stop
- No way to "detach" a running worker

---

## Implementation Plan

### Phase 1: Daemon Binary and IPC Foundation

**1.1 Create `src/bin/granaryd.rs`**

The daemon is a long-running process that:
- Listens on IPC socket for commands
- Manages workers as tokio tasks
- Owns all run processes
- Handles graceful shutdown

```rust
// Minimal structure
#[tokio::main]
async fn main() -> Result<()> {
    let config = load_daemon_config()?;
    let db = open_global_db().await?;

    // Start IPC listener
    let listener = create_ipc_listener(&config).await?;

    // Restore running workers from DB
    let manager = WorkerManager::new(db);
    manager.restore_workers().await?;

    // Main loop: accept connections, handle commands
    loop {
        select! {
            conn = listener.accept() => handle_connection(conn, &manager),
            _ = shutdown_signal() => break,
        }
    }

    manager.shutdown_all().await?;
    Ok(())
}
```

**1.2 IPC Transport**

Unix socket on Unix, named pipe on Windows:

```
~/.granary/daemon/granaryd.sock      # Unix
\\.\pipe\granaryd-<user>             # Windows
```

Protocol: length-delimited JSON frames

```rust
// Request
struct Request {
    id: u64,
    op: String,
    body: serde_json::Value,
}

// Response
struct Response {
    id: u64,
    ok: bool,
    body: serde_json::Value,
    error: Option<String>,
}
```

**1.3 Operations**

| Operation | Request Body | Response |
|-----------|-------------|----------|
| `StartWorker` | `CreateWorker` fields + `attach: bool` | `{ worker_id }` |
| `StopWorker` | `{ worker_id, stop_runs: bool }` | `{}` |
| `GetWorker` | `{ worker_id }` | `Worker` |
| `ListWorkers` | `{ all: bool }` | `Vec<Worker>` |
| `PruneWorkers` | `{}` | `{ pruned: i32 }` |
| `WorkerLogs` | `{ worker_id, follow: bool, lines: i32 }` | Stream of log lines |
| `GetRun` | `{ run_id }` | `Run` |
| `ListRuns` | `{ worker_id?, status?, all: bool }` | `Vec<Run>` |
| `StopRun` | `{ run_id }` | `{}` |
| `PauseRun` | `{ run_id }` | `{}` |
| `ResumeRun` | `{ run_id }` | `{}` |
| `RunLogs` | `{ run_id, follow: bool, lines: i32 }` | Stream of log lines |
| `Ping` | `{}` | `{ version }` |
| `Shutdown` | `{}` | `{}` |

**1.4 Security**

- Socket file permissions: 0600 (Unix)
- Per-user pipe ACLs (Windows)
- Optional auth token in `~/.granary/daemon/auth.token`

```rust
// First message on connect must be auth
struct AuthMessage {
    token: String,
}
```

### Phase 2: Worker Manager

**2.1 WorkerManager struct**

Owns all worker tasks and coordinates lifecycle:

```rust
struct WorkerManager {
    db: Pool<Sqlite>,
    workers: HashMap<String, WorkerHandle>,
}

struct WorkerHandle {
    worker_id: String,
    runtime: Arc<WorkerRuntime>,  // Reuse existing!
    task: JoinHandle<()>,
    shutdown_tx: watch::Sender<bool>,
}
```

**2.2 Starting a Worker**

```rust
impl WorkerManager {
    async fn start_worker(&mut self, create: CreateWorker) -> Result<Worker> {
        // 1. Create DB record
        let worker = db::workers::create(&self.db, &create).await?;

        // 2. Get workspace pool
        let workspace_pool = open_workspace_db(&worker.instance_path).await?;

        // 3. Create runtime (reuse existing WorkerRuntime!)
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let runtime = WorkerRuntime::new(
            worker.clone(),
            self.db.clone(),
            workspace_pool,
            shutdown_rx,
            WorkerRuntimeConfig::default(),
        );

        // 4. Spawn as tokio task
        let runtime = Arc::new(runtime);
        let runtime_clone = runtime.clone();
        let task = tokio::spawn(async move {
            runtime_clone.run().await;
        });

        // 5. Update status
        db::workers::update_status(&self.db, &worker.id, "running", Some(std::process::id())).await?;

        // 6. Track handle
        self.workers.insert(worker.id.clone(), WorkerHandle {
            worker_id: worker.id.clone(),
            runtime,
            task,
            shutdown_tx,
        });

        Ok(worker)
    }
}
```

**2.3 Restoring Workers on Daemon Start**

```rust
impl WorkerManager {
    async fn restore_workers(&mut self) -> Result<()> {
        // Find workers that were running when daemon last stopped
        let workers = db::workers::list_by_status(&self.db, "running").await?;

        for worker in workers {
            // Check if workspace still exists
            if !Path::new(&worker.instance_path).exists() {
                db::workers::update_status(&self.db, &worker.id, "error", None).await?;
                db::workers::set_error(&self.db, &worker.id, "workspace directory missing").await?;
                continue;
            }

            // Restart the worker
            self.start_existing_worker(worker).await?;
        }

        Ok(())
    }
}
```

### Phase 3: Daemon Lifecycle

**3.1 Auto-Start from CLI**

When CLI needs the daemon:

```rust
async fn ensure_daemon() -> Result<DaemonClient> {
    // 1. Try to connect
    match DaemonClient::connect().await {
        Ok(client) => return Ok(client),
        Err(_) => {}
    }

    // 2. Spawn daemon
    spawn_daemon()?;

    // 3. Retry with backoff
    for i in 0..10 {
        tokio::time::sleep(Duration::from_millis(50 * (i + 1))).await;
        if let Ok(client) = DaemonClient::connect().await {
            return Ok(client);
        }
    }

    Err(anyhow!("Failed to start daemon. Check ~/.granary/daemon/daemon.log"))
}
```

**3.2 Platform-Specific Daemon Spawn**

```rust
#[cfg(unix)]
fn spawn_daemon() -> Result<()> {
    use std::process::Command;

    Command::new(std::env::current_exe()?.with_file_name("granaryd"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

#[cfg(windows)]
fn spawn_daemon() -> Result<()> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x00000008;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    Command::new(std::env::current_exe()?.with_file_name("granaryd.exe"))
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn()?;

    Ok(())
}
```

**3.3 Graceful Shutdown**

On SIGTERM/SIGINT:
1. Stop accepting new connections
2. Signal all workers to stop
3. Wait up to 30s for workers to finish
4. Force-kill remaining runs
5. Update all worker statuses to "stopped"
6. Exit

### Phase 4: Rewire CLI Commands

**4.1 Worker Commands**

```rust
// Before: runs worker in current process or prints instructions
// After: sends IPC command to daemon

pub async fn worker_start(args: WorkerStartArgs) -> Result<()> {
    let client = ensure_daemon().await?;

    let worker = client.start_worker(StartWorkerRequest {
        command: args.command,
        args: args.args,
        event_type: args.on,
        filters: args.filters,
        concurrency: args.concurrency,
        instance_path: workspace.root().to_string(),
        attach: !args.detached,
    }).await?;

    println!("Worker {} started.", worker.id);

    if !args.detached {
        // Stream logs until Ctrl+C
        let mut stream = client.worker_logs(worker.id, true).await?;
        while let Some(line) = stream.next().await {
            print!("{}", line);
        }
    }

    Ok(())
}
```

**4.2 Command Mapping**

| CLI Command | IPC Operation |
|-------------|---------------|
| `worker start` | `StartWorker` → `WorkerLogs` (if attached) |
| `worker stop` | `StopWorker` |
| `worker status` | `GetWorker` |
| `worker logs` | `WorkerLogs` |
| `worker prune` | `PruneWorkers` |
| `workers` | `ListWorkers` |
| `run stop` | `StopRun` |
| `run pause` | `PauseRun` |
| `run resume` | `ResumeRun` |
| `run status` | `GetRun` |
| `run logs` | `RunLogs` |
| `runs` | `ListRuns` |

### Phase 5: Log Streaming

**5.1 File-Based with Tail**

Keep current approach: runs write to `~/.granary/logs/{worker_id}/{run_id}.log`

Daemon streams logs by tailing files:

```rust
async fn stream_run_logs(run_id: &str, follow: bool) -> impl Stream<Item = String> {
    let path = logs_dir().join(run_id).with_extension("log");

    if follow {
        // Use notify or tokio file watching
        tail_follow(&path).await
    } else {
        // Read last N lines
        tail_lines(&path, 100).await
    }
}
```

**5.2 Worker Logs**

Aggregate all run logs for a worker, or maintain a separate worker event log:

```
~/.granary/logs/{worker_id}/
├── worker.log           # Worker events (started, stopped, errors)
├── {run_id_1}.log       # Run output
├── {run_id_2}.log
└── ...
```

---

## File Layout

```
~/.granary/
├── workers.db                    # Global worker/run registry
├── config.toml                   # Global config (optional)
├── daemon/
│   ├── granaryd.sock             # Unix socket (Unix)
│   ├── pipe_name                 # Pipe name file (Windows)
│   ├── granaryd.pid              # Daemon PID
│   ├── auth.token                # Auth secret (0600 perms)
│   └── daemon.log                # Daemon logs
└── logs/
    └── {worker_id}/
        ├── worker.log            # Worker lifecycle events
        └── {run_id}.log          # Run stdout/stderr
```

---

## Implementation Order

```
Phase 1: Foundation
├── 1.1 Create granaryd binary skeleton
├── 1.2 Implement IPC listener (Unix socket first)
├── 1.3 Define Request/Response types
└── 1.4 Add Ping operation for testing

Phase 2: Worker Management
├── 2.1 Create WorkerManager
├── 2.2 Implement StartWorker (reuse WorkerRuntime)
├── 2.3 Implement StopWorker
├── 2.4 Implement worker restoration on daemon start
└── 2.5 Implement ListWorkers, GetWorker

Phase 3: CLI Integration
├── 3.1 Create DaemonClient
├── 3.2 Implement ensure_daemon() with auto-start
├── 3.3 Rewire `worker start` to use daemon
├── 3.4 Rewire `worker stop` to use daemon
└── 3.5 Rewire remaining worker commands

Phase 4: Run Management
├── 4.1 Implement StopRun, PauseRun, ResumeRun
├── 4.2 Implement ListRuns, GetRun
├── 4.3 Rewire CLI run commands
└── 4.4 Implement log streaming

Phase 5: Polish
├── 5.1 Windows named pipe support
├── 5.2 Auth token implementation
├── 5.3 Log retention/rotation
├── 5.4 Process groups (kill whole tree)
└── 5.5 `granary daemon status/stop/restart` commands
```

---

## Dependencies to Add

```toml
[dependencies]
# Already have tokio, serde, serde_json

# IPC
tokio-util = { version = "0.7", features = ["codec"] }  # Length-delimited framing

# Platform-specific (optional, can use std)
# nix = "0.29"  # Unix process management
```

---

## Testing Strategy

1. **Unit tests**: WorkerManager, IPC codec
2. **Integration tests**: Start daemon → send commands → verify state
3. **Manual testing**:
   - Start worker attached, Ctrl+C
   - Start worker detached, view logs
   - Kill daemon, verify workers restore on restart
   - Delete workspace dir, verify worker goes to error state
