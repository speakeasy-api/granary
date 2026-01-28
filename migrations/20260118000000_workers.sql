-- Workers table for tracking worker processes
-- Workers are long-running processes that subscribe to granary events and spawn runners
-- This table is stored in a GLOBAL database (~/.granary/workers.db) to allow
-- `granary worker list` to show workers across all workspaces

CREATE TABLE IF NOT EXISTS workers (
    id TEXT PRIMARY KEY,
    runner_name TEXT,                       -- references config runner, or NULL for inline
    command TEXT NOT NULL,
    args TEXT NOT NULL DEFAULT '[]',        -- JSON array of arguments
    event_type TEXT NOT NULL,               -- e.g., "task.unblocked"
    filters TEXT NOT NULL DEFAULT '[]',     -- JSON array of filter expressions
    concurrency INTEGER NOT NULL DEFAULT 1, -- max concurrent runner instances
    instance_path TEXT NOT NULL,            -- workspace root this worker is attached to
    status TEXT NOT NULL DEFAULT 'pending', -- pending, running, stopped, error
    error_message TEXT,                     -- error message if status is error
    pid INTEGER,                            -- OS process ID when running
    detached INTEGER NOT NULL DEFAULT 0,    -- 1 if running as daemon
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    stopped_at TEXT                         -- timestamp when worker was stopped
);

CREATE INDEX IF NOT EXISTS idx_workers_status ON workers(status);
CREATE INDEX IF NOT EXISTS idx_workers_instance_path ON workers(instance_path);
CREATE INDEX IF NOT EXISTS idx_workers_event_type ON workers(event_type);
