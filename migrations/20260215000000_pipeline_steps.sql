-- Add pipeline_steps column to workers table for pipeline action support.
-- When non-null, contains JSON-encoded array of step configs.
-- When null, worker is a simple action (backwards compatible).
ALTER TABLE workers ADD COLUMN pipeline_steps TEXT DEFAULT NULL;
