use crate::cli::args::CheckpointAction;
use crate::error::{GranaryError, Result};
use crate::output::{Formatter, OutputFormat, json};
use crate::services::{self, Workspace};

/// Handle checkpoint subcommands
pub async fn checkpoint(action: CheckpointAction, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;
    let formatter = Formatter::new(format);

    let session_id = workspace
        .current_session_id()
        .ok_or(GranaryError::NoActiveSession)?;

    match action {
        CheckpointAction::Create { name } => {
            let checkpoint = services::create_checkpoint(&pool, &session_id, &name).await?;
            println!("Created checkpoint: {}", checkpoint.name);
            println!("{}", formatter.format_checkpoint(&checkpoint));
        }

        CheckpointAction::List => {
            let checkpoints = services::list_checkpoints(&pool, &session_id).await?;
            println!("{}", formatter.format_checkpoints(&checkpoints));
        }

        CheckpointAction::Diff { from, to } => {
            let diff = services::diff_checkpoints(&pool, &session_id, &from, &to).await?;

            match format {
                OutputFormat::Json => {
                    println!("{}", json::format_checkpoint_diff(&diff));
                }
                _ => {
                    println!("Diff: {} -> {}", diff.from, diff.to);
                    println!();
                    if diff.changes.is_empty() {
                        println!("No changes");
                    } else {
                        for change in &diff.changes {
                            println!(
                                "  {} {} .{}: {:?} -> {:?}",
                                change.entity_type,
                                change.entity_id,
                                change.field,
                                change.old_value,
                                change.new_value
                            );
                        }
                    }
                }
            }
        }

        CheckpointAction::Restore { name } => {
            services::restore_checkpoint(&pool, &session_id, &name).await?;
            println!("Restored checkpoint: {}", name);
        }
    }

    Ok(())
}
