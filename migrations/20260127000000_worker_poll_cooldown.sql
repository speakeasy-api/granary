-- Add poll_cooldown_secs column to workers table for polled event time-gating
-- Default is 300 seconds (5 minutes)

ALTER TABLE workers ADD COLUMN poll_cooldown_secs INTEGER NOT NULL DEFAULT 300;
