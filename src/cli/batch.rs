use std::io::{self, BufRead, Read};

use crate::cli::args::CliOutputFormat;
use crate::error::Result;
use crate::services::{self, Workspace, batch_service::BatchRequest};

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

    match format {
        Some(CliOutputFormat::Json) => {
            println!("{}", serde_json::to_string_pretty(&results)?);
        }
        _ => {
            let success_count = results.iter().filter(|r| r.success).count();
            let fail_count = results.len() - success_count;

            println!(
                "Batch complete: {} succeeded, {} failed",
                success_count, fail_count
            );

            for result in &results {
                if result.success {
                    if let Some(id) = &result.id {
                        println!("  [OK] {} -> {}", result.op, id);
                    } else {
                        println!("  [OK] {}", result.op);
                    }
                } else {
                    println!(
                        "  [ERR] {}: {}",
                        result.op,
                        result.error.as_deref().unwrap_or("Unknown error")
                    );
                }
            }
        }
    }

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

    match format {
        Some(CliOutputFormat::Json) => {
            println!("{}", serde_json::to_string_pretty(&all_results)?);
        }
        _ => {
            let success_count = all_results.iter().filter(|r| r.success).count();
            let fail_count = all_results.len() - success_count;

            println!(
                "Batch complete: {} succeeded, {} failed",
                success_count, fail_count
            );

            for result in &all_results {
                if !result.success {
                    println!(
                        "  [ERR] {}: {}",
                        result.op,
                        result.error.as_deref().unwrap_or("Unknown error")
                    );
                }
            }
        }
    }

    Ok(())
}
