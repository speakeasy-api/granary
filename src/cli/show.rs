use crate::db;
use crate::error::{GranaryError, Result};
use crate::output::{Formatter, OutputFormat};
use crate::services::{self, Workspace};

/// Detected entity type from an ID
#[derive(Debug, Clone, PartialEq)]
pub enum EntityKind {
    Initiative,
    Project,
    Task,
    Session,
    Checkpoint,
    Comment,
    Artifact,
}

/// Detect the entity type from an ID based on naming patterns
///
/// ID patterns:
/// - Session: `sess-<date>-<suffix>` e.g., `sess-20260111-7f2c`
/// - Checkpoint: `chkpt-<suffix>` e.g., `chkpt-abcd1234`
/// - Task: `<project_id>-task-<n>` e.g., `my-project-5h18-task-42`
/// - Comment: `<parent_id>-comment-<n>` e.g., `my-project-5h18-task-42-comment-3`
/// - Artifact: `<parent_id>-artifact-<n>` e.g., `my-project-5h18-task-42-artifact-1`
/// - Initiative: `<slug>-<4char suffix>` e.g., `my-initiative-5h18` (same pattern as Project)
/// - Project: `<slug>-<4char suffix>` e.g., `my-project-5h18`
///
/// Note: Since Initiative and Project share the same ID pattern, we cannot distinguish them
/// by pattern alone. The show function will try initiative first, then project.
pub fn detect_entity_kind(id: &str) -> EntityKind {
    if id.starts_with("sess-") {
        EntityKind::Session
    } else if id.starts_with("chkpt-") {
        EntityKind::Checkpoint
    } else if id.contains("-comment-") {
        EntityKind::Comment
    } else if id.contains("-artifact-") {
        EntityKind::Artifact
    } else if id.contains("-task-") {
        EntityKind::Task
    } else {
        // Could be initiative or project - they share the same ID pattern
        // Return Project as default, but show() will try Initiative first
        EntityKind::Project
    }
}

/// Show an entity by ID, auto-detecting its type
pub async fn show(id: &str, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;
    let formatter = Formatter::new(format);

    let kind = detect_entity_kind(id);

    match kind {
        EntityKind::Initiative => {
            // This case is used when explicitly looking up initiatives
            let initiative = services::get_initiative_or_error(&pool, id).await?;
            println!("{}", formatter.format_initiative(&initiative));
        }

        EntityKind::Project => {
            // Since Initiative and Project share the same ID pattern,
            // try Initiative first, then fall back to Project
            if let Some(initiative) = services::get_initiative(&pool, id).await? {
                println!("{}", formatter.format_initiative(&initiative));
            } else {
                let project = services::get_project(&pool, id).await?;
                println!("{}", formatter.format_project(&project));
            }
        }

        EntityKind::Task => {
            let (task, blocked_by) = services::get_task_with_deps(&pool, id).await?;
            println!("{}", formatter.format_task_with_deps(&task, blocked_by));
        }

        EntityKind::Session => {
            let session = services::get_session(&pool, id).await?;
            println!("{}", formatter.format_session(&session));

            // Also show scope
            let scope = services::get_scope(&pool, id).await?;
            if !scope.is_empty() {
                println!("\nScope:");
                for item in scope {
                    println!("  {} {}", item.item_type, item.item_id);
                }
            }
        }

        EntityKind::Checkpoint => {
            let checkpoint = services::get_checkpoint(&pool, id).await?;
            println!("{}", formatter.format_checkpoint(&checkpoint));
        }

        EntityKind::Comment => {
            let comment = db::comments::get(&pool, id)
                .await?
                .ok_or_else(|| GranaryError::CommentNotFound(id.to_string()))?;
            println!("{}", formatter.format_comment(&comment));
        }

        EntityKind::Artifact => {
            let artifact = db::artifacts::get(&pool, id)
                .await?
                .ok_or_else(|| GranaryError::ArtifactNotFound(id.to_string()))?;
            println!("{}", formatter.format_artifact(&artifact));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_entity_kind() {
        assert_eq!(
            detect_entity_kind("sess-20260111-7f2c"),
            EntityKind::Session
        );
        assert_eq!(detect_entity_kind("chkpt-abcd1234"), EntityKind::Checkpoint);
        assert_eq!(
            detect_entity_kind("my-project-5h18-task-42"),
            EntityKind::Task
        );
        assert_eq!(
            detect_entity_kind("my-project-5h18-task-42-comment-3"),
            EntityKind::Comment
        );
        assert_eq!(
            detect_entity_kind("my-project-5h18-task-42-artifact-1"),
            EntityKind::Artifact
        );
        // Initiative and Project share the same ID pattern (slug-suffix)
        // detect_entity_kind returns Project as default, show() handles the distinction
        assert_eq!(detect_entity_kind("my-project-5h18"), EntityKind::Project);
        assert_eq!(
            detect_entity_kind("my-initiative-5h18"),
            EntityKind::Project
        );
        assert_eq!(detect_entity_kind("general-qol-abjc"), EntityKind::Project);
    }
}
