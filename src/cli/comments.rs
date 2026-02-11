use crate::cli::args::CliOutputFormat;
use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::*;
use crate::output::{Output, json, prompt, table};
use crate::services::Workspace;

// =============================================================================
// Output Types
// =============================================================================

/// Output for a single comment
pub struct CommentOutput {
    pub comment: Comment,
}

impl Output for CommentOutput {
    fn to_json(&self) -> String {
        json::format_comment(&self.comment)
    }

    fn to_prompt(&self) -> String {
        prompt::format_comment(&self.comment)
    }

    fn to_text(&self) -> String {
        table::format_comment(&self.comment)
    }
}

/// Output for a list of comments
pub struct CommentsOutput {
    pub comments: Vec<Comment>,
}

impl Output for CommentsOutput {
    fn to_json(&self) -> String {
        json::format_comments(&self.comments)
    }

    fn to_prompt(&self) -> String {
        prompt::format_comments(&self.comments)
    }

    fn to_text(&self) -> String {
        table::format_comments(&self.comments)
    }
}

/// Show a comment by ID
pub async fn show_comment(id: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let comment = db::comments::get(&pool, id)
        .await?
        .ok_or_else(|| GranaryError::CommentNotFound(id.to_string()))?;

    let output = CommentOutput { comment };
    println!("{}", output.format(cli_format));

    Ok(())
}

/// Update a comment
pub async fn update_comment(
    id: &str,
    content: Option<String>,
    kind: Option<String>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let mut comment = db::comments::get(&pool, id)
        .await?
        .ok_or_else(|| GranaryError::CommentNotFound(id.to_string()))?;

    if let Some(c) = content {
        comment.content = c;
    }
    if let Some(k) = kind
        && let Ok(ck) = k.parse::<CommentKind>()
    {
        comment.kind = ck.as_str().to_string();
    }

    let updated = db::comments::update(&pool, &comment).await?;
    if !updated {
        return Err(GranaryError::VersionMismatch {
            expected: comment.version,
            found: comment.version + 1,
        });
    }

    let updated_comment = db::comments::get(&pool, id)
        .await?
        .ok_or_else(|| GranaryError::CommentNotFound(id.to_string()))?;

    let output = CommentOutput {
        comment: updated_comment,
    };
    println!("{}", output.format(cli_format));

    Ok(())
}
