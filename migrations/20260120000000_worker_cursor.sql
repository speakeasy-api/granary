-- Add last_event_id column to workers table for event cursor tracking
-- This tracks the last processed event ID per worker to prevent duplicate processing

ALTER TABLE workers ADD COLUMN last_event_id INTEGER DEFAULT 0;

-- Index for efficient cursor-based queries
CREATE INDEX IF NOT EXISTS idx_workers_last_event_id ON workers(last_event_id);
