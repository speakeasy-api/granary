use std::io::{self, BufRead, Read};

use crate::cli::args::CliOutputFormat;
use crate::error::Result;
use crate::output::Output;
use crate::services::batch_service::BatchResult;
use crate::services::{self, Workspace, batch_service::BatchRequest};

/// Output for batch/apply commands
pub struct BatchOutput {
    pub results: Vec<BatchResult>,
    pub success_count: usize,
    pub fail_count: usize,
}

impl Output for BatchOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.results).unwrap()
    }

    fn to_prompt(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Batch complete: {} succeeded, {} failed out of {} operations.",
            self.success_count,
            self.fail_count,
            self.results.len()
        ));

        if self.fail_count > 0 {
            lines.push("Failures:".to_string());
            for result in &self.results {
                if !result.success {
                    lines.push(format!(
                        "- {} (index {}): {}",
                        result.op,
                        result.index,
                        result.error.as_deref().unwrap_or("Unknown error")
                    ));
                }
            }
        }

        if self.success_count > 0 {
            lines.push("Successes:".to_string());
            for result in &self.results {
                if result.success {
                    if let Some(id) = &result.id {
                        lines.push(format!("- {} -> {}", result.op, id));
                    } else {
                        lines.push(format!("- {}", result.op));
                    }
                }
            }
        }

        lines.join("\n")
    }

    fn to_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Batch complete: {} succeeded, {} failed",
            self.success_count, self.fail_count
        ));

        for result in &self.results {
            if result.success {
                if let Some(id) = &result.id {
                    lines.push(format!("  [OK] {} -> {}", result.op, id));
                } else {
                    lines.push(format!("  [OK] {}", result.op));
                }
            } else {
                lines.push(format!(
                    "  [ERR] {}: {}",
                    result.op,
                    result.error.as_deref().unwrap_or("Unknown error")
                ));
            }
        }

        lines.join("\n")
    }
}

/// Output for batch (JSONL) command - only shows errors in text mode
pub struct BatchStreamOutput {
    pub results: Vec<BatchResult>,
    pub success_count: usize,
    pub fail_count: usize,
}

impl Output for BatchStreamOutput {
    fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.results).unwrap()
    }

    fn to_prompt(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Batch complete: {} succeeded, {} failed out of {} operations.",
            self.success_count,
            self.fail_count,
            self.results.len()
        ));

        if self.fail_count > 0 {
            lines.push("Failures:".to_string());
            for result in &self.results {
                if !result.success {
                    lines.push(format!(
                        "- {} (index {}): {}",
                        result.op,
                        result.index,
                        result.error.as_deref().unwrap_or("Unknown error")
                    ));
                }
            }
        }

        lines.join("\n")
    }

    fn to_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "Batch complete: {} succeeded, {} failed",
            self.success_count, self.fail_count
        ));

        for result in &self.results {
            if !result.success {
                lines.push(format!(
                    "  [ERR] {}: {}",
                    result.op,
                    result.error.as_deref().unwrap_or("Unknown error")
                ));
            }
        }

        lines.join("\n")
    }
}

/// Apply a batch of operations from JSON
pub async fn apply(stdin: bool, format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let input = if stdin {
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;
        buffer
    } else {
        return Err(crate::error::GranaryError::InvalidArgument(
            "Use --stdin to read from stdin".to_string(),
        ));
    };

    let request: BatchRequest = serde_json::from_str(&input)?;
    let results = services::apply_batch(&pool, &request).await?;

    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = results.len() - success_count;

    let output = BatchOutput {
        results,
        success_count,
        fail_count,
    };
    println!("{}", output.format(format));

    Ok(())
}

/// Process a batch of operations from JSONL (one JSON object per line)
pub async fn batch(stdin: bool, format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    if !stdin {
        return Err(crate::error::GranaryError::InvalidArgument(
            "Use --stdin to read from stdin".to_string(),
        ));
    }

    let stdin = io::stdin();
    let mut all_results = Vec::new();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Each line is a single operation
        let op: services::batch_service::BatchOp = serde_json::from_str(&line)?;
        let request = BatchRequest { ops: vec![op] };
        let results = services::apply_batch(&pool, &request).await?;
        all_results.extend(results);
    }

    let success_count = all_results.iter().filter(|r| r.success).count();
    let fail_count = all_results.len() - success_count;

    let output = BatchStreamOutput {
        results: all_results,
        success_count,
        fail_count,
    };
    println!("{}", output.format(format));

    Ok(())
}
