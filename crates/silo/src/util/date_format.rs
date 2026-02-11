use chrono::{DateTime, Local, Utc};

/// Format a UTC datetime as a human-readable relative time
/// e.g., "2 hours ago", "yesterday", "3 days ago"
pub fn format_relative(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(dt);

    let seconds = duration.num_seconds();
    let minutes = duration.num_minutes();
    let hours = duration.num_hours();
    let days = duration.num_days();

    if seconds < 60 {
        "just now".to_string()
    } else if minutes < 60 {
        format!(
            "{} minute{} ago",
            minutes,
            if minutes == 1 { "" } else { "s" }
        )
    } else if hours < 24 {
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if days == 1 {
        "yesterday".to_string()
    } else if days < 7 {
        format!("{} days ago", days)
    } else if days < 30 {
        let weeks = days / 7;
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else {
        format_short(dt)
    }
}

/// Format a UTC datetime as short date (e.g., "Jan 15, 2024")
pub fn format_short(dt: DateTime<Utc>) -> String {
    let local: DateTime<Local> = dt.into();
    local.format("%b %d, %Y").to_string()
}

/// Format a UTC datetime as full date and time (e.g., "January 15, 2024 at 3:45 PM")
pub fn format_full(dt: DateTime<Utc>) -> String {
    let local: DateTime<Local> = dt.into();
    local.format("%B %d, %Y at %I:%M %p").to_string()
}

/// Format a UTC datetime as ISO 8601 string
pub fn format_iso(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}
