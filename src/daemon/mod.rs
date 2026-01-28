//! Daemon module for background process management.
//!
//! This module provides the IPC protocol and types for communicating between
//! the CLI and a long-running daemon process that manages workers and runs.
//!
//! ## Components
//!
//! - [`protocol`]: Request/Response types and length-delimited JSON framing
//! - [`listener`]: Unix socket listener for accepting CLI connections
//! - [`worker_manager`]: Worker lifecycle management (start/stop/query workers)
//! - [`client`]: DaemonClient for CLI-to-daemon communication
//! - [`auto_start`]: Auto-start logic to ensure daemon is running

pub mod auto_start;
pub mod client;
pub mod listener;
pub mod protocol;
pub mod worker_manager;

pub use auto_start::ensure_daemon;
pub use client::DaemonClient;
pub use listener::{IpcConnection, IpcListener};
pub use protocol::*;
pub use worker_manager::WorkerManager;
