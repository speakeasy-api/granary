//! Event consumer service for claim-based event consumption.
//!
//! Each consumer independently tracks its position in the event stream.
//! Events are claimed atomically via a single SQL query, preventing
//! double-processing even when multiple consumers share an ID.

use sqlx::SqlitePool;

use crate::db;
use crate::error::Result;
use crate::models::{Event, EventConsumer};
use crate::services::filter::{Filter, parse_filters};

/// Service for consuming events with atomic claim-based processing.
pub struct EventConsumerService {
    pool: SqlitePool,
    consumer_id: String,
    event_type: String,
    filters: Vec<Filter>,
    /// Cached SQL clause tuples from Filter::to_sql()
    filter_clauses: Vec<(String, String, String)>,
    /// Consumer record (lazily loaded after register)
    consumer: EventConsumer,
}

impl EventConsumerService {
    /// Create a new event consumer service, registering the consumer if it doesn't exist.
    ///
    /// # Arguments
    /// * `pool` - Workspace database pool (where events live)
    /// * `consumer_id` - Unique identifier for this consumer
    /// * `event_type` - Event type to subscribe to
    /// * `filter_strings` - Filter expressions as strings
    /// * `start_from` - ISO timestamp; events before this are ignored. If None, uses "now".
    /// * `initial_last_seen` - Initial last_seen_id for the consumer (0 for new consumers)
    pub async fn new(
        pool: SqlitePool,
        consumer_id: String,
        event_type: String,
        filter_strings: &[String],
        start_from: Option<String>,
        initial_last_seen: i64,
    ) -> Result<Self> {
        let filters = parse_filters(filter_strings)?;
        let filter_clauses: Vec<_> = filters.iter().map(|f| f.to_sql()).collect();

        let started_at = start_from.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

        let consumer = db::event_consumers::register(
            &pool,
            &consumer_id,
            &event_type,
            &started_at,
            initial_last_seen,
        )
        .await?;

        Ok(Self {
            pool,
            consumer_id,
            event_type,
            filters,
            filter_clauses,
            consumer,
        })
    }

    /// Poll for new events, claiming each one atomically.
    ///
    /// Calls try_claim_next in a loop until no more events are available.
    /// Returns all claimed events. Respects batch_size if provided.
    pub async fn poll(&mut self, batch_size: Option<usize>) -> Result<Vec<Event>> {
        let mut claimed = Vec::new();
        let limit = batch_size.unwrap_or(usize::MAX);

        loop {
            if claimed.len() >= limit {
                break;
            }

            let event = db::event_consumptions::try_claim_next(
                &self.pool,
                &self.consumer_id,
                &self.event_type,
                self.consumer.last_seen_id,
                &self.consumer.started_at,
                &self.filter_clauses,
            )
            .await?;

            match event {
                Some(e) => {
                    // Update last_seen_id to optimize future scans
                    if e.id > self.consumer.last_seen_id {
                        self.consumer.last_seen_id = e.id;
                    }
                    claimed.push(e);
                }
                None => break,
            }
        }

        // Persist last_seen_id if we claimed any events
        if !claimed.is_empty() {
            db::event_consumers::update_last_seen(
                &self.pool,
                &self.consumer_id,
                self.consumer.last_seen_id,
            )
            .await?;
        }

        Ok(claimed)
    }

    /// Acknowledge processing up to a specific event ID.
    /// Updates the consumer's last_seen_id.
    pub async fn acknowledge(&mut self, event_id: i64) -> Result<()> {
        if event_id > self.consumer.last_seen_id {
            self.consumer.last_seen_id = event_id;
            db::event_consumers::update_last_seen(&self.pool, &self.consumer_id, event_id).await?;
        }
        Ok(())
    }

    /// Get the consumer ID.
    pub fn consumer_id(&self) -> &str {
        &self.consumer_id
    }

    /// Get the event type.
    pub fn event_type(&self) -> &str {
        &self.event_type
    }

    /// Get the filters.
    pub fn filters(&self) -> &[Filter] {
        &self.filters
    }
}
