-- Runs table for tracking individual runner executions
-- Each time a worker spawns a runner in response to an event, that execution
-- is tracked as a "Run". Runs have their own lifecycle and support retry
-- with exponential backoff.
-- This table is stored in the same GLOBAL database as workers (~/.granary/workers.db)

CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    worker_id TEXT NOT NULL REFERENCES workers(id),
    event_id INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    command TEXT NOT NULL,
    args TEXT NOT NULL DEFAULT '[]',       -- JSON array of arguments
    status TEXT NOT NULL DEFAULT 'pending', -- pending, running, completed, failed, paused, cancelled
    exit_code INTEGER,
    error_message TEXT,
    attempt INTEGER NOT NULL DEFAULT 1,     -- retry attempt number (1-based)
    max_attempts INTEGER NOT NULL DEFAULT 3, -- max retries before giving up
    next_retry_at TEXT,                     -- when to retry (with backoff)
    pid INTEGER,                            -- OS process ID when running
    log_path TEXT,                          -- path to stdout/stderr log file
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runs_worker_id ON runs(worker_id);
CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);
CREATE INDEX IF NOT EXISTS idx_runs_event_id ON runs(event_id);
CREATE INDEX IF NOT EXISTS idx_runs_next_retry_at ON runs(next_retry_at);
