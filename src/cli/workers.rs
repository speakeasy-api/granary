//! Workers list CLI command.
//!
//! Lists all workers across all workspaces from the global database.

use crate::db;
use crate::error::Result;
use crate::models::worker::WorkerStatus;
use crate::output::{Formatter, OutputFormat};
use crate::services::global_config_service;

/// List all workers
pub async fn list_workers(all: bool, format: OutputFormat) -> Result<()> {
    let global_pool = global_config_service::global_pool().await?;

    let workers = db::workers::list(&global_pool).await?;

    // Filter out stopped/errored workers unless --all is specified
    let workers: Vec<_> = if all {
        workers
    } else {
        workers
            .into_iter()
            .filter(|w| {
                let status = w.status_enum();
                !matches!(status, WorkerStatus::Stopped | WorkerStatus::Error)
            })
            .collect()
    };

    if workers.is_empty() {
        if all {
            println!("No workers found.");
        } else {
            println!("No active workers found.");
            println!("Use --all to include stopped/errored workers.");
        }
        return Ok(());
    }

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_workers(&workers));

    Ok(())
}
