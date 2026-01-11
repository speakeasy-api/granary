use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::*;
use crate::output::{Formatter, OutputFormat};
use crate::services::Workspace;

/// Show a comment by ID
pub async fn show_comment(id: &str, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let comment = db::comments::get(&pool, id)
        .await?
        .ok_or_else(|| GranaryError::CommentNotFound(id.to_string()))?;

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_comment(&comment));

    Ok(())
}

/// Update a comment
pub async fn update_comment(
    id: &str,
    content: Option<String>,
    kind: Option<String>,
    format: OutputFormat,
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

    let formatter = Formatter::new(format);
    println!("{}", formatter.format_comment(&updated_comment));

    Ok(())
}
