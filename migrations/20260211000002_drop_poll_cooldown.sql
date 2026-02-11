-- Remove poll_cooldown_secs column from workers table
-- No longer needed since polling infrastructure is replaced by trigger-based events
ALTER TABLE workers DROP COLUMN poll_cooldown_secs;
