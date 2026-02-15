-- Add env column to workers table for environment variables
-- Stored as a JSON object, e.g. {"SLACK_WEBHOOK_URL": "https://..."}
ALTER TABLE workers ADD COLUMN env TEXT NOT NULL DEFAULT '{}';
