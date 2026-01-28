//! Event polling service for workers.
//!
//! This service provides a mechanism for workers to poll for new events
//! that match their subscriptions. It uses cursor-based pagination to
//! ensure events are not processed multiple times.

use std::time::Duration;

use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::event::Event;
use crate::services::filter::{Filter, matches_all, parse_filters};

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
/// It polls the workspace database for new events matching the worker's
/// event type and filters, and tracks the cursor in the global database.
pub struct EventPoller {
    /// The workspace database pool (where events are stored)
    workspace_pool: SqlitePool,
    /// The global database pool (where workers are stored)
    global_pool: SqlitePool,
    /// The worker ID
    worker_id: String,
    /// The event type to subscribe to (e.g., "task.unblocked")
    event_type: String,
    /// Parsed filter expressions
    filters: Vec<Filter>,
    /// Configuration
    config: EventPollerConfig,
}

impl EventPoller {
    /// Create a new event poller
    ///
    /// # Arguments
    /// * `workspace_pool` - Database pool for the workspace (events)
    /// * `global_pool` - Database pool for the global database (workers)
    /// * `worker_id` - The ID of the worker
    /// * `event_type` - The event type to subscribe to
    /// * `filter_strings` - Filter expressions as strings
    /// * `config` - Poller configuration
    ///
    /// # Returns
    /// A new EventPoller or an error if filters cannot be parsed
    pub fn new(
        workspace_pool: SqlitePool,
        global_pool: SqlitePool,
        worker_id: String,
        event_type: String,
        filter_strings: &[String],
        config: EventPollerConfig,
    ) -> Result<Self> {
        let filters = parse_filters(filter_strings)?;
        Ok(Self {
            workspace_pool,
            global_pool,
            worker_id,
            event_type,
            filters,
            config,
        })
    }

    /// Create a new event poller with default configuration
    pub fn new_default(
        workspace_pool: SqlitePool,
        global_pool: SqlitePool,
        worker_id: String,
        event_type: String,
        filter_strings: &[String],
    ) -> Result<Self> {
        Self::new(
            workspace_pool,
            global_pool,
            worker_id,
            event_type,
            filter_strings,
            EventPollerConfig::default(),
        )
    }

    /// Get the worker's current event cursor (last processed event ID)
    async fn get_cursor(&self) -> Result<i64> {
        let worker = db::workers::get(&self.global_pool, &self.worker_id)
            .await?
            .ok_or_else(|| {
                GranaryError::Conflict(format!("Worker {} not found", self.worker_id))
            })?;
        Ok(worker.last_event_id)
    }

    /// Update the worker's event cursor
    async fn update_cursor(&self, last_event_id: i64) -> Result<()> {
        db::workers::update_cursor(&self.global_pool, &self.worker_id, last_event_id).await?;
        Ok(())
    }

    /// Poll for new events matching the worker's subscription
    ///
    /// This method:
    /// 1. Gets the current cursor from the worker
    /// 2. Fetches events since that cursor
    /// 3. Filters events by type and custom filters
    /// 4. Updates the cursor to the last processed event
    /// 5. Returns matching events
    pub async fn poll(&self) -> Result<Vec<Event>> {
        let cursor = self.get_cursor().await?;

        // Fetch events since the cursor, pre-filtered by event type
        let events =
            db::events::list_since_id_by_type(&self.workspace_pool, cursor, &self.event_type)
                .await?;

        if events.is_empty() {
            return Ok(vec![]);
        }

        // Apply custom filters
        let matching: Vec<_> = events
            .into_iter()
            .filter(|e| {
                if self.filters.is_empty() {
                    return true;
                }
                let payload = e.payload_json();
                matches_all(&self.filters, &payload)
            })
            .collect();

        // Apply batch size limit if configured
        let matching = if let Some(batch_size) = self.config.batch_size {
            matching.into_iter().take(batch_size).collect()
        } else {
            matching
        };

        // Update cursor if we have matching events and auto-update is enabled
        if self.config.auto_update_cursor
            && let Some(last) = matching.last()
        {
            self.update_cursor(last.id).await?;
        }

        Ok(matching)
    }

    /// Manually update the cursor to a specific event ID
    ///
    /// This is useful when auto_update_cursor is disabled and you want
    /// to manually acknowledge events after processing.
    pub async fn acknowledge(&self, event_id: i64) -> Result<()> {
        self.update_cursor(event_id).await
    }

    /// Start a continuous polling loop
    ///
    /// This method will poll for events at the configured interval and
    /// send matching events to the provided channel. The loop continues
    /// until the channel is closed or an unrecoverable error occurs.
    ///
    /// # Arguments
    /// * `tx` - The channel sender to send events to
    ///
    /// # Returns
    /// Ok(()) if the channel was closed normally, or an error if polling failed
    pub async fn start_polling(&self, tx: mpsc::Sender<Event>) -> Result<()> {
        loop {
            // Check if the channel is still open
            if tx.is_closed() {
                return Ok(());
            }

            // Poll for events
            match self.poll().await {
                Ok(events) => {
                    for event in events {
                        if tx.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    // Check if this is a fatal error (worker not found)
                    if let GranaryError::Conflict(msg) = &e
                        && msg.contains("not found")
                    {
                        return Err(e);
                    }
                    // For other errors, continue polling (workspace may be temporarily unavailable)
                }
            }

            // Wait for the next poll interval
            tokio::time::sleep(self.config.poll_interval).await;
        }
    }

    /// Start polling with a cancellation token
    ///
    /// This is similar to `start_polling` but allows for graceful shutdown
    /// via a cancellation token.
    pub async fn start_polling_with_cancel(
        &self,
        tx: mpsc::Sender<Event>,
        cancel: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        let mut cancel = cancel;

        loop {
            // Check for cancellation
            if *cancel.borrow() {
                return Ok(());
            }

            // Check if the channel is still open
            if tx.is_closed() {
                return Ok(());
            }

            // Poll for events
            match self.poll().await {
                Ok(events) => {
                    for event in events {
                        if tx.send(event).await.is_err() {
                            return Ok(());
                        }
                    }
                }
                Err(e) => {
                    // Check if this is a fatal error (worker not found)
                    if let GranaryError::Conflict(msg) = &e
                        && msg.contains("not found")
                    {
                        return Err(e);
                    }
                    // For other errors, continue polling
                }
            }

            // Wait for either the poll interval or cancellation
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
/// This is a convenience function that creates an EventPoller from
/// an existing Worker struct.
pub fn create_poller_for_worker(
    worker: &crate::models::worker::Worker,
    workspace_pool: SqlitePool,
    global_pool: SqlitePool,
    config: EventPollerConfig,
) -> Result<EventPoller> {
    let filter_strings = worker.filters_vec();
    EventPoller::new(
        workspace_pool,
        global_pool,
        worker.id.clone(),
        worker.event_type.clone(),
        &filter_strings,
        config,
    )
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
