use clap::Parser;
use std::process::ExitCode;

use granary::cli::args::{Cli, Commands};
use granary::cli::{
    batch, checkpoints, config, init, projects, search, sessions, show, summary, tasks,
};
use granary::error::{GranaryError, exit_codes};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let result = run(cli).await;

    match result {
        Ok(()) => ExitCode::from(exit_codes::SUCCESS as u8),
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

async fn run(cli: Cli) -> granary::Result<()> {
    let format = cli.output_format();

    match cli.command {
        Commands::Init => {
            init::init().await?;
        }

        Commands::Doctor => {
            init::doctor().await?;
        }

        Commands::Show { id } => {
            show::show(&id, format).await?;
        }

        Commands::Projects { action, all } => {
            projects::projects(action, all, format).await?;
        }

        Commands::Project { id, action } => {
            // Handle "projects create <name>" shorthand
            if id == "create" {
                return Err(GranaryError::InvalidArgument(
                    "To create a project, use: granary project create --name \"Project Name\""
                        .to_string(),
                ));
            }
            projects::project(&id, action, format).await?;
        }

        Commands::Tasks {
            all,
            status,
            priority,
            owner,
        } => {
            tasks::list_tasks(all, status, priority, owner, format).await?;
        }

        Commands::Task { id, action } => {
            tasks::task(&id, action, format).await?;
        }

        Commands::Next {
            include_reason,
            all,
        } => {
            tasks::next_task(include_reason, all, format).await?;
        }

        Commands::Start {
            task_id,
            owner,
            lease,
        } => {
            tasks::start_task(&task_id, owner, lease, format).await?;
        }

        Commands::Focus { task_id } => {
            tasks::focus_task(&task_id, format).await?;
        }

        Commands::Pin { task_id } => {
            tasks::pin_task(&task_id).await?;
        }

        Commands::Unpin { task_id } => {
            tasks::unpin_task(&task_id).await?;
        }

        Commands::Sessions { all } => {
            sessions::list_sessions(all, format).await?;
        }

        Commands::Session { action } => {
            sessions::session(action, format).await?;
        }

        Commands::Summary { token_budget } => {
            summary::summary(token_budget, format).await?;
        }

        Commands::Context { include, max_items } => {
            summary::context(include, max_items, format).await?;
        }

        Commands::Checkpoint { action } => {
            checkpoints::checkpoint(action, format).await?;
        }

        Commands::Handoff {
            to,
            tasks,
            constraints,
            acceptance_criteria,
        } => {
            summary::handoff(&to, &tasks, constraints, acceptance_criteria, format).await?;
        }

        Commands::Apply { stdin } => {
            batch::apply(stdin, format).await?;
        }

        Commands::Batch { stdin } => {
            batch::batch(stdin, format).await?;
        }

        Commands::Config { action } => {
            config::config(action, format).await?;
        }

        Commands::Steering { action } => {
            config::steering(action, format).await?;
        }

        Commands::Search { query } => {
            search::search(&query, format).await?;
        }
    }

    Ok(())
}
