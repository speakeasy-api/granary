-- Add last_edited_by columns to mutable entities for trigger-based event system.
-- Triggers will read this column to set the actor on emitted events.

ALTER TABLE projects ADD COLUMN last_edited_by TEXT;
ALTER TABLE tasks ADD COLUMN last_edited_by TEXT;
ALTER TABLE sessions ADD COLUMN last_edited_by TEXT;

-- Index for efficient event filtering by type and entity (used by trigger-emitted .next events)
CREATE INDEX IF NOT EXISTS idx_events_type_entity ON events(event_type, entity_id);
