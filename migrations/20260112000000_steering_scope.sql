-- Add scope support to steering files
-- Allows steering to be attached to: global (NULL), project, task, or session

-- Create new table with scope columns
CREATE TABLE IF NOT EXISTS steering_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL,
    mode TEXT NOT NULL DEFAULT 'always',
    scope_type TEXT,  -- NULL (global), 'project', 'task', 'session'
    scope_id TEXT,    -- The entity ID when scoped
    created_at TEXT NOT NULL,
    UNIQUE(path, scope_type, scope_id)
);

-- Copy existing data (all become global scope)
INSERT OR IGNORE INTO steering_new (id, path, mode, scope_type, scope_id, created_at)
SELECT id, path, mode, NULL, NULL, created_at FROM steering;

-- Drop old table
DROP TABLE IF EXISTS steering;

-- Rename new table
ALTER TABLE steering_new RENAME TO steering;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_steering_mode ON steering(mode);
CREATE INDEX IF NOT EXISTS idx_steering_scope ON steering(scope_type, scope_id);
