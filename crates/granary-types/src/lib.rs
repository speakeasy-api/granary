//! Shared types for granary CLI output serialization.
//!
//! These types represent the JSON structure returned by granary CLI commands
//! and can be used by any tool that needs to parse granary output.
//!
//! # Features
//!
//! - `sqlx`: Enables `sqlx::FromRow` derive for database integration.

pub mod artifact;
pub mod checkpoint;
pub mod comment;
pub mod event;
pub mod event_consumer;
pub mod global_config;
pub mod ids;
pub mod initiative;
pub mod project;
pub mod run;
pub mod search;
pub mod session;
pub mod task;
pub mod worker;

pub use artifact::*;
pub use checkpoint::*;
pub use comment::*;
pub use event::*;
pub use event_consumer::*;
pub use global_config::*;
pub use ids::*;
pub use initiative::*;
pub use project::*;
pub use run::*;
pub use search::*;
pub use session::*;
pub use task::*;
pub use worker::*;
