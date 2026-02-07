use std::time::Duration;

use crate::cli::args::CliOutputFormat;
use crate::cli::watch::{watch_loop, watch_status_line};
use crate::error::Result;
use crate::models::SearchResult;
use crate::output::{Output, json, prompt, table};
use crate::services::{self, Workspace};

// =============================================================================
// Output Types
// =============================================================================

/// Output for search results
pub struct SearchOutput {
    pub results: Vec<SearchResult>,
}

impl Output for SearchOutput {
    fn to_json(&self) -> String {
        json::format_search_results(&self.results)
    }

    fn to_prompt(&self) -> String {
        prompt::format_search_results(&self.results)
    }

    fn to_text(&self) -> String {
        table::format_search_results(&self.results)
    }
}

/// Handle search command
pub async fn search(
    query: &str,
    cli_format: Option<CliOutputFormat>,
    watch: bool,
    interval: u64,
) -> Result<()> {
    if watch {
        let interval_duration = Duration::from_secs(interval);
        let query = query.to_string();

        watch_loop(interval_duration, || async {
            let output = fetch_and_format_search(&query, cli_format).await?;
            Ok(format!(
                "{}\n\n{}",
                watch_status_line(interval_duration),
                output
            ))
        })
        .await?;
    } else {
        let output = fetch_and_format_search(query, cli_format).await?;
        println!("{}", output);
    }

    Ok(())
}

/// Fetch search results and format them for display
async fn fetch_and_format_search(
    query: &str,
    cli_format: Option<CliOutputFormat>,
) -> Result<String> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let results = services::search(&pool, query).await?;
    let output = SearchOutput { results };
    Ok(output.format(cli_format))
}
