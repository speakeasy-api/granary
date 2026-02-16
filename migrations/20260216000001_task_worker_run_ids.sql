-- Add worker_ids and run_ids columns to tasks table
-- These track which workers and runs have operated on a task
-- Stored as JSON arrays (same pattern as tags)
ALTER TABLE tasks ADD COLUMN worker_ids TEXT;
ALTER TABLE tasks ADD COLUMN run_ids TEXT;
