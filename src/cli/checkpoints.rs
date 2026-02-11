use crate::cli::args::{CheckpointAction, CliOutputFormat};
use crate::error::{GranaryError, Result};
use crate::models::Checkpoint;
use crate::output::{Output, json, prompt, table};
use crate::services::{self, Workspace};

// =============================================================================
// Output Types
// =============================================================================

/// Output for a single checkpoint
pub struct CheckpointOutput {
    pub checkpoint: Checkpoint,
}

impl Output for CheckpointOutput {
    fn to_json(&self) -> String {
        json::format_checkpoint(&self.checkpoint)
    }

    fn to_prompt(&self) -> String {
        prompt::format_checkpoint(&self.checkpoint)
    }

    fn to_text(&self) -> String {
        table::format_checkpoint(&self.checkpoint)
    }
}

/// Output for a list of checkpoints
pub struct CheckpointsOutput {
    pub checkpoints: Vec<Checkpoint>,
}

impl Output for CheckpointsOutput {
    fn to_json(&self) -> String {
        json::format_checkpoints(&self.checkpoints)
    }

    fn to_prompt(&self) -> String {
        prompt::format_checkpoints(&self.checkpoints)
    }

    fn to_text(&self) -> String {
        table::format_checkpoints(&self.checkpoints)
    }
}

/// Handle checkpoint subcommands
pub async fn checkpoint(
    action: CheckpointAction,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let session_id = workspace
        .current_session_id()
        .ok_or(GranaryError::NoActiveSession)?;

    match action {
        CheckpointAction::Create {
            name_positional,
            name_flag,
        } => {
            let name = name_positional.or(name_flag).ok_or_else(|| {
                GranaryError::InvalidArgument(
                    "Checkpoint name is required. Usage: granary checkpoint create <name>"
                        .to_string(),
                )
            })?;
            let checkpoint = services::create_checkpoint(&pool, &session_id, &name).await?;
            println!("Created checkpoint: {}", checkpoint.name);
            let output = CheckpointOutput { checkpoint };
            println!("{}", output.format(cli_format));
        }

        CheckpointAction::List => {
            let checkpoints = services::list_checkpoints(&pool, &session_id).await?;
            let output = CheckpointsOutput { checkpoints };
            println!("{}", output.format(cli_format));
        }

        CheckpointAction::Diff { from, to } => {
            let diff = services::diff_checkpoints(&pool, &session_id, &from, &to).await?;

            match cli_format {
                Some(CliOutputFormat::Json) => {
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
