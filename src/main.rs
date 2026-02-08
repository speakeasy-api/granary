use clap::Parser;
use std::process::ExitCode;

use granary::cli::args::{Cli, Commands};
use granary::cli::{
    batch, checkpoints, config, daemon, entrypoint, events, init, initiate, initiatives, plan,
    projects, run, search, sessions, show, summary, tasks, update, work, worker, workers,
    workspace,
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
    let format_override = cli.output_format_override();

    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            entrypoint::show_entry_point().await?;
            return Ok(());
        }
    };

    match command {
        Commands::Init {
            local,
            force,
            skip_git_check,
        } => {
            if local {
                // --local: delegate to workspace init --local (same behavior as before)
                workspace::workspace_init(true, force, skip_git_check, None, format_override)
                    .await?;
            } else {
                // Default: generate a random workspace name (e.g. workspace_a3f1k)
                let random_suffix = nanoid::nanoid!(5, &nanoid::alphabet::SAFE);
                let generated_name = format!("workspace_{}", random_suffix);
                workspace::workspace_init(
                    false,
                    force,
                    skip_git_check,
                    Some(generated_name),
                    format_override,
                )
                .await?;
            }
        }

        Commands::Workspace { action } => {
            workspace::workspace(action, format_override).await?;
        }

        Commands::Workspaces => {
            workspace::workspace_list(format_override).await?;
        }

        Commands::Doctor => {
            init::doctor().await?;
        }

        Commands::Plan { name, project } => {
            plan::plan(name.as_deref(), project, format_override).await?;
        }

        Commands::Work { command } => {
            work::work(command, format_override).await?;
        }

        Commands::Show { id } => {
            show::show(&id, format_override).await?;
        }

        Commands::Projects { action, all } => {
            projects::projects(action, all, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Project { id, action } => {
            // Handle "projects create <name>" shorthand
            if id == "create" {
                return Err(GranaryError::InvalidArgument(
                    "To create a project, use: granary project create --name \"Project Name\""
                        .to_string(),
                ));
            }
            projects::project(&id, action, format_override).await?;
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
                format_override,
                cli.watch,
                cli.interval,
            )
            .await?;
        }

        Commands::Task { id, action } => {
            tasks::task(&id, action, format_override).await?;
        }

        Commands::Next {
            include_reason,
            all,
        } => {
            tasks::next_task(include_reason, all, format_override).await?;
        }

        Commands::Start {
            task_id,
            owner,
            lease,
        } => {
            tasks::start_task(&task_id, owner, lease, format_override).await?;
        }

        Commands::Focus { task_id } => {
            tasks::focus_task(&task_id, format_override).await?;
        }

        Commands::Pin { task_id } => {
            tasks::pin_task(&task_id).await?;
        }

        Commands::Unpin { task_id } => {
            tasks::unpin_task(&task_id).await?;
        }

        Commands::Sessions { all } => {
            sessions::list_sessions(all, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Session { action } => {
            sessions::session(action, format_override).await?;
        }

        Commands::Summary { token_budget } => {
            summary::summary(token_budget, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Context { include, max_items } => {
            summary::context(include, max_items, format_override).await?;
        }

        Commands::Checkpoint { action } => {
            checkpoints::checkpoint(action, format_override).await?;
        }

        Commands::Handoff {
            to,
            tasks,
            constraints,
            acceptance_criteria,
        } => {
            summary::handoff(
                &to,
                &tasks,
                constraints,
                acceptance_criteria,
                format_override,
            )
            .await?;
        }

        Commands::Apply { stdin } => {
            batch::apply(stdin, format_override).await?;
        }

        Commands::Batch { stdin } => {
            batch::batch(stdin, format_override).await?;
        }

        Commands::Config { action } => {
            config::config(action, format_override).await?;
        }

        Commands::Steering { action } => {
            config::steering(action, format_override).await?;
        }

        Commands::Search { query } => {
            search::search(&query, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Initiatives { action, all } => {
            initiatives::initiatives(action, all, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Initiative { id, action } => {
            initiatives::initiative(&id, action, format_override).await?;
        }

        Commands::Initiate { name, description } => {
            initiate::initiate(&name, description, format_override).await?;
        }

        Commands::Update { check, to } => {
            update::update(check, to).await?;
        }

        Commands::Workers { all } => {
            workers::list_workers(all, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Worker { command } => {
            worker::worker(command, format_override).await?;
        }

        Commands::Runs {
            worker,
            status,
            all,
            limit,
        } => {
            run::list_runs(
                worker,
                status,
                all,
                limit,
                format_override,
                cli.watch,
                cli.interval,
            )
            .await?;
        }

        Commands::Run { command } => {
            run::run(command, format_override).await?;
        }

        Commands::Events {
            action,
            event_type,
            entity,
            since,
            limit,
        } => match action {
            Some(granary::cli::args::EventsAction::Drain { before }) => {
                events::drain_events(&before, format_override).await?;
            }
            None => {
                events::list_events(
                    event_type,
                    entity,
                    since,
                    limit,
                    format_override,
                    cli.watch,
                    cli.interval,
                )
                .await?;
            }
        },

        Commands::Daemon { command } => {
            daemon::daemon(command).await?;
        }
    }

    Ok(())
}
