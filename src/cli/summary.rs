use crate::error::Result;
use crate::output::{OutputFormat, json, prompt};
use crate::services::{self, Workspace};

/// Generate summary
pub async fn summary(token_budget: Option<usize>, format: OutputFormat) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let summary = services::generate_summary(&pool, &workspace, token_budget).await?;

    match format {
        OutputFormat::Json => {
            println!("{}", json::format_summary(&summary));
        }
        OutputFormat::Prompt => {
            println!("{}", prompt::format_summary(&summary));
        }
        _ => {
            // Table/default format
            print_summary_table(&summary);
        }
    }

    Ok(())
}

/// Generate context pack
pub async fn context(
    include: Option<String>,
    max_items: Option<usize>,
    format: OutputFormat,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let include_vec = include.map(|s| s.split(',').map(|s| s.trim().to_string()).collect());

    let context = services::generate_context(&pool, &workspace, include_vec, max_items).await?;

    match format {
        OutputFormat::Json => {
            println!("{}", json::format_context(&context));
        }
        OutputFormat::Prompt => {
            println!("{}", prompt::format_context(&context));
        }
        _ => {
            // Default to prompt format for context
            println!("{}", prompt::format_context(&context));
        }
    }

    Ok(())
}

/// Generate handoff document
pub async fn handoff(
    to: &str,
    tasks: &str,
    constraints: Option<String>,
    acceptance_criteria: Option<String>,
    format: OutputFormat,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    let task_ids: Vec<String> = tasks.split(',').map(|s| s.trim().to_string()).collect();

    let handoff = services::generate_handoff(
        &pool,
        to,
        &task_ids,
        constraints.as_deref(),
        acceptance_criteria.as_deref(),
        None,
    )
    .await?;

    match format {
        OutputFormat::Json => {
            println!("{}", json::format_handoff(&handoff));
        }
        OutputFormat::Prompt => {
            println!("{}", prompt::format_handoff(&handoff));
        }
        _ => {
            println!("{}", prompt::format_handoff(&handoff));
        }
    }

    Ok(())
}

fn print_summary_table(summary: &json::SummaryOutput) {
    println!("=== Summary ===\n");

    if let Some(session) = &summary.session {
        println!(
            "Session: {} ({})",
            session.name.as_deref().unwrap_or("-"),
            session.id
        );
        if let Some(mode) = &session.mode {
            println!("Mode: {}", mode);
        }
        if let Some(focus) = &session.focus_task_id {
            println!("Focus: {}", focus);
        }
        println!();
    }

    println!("State of Work:");
    println!("  Total tasks: {}", summary.state.total_tasks);
    println!(
        "  By status: {} todo, {} in progress, {} done, {} blocked",
        summary.state.by_status.todo,
        summary.state.by_status.in_progress,
        summary.state.by_status.done,
        summary.state.by_status.blocked
    );
    println!(
        "  By priority: {} P0, {} P1, {} P2, {} P3, {} P4",
        summary.state.by_priority.p0,
        summary.state.by_priority.p1,
        summary.state.by_priority.p2,
        summary.state.by_priority.p3,
        summary.state.by_priority.p4
    );
    println!();

    if let Some(focus) = &summary.focus_task {
        println!("Focus Task:");
        println!("  {} ({})", focus.title, focus.id);
        println!("  Status: {} | Priority: {}", focus.status, focus.priority);
        if let Some(desc) = &focus.description {
            println!("  {}", desc);
        }
        println!();
    }

    if !summary.blockers.is_empty() {
        println!("Blockers ({}):", summary.blockers.len());
        for task in &summary.blockers {
            print!("  - {} ({})", task.title, task.id);
            if let Some(reason) = &task.blocked_reason {
                print!(": {}", reason);
            }
            println!();
        }
        println!();
    }

    if !summary.next_actions.is_empty() {
        println!("Next Actions:");
        for task in &summary.next_actions {
            println!("  - [{}] {} ({})", task.priority, task.title, task.id);
        }
        println!();
    }

    if !summary.recent_decisions.is_empty() {
        println!("Recent Decisions:");
        for comment in &summary.recent_decisions {
            let author = comment.author.as_deref().unwrap_or("unknown");
            println!("  - {}: {}", author, comment.content);
        }
        println!();
    }
}
