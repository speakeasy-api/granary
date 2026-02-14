//! Event polling service for workers.
//!
//! This service provides a mechanism for workers to poll for new events
//! that match their subscriptions. It uses claim-based consumption via
//! EventConsumerService to ensure events are not processed multiple times,
//! even across multiple consumers.

use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::error::{GranaryError, Result};
use crate::models::Event;
use crate::services::event_consumer::EventConsumerService;

/// Default polling interval in milliseconds
const DEFAULT_POLL_INTERVAL_MS: u64 = 1000;

/// Configuration for the event poller
#[derive(Debug, Clone)]
pub struct EventPollerConfig {
    /// Interval between polls
    pub poll_interval: Duration,
    /// Maximum number of events to process per poll
    pub batch_size: Option<usize>,
    /// Whether to automatically update the cursor after processing
    pub auto_update_cursor: bool,
}

impl Default for EventPollerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(DEFAULT_POLL_INTERVAL_MS),
            batch_size: None,
            auto_update_cursor: true,
        }
    }
}

impl EventPollerConfig {
    /// Create a new config with a specific poll interval
    pub fn with_poll_interval(poll_interval: Duration) -> Self {
        Self {
            poll_interval,
            ..Default::default()
        }
    }

    /// Set the batch size
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = Some(size);
        self
    }

    /// Set whether to auto-update cursor
    pub fn auto_update_cursor(mut self, auto: bool) -> Self {
        self.auto_update_cursor = auto;
        self
    }
}

/// Event poller for a worker
///
/// The event poller manages the subscription to events for a specific worker.
/// It uses claim-based consumption via EventConsumerService to atomically
/// claim events, preventing double-processing across consumers.
pub struct EventPoller {
    /// The worker ID (used as consumer ID)
    worker_id: String,
    /// The event type to subscribe to
    event_type: String,
    /// The underlying consumer service
    consumer: EventConsumerService,
    /// Configuration
    config: EventPollerConfig,
}

impl EventPoller {
    /// Create a new event poller backed by EventConsumerService.
    ///
    /// # Arguments
    /// * `workspace_pool` - Database pool for the workspace (events)
    /// * `worker_id` - The ID of the worker (used as consumer ID)
    /// * `event_type` - The event type to subscribe to
    /// * `filter_strings` - Filter expressions as strings
    /// * `config` - Poller configuration
    /// * `start_from` - ISO timestamp; events before this are ignored
    /// * `initial_last_seen` - Initial last_seen_id for the consumer
    pub async fn new(
        workspace_pool: SqlitePool,
        worker_id: String,
        event_type: String,
        filter_strings: &[String],
        config: EventPollerConfig,
        start_from: Option<String>,
        initial_last_seen: i64,
    ) -> Result<Self> {
        let consumer = EventConsumerService::new(
            workspace_pool,
            worker_id.clone(),
            event_type.clone(),
            filter_strings,
            start_from,
            initial_last_seen,
        )
        .await?;

        Ok(Self {
            worker_id,
            event_type,
            consumer,
            config,
        })
    }

    /// Poll for new events matching the worker's subscription.
    ///
    /// Uses claim-based consumption: each event is atomically claimed
    /// and returned. No double-processing is possible.
    ///
    /// The `limit` parameter overrides the configured batch_size when provided.
    /// This is used by the worker runtime to only claim as many events as it
    /// has available concurrency slots, preventing events from being claimed
    /// but never processed.
    pub async fn poll(&mut self, limit: Option<usize>) -> Result<Vec<Event>> {
        let batch_size = limit.or(self.config.batch_size);
        self.consumer.poll(batch_size).await
    }

    /// Manually acknowledge processing up to a specific event ID.
    pub async fn acknowledge(&mut self, event_id: i64) -> Result<()> {
        self.consumer.acknowledge(event_id).await
    }

    /// Start a continuous polling loop
    ///
    /// This method will poll for events at the configured interval and
    /// send matching events to the provided channel. The loop continues
    /// until the channel is closed or an unrecoverable error occurs.
    pub async fn start_polling(&mut self, tx: mpsc::Sender<Event>) -> Result<()> {
        loop {
            if tx.is_closed() {
                return Ok(());
            }

            match self.poll(None).await {
                Ok(events) => {
                    for event in events {
                        if tx.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    if let GranaryError::Conflict(msg) = &e
                        && msg.contains("not found")
                    {
                        return Err(e);
                    }
                }
            }

            tokio::time::sleep(self.config.poll_interval).await;
        }
    }

    /// Start polling with a cancellation token
    pub async fn start_polling_with_cancel(
        &mut self,
        tx: mpsc::Sender<Event>,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        let mut cancel = cancel;

        loop {
            if *cancel.borrow() {
                return Ok(());
            }

            if tx.is_closed() {
                return Ok(());
            }

            match self.poll(None).await {
                Ok(events) => {
                    for event in events {
                        if tx.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    if let GranaryError::Conflict(msg) = &e
                        && msg.contains("not found")
                    {
                        return Err(e);
                    }
                }
            }

            tokio::select! {
                _ = tokio::time::sleep(self.config.poll_interval) => {}
                _ = cancel.changed() => {
                    if *cancel.borrow() {
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Get the worker ID
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// Get the event type
    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    /// Get the poll interval
    pub fn poll_interval(&self) -> Duration {
        self.config.poll_interval
    }
}

/// Create an event poller from a worker record
///
/// For existing workers with last_event_id > 0, uses that as initial last_seen_id
/// and sets started_at to epoch (to not miss old events).
/// For new workers, started_at = now, last_seen_id = 0.
pub async fn create_poller_for_worker(
    worker: &crate::models::Worker,
    workspace_pool: SqlitePool,
    config: EventPollerConfig,
    since: Option<String>,
) -> Result<EventPoller> {
    let filter_strings = worker.filters_vec();

    let (start_from, initial_last_seen) = if worker.last_event_id > 0 {
        // Existing worker migrating from cursor-based: use epoch so we don't skip events
        (
            Some("1970-01-01T00:00:00Z".to_string()),
            worker.last_event_id,
        )
    } else if let Some(since_ts) = since {
        // New worker with --since: start from the given timestamp
        (Some(since_ts), 0i64)
    } else {
        // New worker: start from now
        (None, 0i64)
    };

    EventPoller::new(
        workspace_pool,
        worker.id.clone(),
        worker.event_type.clone(),
        &filter_strings,
        config,
        start_from,
        initial_last_seen,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EventPollerConfig::default();
        assert_eq!(
            config.poll_interval.as_millis(),
            DEFAULT_POLL_INTERVAL_MS as u128
        );
        assert!(config.batch_size.is_none());
        assert!(config.auto_update_cursor);
    }

    #[test]
    fn test_config_builder() {
        let config = EventPollerConfig::with_poll_interval(Duration::from_secs(5))
            .batch_size(10)
            .auto_update_cursor(false);

        assert_eq!(config.poll_interval.as_secs(), 5);
        assert_eq!(config.batch_size, Some(10));
        assert!(!config.auto_update_cursor);
    }
}
