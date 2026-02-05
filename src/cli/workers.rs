//! Workers list CLI command.
//!
//! Lists all workers across all workspaces from the global database.

use std::time::Duration;

use crate::cli::watch::{watch_loop, watch_status_line};
use crate::db;
use crate::error::Result;
use crate::models::WorkerStatus;
use crate::output::{Formatter, OutputFormat};
use crate::services::global_config_service;

/// List all workers with optional watch mode
pub async fn list_workers(
    all: bool,
    format: OutputFormat,
    watch: bool,
    interval: u64,
) -> Result<()> {
    if watch {
        let interval_duration = Duration::from_secs(interval);
        watch_loop(interval_duration, || async {
            let output = fetch_and_format_workers(all, format).await?;
            Ok(format!(
                "{}\n{}",
                watch_status_line(interval_duration),
                output
            ))
        })
        .await?;
    } else {
        let output = fetch_and_format_workers(all, format).await?;
        println!("{}", output);
    }

    Ok(())
}

/// Fetch workers and format them for display
async fn fetch_and_format_workers(all: bool, format: OutputFormat) -> anyhow::Result<String> {
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
            return Ok("No workers found.".to_string());
        } else {
            return Ok(
                "No active workers found.\nUse --all to include stopped/errored workers."
                    .to_string(),
            );
        }
    }

    let formatter = Formatter::new(format);
    Ok(formatter.format_workers(&workers))
}
