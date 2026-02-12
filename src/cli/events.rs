//! Events CLI command.
//!
//! Lists and manages events in the workspace event log.

use std::time::Duration;

use crate::cli::args::CliOutputFormat;
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::Event;
use crate::output::{Output, json, table};
use crate::services::Workspace;

// =============================================================================
// Output Types
// =============================================================================

/// Output for a list of events
pub struct EventsOutput {
    pub events: Vec<Event>,
}

impl Output for EventsOutput {
    fn to_json(&self) -> String {
        json::format_events(&self.events)
    }

    fn to_prompt(&self) -> String {
        table::format_events(&self.events)
    }

    fn to_text(&self) -> String {
        if self.events.is_empty() {
            return "No events found.".to_string();
        }
        table::format_events(&self.events)
    }
}

/// Output for drain result
pub struct DrainOutput {
    pub events_deleted: u64,
    pub consumptions_deleted: u64,
}

impl Output for DrainOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&serde_json::json!({
            "events_deleted": self.events_deleted,
            "consumptions_deleted": self.consumptions_deleted,
        }))
        .unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        self.to_text()
    }

    fn to_text(&self) -> String {
        format!(
            "Drained {} events and {} consumption records.",
            self.events_deleted, self.consumptions_deleted
        )
    }
}

// =============================================================================
// Commands
// =============================================================================

/// List events with optional filters and watch mode
pub async fn list_events(
    event_type: Option<String>,
    entity: Option<String>,
    since: Option<String>,
    limit: u32,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    if watch {
        let interval_duration = Duration::from_secs(interval);
        let event_type = event_type.clone();
        let entity = entity.clone();
        let since = since.clone();
        watch_loop(interval_duration, || {
            let event_type = event_type.clone();
            let entity = entity.clone();
            let since = since.clone();
            async move {
                let output = fetch_and_format_events(
                    event_type.as_deref(),
                    entity.as_deref(),
                    since.as_deref(),
                    limit,
                    cli_format,
                )
                .await?;
                Ok(format!(
                    "{}\n{}",
                    watch_status_line(interval_duration),
                    output
                ))
            }
        })
        .await?;
    } else {
        let output = fetch_and_format_events(
            event_type.as_deref(),
            entity.as_deref(),
            since.as_deref(),
            limit,
            cli_format,
        )
        .await?;
        println!("{}", output);
    }

    Ok(())
}

/// Drain old events before a given duration or timestamp
pub async fn drain_events(before: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let before_timestamp = parse_duration_or_timestamp(before)?;

    // Get max event ID before the cutoff (for cleaning consumptions)
    let max_id = db::events::max_id_before(&pool, &before_timestamp).await?;

    // Delete events
    let events_deleted = db::events::delete_before(&pool, &before_timestamp).await?;

    // Delete consumption records for deleted events
    let consumptions_deleted = if max_id > 0 {
        db::event_consumptions::delete_before(&pool, max_id).await?
    } else {
        0
    };

    let output = DrainOutput {
        events_deleted,
        consumptions_deleted,
    };
    println!("{}", output.format(cli_format));

    Ok(())
}

// =============================================================================
// Helpers
// =============================================================================

async fn fetch_and_format_events(
    event_type: Option<&str>,
    entity: Option<&str>,
    since: Option<&str>,
    limit: u32,
    cli_format: Option<CliOutputFormat>,
) -> anyhow::Result<String> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Parse since if it's a relative duration
    let since_timestamp = match since {
        Some(s) => Some(parse_duration_or_timestamp(s)?),
        None => None,
    };

    let events =
        db::events::list_filtered(&pool, event_type, entity, since_timestamp.as_deref(), limit)
            .await?;

    let output = EventsOutput { events };
    Ok(output.format(cli_format))
}

/// Parse a string as either a relative duration ("1h", "7d", "30m") or an ISO timestamp.
pub fn parse_duration_or_timestamp(s: &str) -> Result<String> {
    // Try relative duration first
    if let Some(duration) = parse_relative_duration(s) {
        let cutoff = chrono::Utc::now() - duration;
        return Ok(cutoff.to_rfc3339());
    }

    // Try ISO timestamp
    if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
        return Ok(s.to_string());
    }

    // Try date-only format
    if chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok() {
        return Ok(format!("{}T00:00:00Z", s));
    }

    Err(GranaryError::InvalidArgument(format!(
        "Invalid duration or timestamp: '{}'. Use relative (1h, 7d, 30m) or ISO timestamp.",
        s
    )))
}

/// Parse a relative duration string like "1h", "7d", "30m", "2w".
fn parse_relative_duration(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    if s.len() < 2 {
        return None;
    }

    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().ok()?;

    match unit {
        "m" => chrono::Duration::try_minutes(num),
        "h" => chrono::Duration::try_hours(num),
        "d" => chrono::Duration::try_days(num),
        "w" => chrono::Duration::try_weeks(num),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_relative_duration_hours() {
        let d = parse_relative_duration("1h").unwrap();
        assert_eq!(d.num_hours(), 1);
    }

    #[test]
    fn test_parse_relative_duration_days() {
        let d = parse_relative_duration("7d").unwrap();
        assert_eq!(d.num_days(), 7);
    }

    #[test]
    fn test_parse_relative_duration_minutes() {
        let d = parse_relative_duration("30m").unwrap();
        assert_eq!(d.num_minutes(), 30);
    }

    #[test]
    fn test_parse_relative_duration_weeks() {
        let d = parse_relative_duration("2w").unwrap();
        assert_eq!(d.num_weeks(), 2);
    }

    #[test]
    fn test_parse_relative_duration_invalid() {
        assert!(parse_relative_duration("abc").is_none());
        assert!(parse_relative_duration("").is_none());
        assert!(parse_relative_duration("x").is_none());
    }

    #[test]
    fn test_parse_duration_or_timestamp_relative() {
        let result = parse_duration_or_timestamp("1h").unwrap();
        // Should be a valid RFC3339 timestamp
        assert!(chrono::DateTime::parse_from_rfc3339(&result).is_ok());
    }

    #[test]
    fn test_parse_duration_or_timestamp_iso() {
        let ts = "2026-01-01T00:00:00+00:00";
        let result = parse_duration_or_timestamp(ts).unwrap();
        assert_eq!(result, ts);
    }

    #[test]
    fn test_parse_duration_or_timestamp_date_only() {
        let result = parse_duration_or_timestamp("2026-01-01").unwrap();
        assert_eq!(result, "2026-01-01T00:00:00Z");
    }

    #[test]
    fn test_parse_duration_or_timestamp_invalid() {
        assert!(parse_duration_or_timestamp("not-a-date").is_err());
    }
}
