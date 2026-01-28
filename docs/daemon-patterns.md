# Daemon Implementation Patterns

## Existing Codebase Patterns

### Worker Runtime (src/services/worker_runtime.rs)
The `WorkerRuntime` struct is the core component that:
- Polls for events using `EventPoller`
- Spawns runner processes via `spawn_runner()`
- Manages concurrency limits (`self.worker.concurrency`)
- Handles retries with exponential backoff
- Uses `watch::channel` for shutdown signaling

Key methods:
- `WorkerRuntime::new()` - Creates runtime with shutdown receiver
- `run()` - Main loop with `tokio::select!` for shutdown + polling
- `graceful_shutdown()` - Waits 30s for active runs, then force-kills

### Database Layer (src/db/mod.rs)
- Uses sqlx with SQLite
- Async queries with `query_as::<_, Model>`
- Pool passed as `&SqlitePool`
- Global DB at `~/.granary/workers.db`
- Workspace DB at `{workspace}/.granary/granary.db`

### Error Handling (src/error.rs)
- `GranaryError` enum with thiserror
- `Result<T>` type alias
- Error variants for common cases

### CLI Pattern (src/cli/*.rs)
- Functions are `async fn command_name(...) -> Result<()>`
- Use `Formatter::new(format)` for output
- Access global pool via `global_config_service::global_pool().await?`

### Worker Model (src/models/worker.rs)
Fields: id, runner_name, command, args (JSON), event_type, filters (JSON), concurrency, instance_path, status, error_message, pid, detached, created_at, updated_at, stopped_at, last_event_id

### Run Model (src/models/run.rs)
Fields: id, worker_id, event_id, event_type, entity_id, command, args, status, exit_code, error_message, log_path, attempt, max_attempts, next_retry_at, pid, created_at, updated_at, completed_at

### Configuration (src/services/global_config.rs)
- `config_dir()` → `~/.granary`
- `global_db_path()` → `~/.granary/workers.db`
- `logs_dir()` → `~/.granary/logs`
- `worker_logs_dir(worker_id)` → `~/.granary/logs/{worker_id}`

## Daemon-Specific Patterns

### File Layout
```
~/.granary/
├── workers.db                    # Global worker/run registry
├── config.toml                   # Global config
├── daemon/
│   ├── granaryd.sock             # Unix socket (Unix)
│   ├── granaryd.pid              # Daemon PID
│   └── daemon.log                # Daemon logs
└── logs/
    └── {worker_id}/
        ├── worker.log            # Worker lifecycle events
        └── {run_id}.log          # Run stdout/stderr
```

### IPC Protocol
Length-delimited JSON frames:
- 4 bytes: message length (big-endian u32)
- N bytes: JSON payload

Request: `{ id: u64, op: Operation }`
Response: `{ id: u64, ok: bool, body?: Value, error?: String }`

### Shutdown Pattern
1. Receive SIGTERM/SIGINT
2. Stop accepting new connections
3. Signal all workers via `watch::Sender::send(true)`
4. Wait up to 30s for workers to finish
5. Force-kill remaining runs
6. Update worker statuses to "stopped"
7. Clean up PID file and exit
