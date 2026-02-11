//! Workers list CLI command.
//!
//! Lists all workers across all workspaces from the global database.

use std::time::Duration;

use crate::cli::args::CliOutputFormat;
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::db;
use crate::error::Result;
use crate::models::{Worker, WorkerStatus};
use crate::output::{Output, json, table};
use crate::services::global_config_service;

// =============================================================================
// Output Types
// =============================================================================

/// Output for a list of workers
pub struct WorkersOutput {
    pub workers: Vec<Worker>,
    pub show_all_hint: bool,
}

impl Output for WorkersOutput {
    fn to_json(&self) -> String {
        json::format_workers(&self.workers)
    }

    fn to_prompt(&self) -> String {
        // Workers don't have a prompt formatter, use table format
        table::format_workers(&self.workers)
    }

    fn to_text(&self) -> String {
        if self.workers.is_empty() {
            if self.show_all_hint {
                return "No active workers found.\nUse --all to include stopped/errored workers."
                    .to_string();
            }
            return "No workers found.".to_string();
        }
        table::format_workers(&self.workers)
    }
}

/// Output for a single worker
pub struct WorkerOutput {
    pub worker: Worker,
}

impl Output for WorkerOutput {
    fn to_json(&self) -> String {
        json::format_worker(&self.worker)
    }

    fn to_prompt(&self) -> String {
        // Workers don't have a prompt formatter, use table format
        table::format_worker(&self.worker)
    }

    fn to_text(&self) -> String {
        table::format_worker(&self.worker)
    }
}

/// List all workers with optional watch mode
pub async fn list_workers(
    all: bool,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    if watch {
        let interval_duration = Duration::from_secs(interval);
        watch_loop(interval_duration, || async {
            let output = fetch_and_format_workers(all, cli_format).await?;
            Ok(format!(
                "{}\n{}",
                watch_status_line(interval_duration),
                output
            ))
        })
        .await?;
    } else {
        let output = fetch_and_format_workers(all, cli_format).await?;
        println!("{}", output);
    }

    Ok(())
}

/// Fetch workers and format them for display
async fn fetch_and_format_workers(
    all: bool,
    cli_format: Option<CliOutputFormat>,
) -> anyhow::Result<String> {
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

    let output = WorkersOutput {
        workers,
        show_all_hint: !all,
    };
    Ok(output.format(cli_format))
}
