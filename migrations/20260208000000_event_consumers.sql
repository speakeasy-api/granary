-- Event consumers: independent consumers that track their own position in the event stream
CREATE TABLE IF NOT EXISTS event_consumers (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    started_at TEXT NOT NULL,
    last_seen_id INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_event_consumers_event_type ON event_consumers(event_type);

-- Event consumptions: tracks which events have been claimed by which consumer
CREATE TABLE IF NOT EXISTS event_consumptions (
    consumer_id TEXT NOT NULL,
    event_id INTEGER NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    consumed_at TEXT NOT NULL,
    PRIMARY KEY (consumer_id, event_id)
);
CREATE INDEX IF NOT EXISTS idx_event_consumptions_event_id ON event_consumptions(event_id);
CREATE INDEX IF NOT EXISTS idx_event_consumptions_consumer_id ON event_consumptions(consumer_id);
