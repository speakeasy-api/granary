use clap::Parser;
use std::process::ExitCode;

use granary::cli::args::{Cli, Commands};
use granary::cli::{
    batch, checkpoints, config, daemon, entrypoint, events, init, initiate, initiatives, plan,
    projects, run, search, sessions, show, summary, tasks, update, work, worker, workspace,
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
            entrypoint::show_entry_point(format_override).await?;
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

        Commands::Doctor { fix } => {
            init::doctor(fix, format_override).await?;
        }

        Commands::Plan {
            name,
            name_flag,
            project,
        } => {
            let resolved_name = name.or(name_flag);
            plan::plan(resolved_name.as_deref(), project, format_override).await?;
        }

        Commands::Work { command } => {
            work::work(command, format_override).await?;
        }

        Commands::Show { id } => {
            show::show(&id, format_override).await?;
        }

        Commands::Project {
            id: None,
            action: None,
            all,
        } => {
            projects::list(all, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Project {
            id: None,
            action: Some(action),
            all: _,
        } => {
            projects::project_action_without_id(action, format_override).await?;
        }

        Commands::Project {
            id: Some(id),
            action,
            all: _,
        } => {
            projects::project(&id, action, format_override).await?;
        }

        Commands::Task {
            id: None,
            action: None,
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

        Commands::Task {
            id: Some(id),
            action,
            ..
        } => {
            tasks::task(&id, action, format_override).await?;
        }

        Commands::Task {
            id: None,
            action: Some(_),
            ..
        } => {
            return Err(GranaryError::InvalidArgument(
                "Task ID is required when specifying an action".to_string(),
            ));
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

        Commands::Session { action: None, all } => {
            sessions::list_sessions(all, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Session {
            action: Some(action),
            all: _,
        } => {
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

        Commands::Initiative { id, action, all } => {
            initiatives::initiative(id, action, all, format_override, cli.watch, cli.interval)
                .await?;
        }

        Commands::Initiate {
            name_positional,
            name_flag,
            description,
        } => {
            let name = name_positional.or(name_flag).ok_or_else(|| {
                GranaryError::InvalidArgument(
                    "Initiative name is required. Usage: granary initiate <name>".to_string(),
                )
            })?;
            initiate::initiate(&name, description, format_override).await?;
        }

        Commands::Update { check, to } => {
            update::update(check, to, format_override).await?;
        }

        Commands::Worker { id, command, all } => {
            worker::worker(id, command, all, format_override, cli.watch, cli.interval).await?;
        }

        Commands::Run {
            id,
            command,
            worker,
            status,
            all,
            limit,
        } => {
            run::run(
                id,
                command,
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
            daemon::daemon(command, format_override).await?;
        }
    }

    Ok(())
}
