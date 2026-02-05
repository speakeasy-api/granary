use clap::Parser;
use std::process::ExitCode;

use granary::cli::args::{Cli, Commands};
use granary::cli::{
    batch, checkpoints, config, daemon, entrypoint, init, initiate, initiatives, plan, projects,
    run, search, sessions, show, summary, tasks, update, work, worker, workers,
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

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            entrypoint::show_entry_point().await?;
            return Ok(());
        }
    };

    match command {
        Commands::Init => {
            init::init().await?;
        }

        Commands::Doctor => {
            init::doctor().await?;
        }

        Commands::Plan { name, project } => {
            plan::plan(name.as_deref(), project).await?;
        }

        Commands::Work { command } => {
            work::work(command).await?;
        }

        Commands::Show { id } => {
            show::show(&id, format).await?;
        }

        Commands::Projects { action, all } => {
            projects::projects(action, all, format, cli.watch, cli.interval).await?;
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
            tasks::list_tasks(
                all,
                status,
                priority,
                owner,
                format,
                cli.watch,
                cli.interval,
            )
            .await?;
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
            sessions::list_sessions(all, format, cli.watch, cli.interval).await?;
        }

        Commands::Session { action } => {
            sessions::session(action, format).await?;
        }

        Commands::Summary { token_budget } => {
            summary::summary(token_budget, format, cli.watch, cli.interval).await?;
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
            search::search(&query, format, cli.watch, cli.interval).await?;
        }

        Commands::Initiatives { action, all } => {
            initiatives::initiatives(action, all, format, cli.watch, cli.interval).await?;
        }

        Commands::Initiative { id, action } => {
            initiatives::initiative(&id, action, format).await?;
        }

        Commands::Initiate { name, description } => {
            initiate::initiate(&name, description).await?;
        }

        Commands::Update { check, to } => {
            update::update(check, to).await?;
        }

        Commands::Workers { all } => {
            workers::list_workers(all, format, cli.watch, cli.interval).await?;
        }

        Commands::Worker { command } => {
            worker::worker(command, format).await?;
        }

        Commands::Runs {
            worker,
            status,
            all,
            limit,
        } => {
            run::list_runs(worker, status, all, limit, format, cli.watch, cli.interval).await?;
        }

        Commands::Run { command } => {
            run::run(command, format).await?;
        }

        Commands::Daemon { command } => {
            daemon::daemon(command).await?;
        }
    }

    Ok(())
}
