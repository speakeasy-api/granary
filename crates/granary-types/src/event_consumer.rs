use serde::{Deserialize, Serialize};

#[cfg(feature = "sqlx")]
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(FromRow))]
pub struct EventConsumer {
    pub id: String,
    pub event_type: String,
    pub started_at: String,
    pub last_seen_id: i64,
    pub created_at: String,
    pub updated_at: String,
}
