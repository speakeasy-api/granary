//! Granary - A CLI context hub for agentic work
//!
//! Granary's differentiator is that it can represent an ongoing agentic loop
//! as a first-class object, and generate machine-consumable context packs
//! on demand.

pub mod cli;
pub mod daemon;
pub mod db;
pub mod error;
pub mod models;
pub mod output;
pub mod services;

pub use error::{GranaryError, Result};
