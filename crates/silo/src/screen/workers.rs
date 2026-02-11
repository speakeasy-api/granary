//! Workers screen - workers list view with runners and active workers.
//!
//! This screen displays the workers management interface with:
//! - Header with title and new worker button
//! - Available Runners section showing configured runners from config
//! - Active Workers section showing running and stopped workers

use crate::appearance::{self, CORNER_RADIUS_SMALL, Palette};
use crate::message::Message;
use crate::widget;
use granary_types::{RunnerConfig, Worker, WorkerStatus};
use iced::border::Radius;
use iced::widget::{
    Column, Row, Space, button, column, container, horizontal_space, row, scrollable, text,
};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};
use lucide_icons::Icon;
use std::path::PathBuf;

/// State passed to the workers screen view function.
///
/// This struct contains all the data needed to render the workers screen,
/// borrowed from the main application state.
pub struct WorkersScreenState<'a> {
    pub runners: &'a [(String, RunnerConfig)],
    pub workers: &'a [Worker],
    pub workspace: Option<&'a PathBuf>,
    pub loading: bool,
}

/// Renders the workers screen with runners and workers sections.
pub fn view<'a>(state: WorkersScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(palette);
    let runners_section = view_runners_section(state.runners, palette);
    let workers_section = view_workers_section(state.workers, state.workspace, palette);

    let content = column![
        header,
        Space::with_height(24),
        runners_section,
        Space::with_height(24),
        workers_section,
    ]
    .padding(32)
    .width(Length::Fill)
    .height(Length::Fill);

    content.into()
}

/// Renders the header with title and new worker button.
fn view_header<'a>(palette: &'a Palette) -> Element<'a, Message> {
    let new_worker_btn = new_worker_button("+ New Worker", palette);

    widget::page_header("Workers", new_worker_btn, palette)
}

/// Renders the Available Runners section.
fn view_runners_section<'a>(
    runners: &'a [(String, RunnerConfig)],
    palette: &'a Palette,
) -> Element<'a, Message> {
    let header = text("Available Runners")
        .size(14)
        .color(palette.text_secondary);

    let content: Element<'a, Message> = if runners.is_empty() {
        container(
            text("No runners configured. Edit ~/.granary/config.toml")
                .size(14)
                .color(palette.text_muted),
        )
        .padding(24)
        .center_x(Length::Fill)
        .into()
    } else {
        let items: Vec<Element<'a, Message>> = runners
            .iter()
            .map(|(name, config)| runner_card(name, config, palette))
            .collect();

        scrollable(Column::from_vec(items).spacing(8).width(Length::Fill))
            .height(Length::FillPortion(1))
            .into()
    };

    let section = column![header, Space::with_height(12), content].width(Length::Fill);

    widget::card(section, palette)
}

/// Sort workers by status and creation time.
fn sort_workers<'a>(workers: &[&'a Worker]) -> Vec<&'a Worker> {
    let mut sorted: Vec<&Worker> = workers.to_vec();
    sorted.sort_by(|a, b| {
        let a_status = a.status_enum();
        let b_status = b.status_enum();

        // Running workers first
        let a_priority = match a_status {
            WorkerStatus::Running => 0,
            WorkerStatus::Pending => 1,
            WorkerStatus::Stopped => 2,
            WorkerStatus::Error => 3,
        };
        let b_priority = match b_status {
            WorkerStatus::Running => 0,
            WorkerStatus::Pending => 1,
            WorkerStatus::Stopped => 2,
            WorkerStatus::Error => 3,
        };

        a_priority
            .cmp(&b_priority)
            .then_with(|| b.created_at.cmp(&a.created_at))
    });
    sorted
}

/// Renders the Active Workers section split into local and global workers.
fn view_workers_section<'a>(
    workers: &'a [Worker],
    current_workspace: Option<&'a PathBuf>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let header = row![
        text("Active Workers")
            .size(14)
            .color(palette.text_secondary),
        horizontal_space(),
        widget::icon_button(Icon::RefreshCw, Message::RefreshWorkers, palette),
    ]
    .align_y(iced::Alignment::Center);

    let content: Element<'a, Message> = if workers.is_empty() {
        container(
            text("No active workers. Start one above.")
                .size(14)
                .color(palette.text_muted),
        )
        .padding(24)
        .center_x(Length::Fill)
        .into()
    } else {
        // Partition workers into local (current workspace) and global (other)
        let (local_workers, global_workers): (Vec<&Worker>, Vec<&Worker>) =
            workers.iter().partition(|w| {
                current_workspace
                    .map(|ws| w.instance_path == ws.to_string_lossy())
                    .unwrap_or(false)
            });

        let local_sorted = sort_workers(&local_workers);
        let global_sorted = sort_workers(&global_workers);

        let mut content_items: Vec<Element<'a, Message>> = Vec::new();

        // Current Workspace Workers section
        if !local_sorted.is_empty() {
            content_items.push(
                text("Current Workspace")
                    .size(12)
                    .color(palette.text_muted)
                    .into(),
            );
            content_items.push(Space::with_height(8).into());

            for worker in &local_sorted {
                content_items.push(worker_card(worker, true, palette));
            }
        }

        // Other Workers section
        if !global_sorted.is_empty() {
            if !local_sorted.is_empty() {
                content_items.push(Space::with_height(16).into());
            }

            content_items.push(
                text("Other Workspaces")
                    .size(12)
                    .color(palette.text_muted)
                    .into(),
            );
            content_items.push(Space::with_height(8).into());

            for worker in &global_sorted {
                content_items.push(worker_card(worker, false, palette));
            }
        }

        scrollable(
            Column::from_vec(content_items)
                .spacing(4)
                .width(Length::Fill),
        )
        .height(Length::FillPortion(2))
        .into()
    };

    let section = column![header, Space::with_height(12), content].width(Length::Fill);

    widget::card(section, palette)
}

/// Renders a runner card with name, command, and action buttons.
fn runner_card<'a>(
    name: &'a str,
    config: &'a RunnerConfig,
    palette: &'a Palette,
) -> Element<'a, Message> {
    // Status indicator (always green for available runners)
    let status_dot = container(Space::new(8, 8)).style(move |_| container::Style {
        background: Some(Background::Color(palette.status_done)),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Runner name
    let runner_name = text(name).size(14).color(palette.text);

    // Command line preview
    let command_preview = if config.args.is_empty() {
        config.command.clone()
    } else {
        format!("{} {}", config.command, config.args.join(" "))
    };
    let command_text = text(command_preview)
        .size(12)
        .color(palette.text_muted)
        .font(iced::Font::MONOSPACE);

    // Event type and concurrency info
    let event_type = config.on.as_deref().unwrap_or("task.next");
    let concurrency = config.concurrency.unwrap_or(1);
    let info_text = text(format!(
        "Event: {} • Concurrency: {}",
        event_type, concurrency
    ))
    .size(11)
    .color(palette.text_muted);

    // Action buttons
    let start_btn = action_button_primary(
        "▶ Start",
        Message::QuickStartRunner(name.to_string()),
        palette,
    );
    let customize_btn = widget::action_button(
        "Customize...",
        Message::OpenCustomizeRunner(name.to_string()),
        palette,
    );

    let content = column![
        row![status_dot, Space::with_width(8), runner_name].align_y(iced::Alignment::Center),
        command_text,
        info_text,
        Space::with_height(8),
        row![start_btn, customize_btn].spacing(8),
    ]
    .spacing(4)
    .width(Length::Fill);

    // Card styling
    let bg = palette.card;
    let bg_hover = palette.card_hover;
    let border = palette.border;
    let border_hover = palette.border_hover;

    button(container(content).padding(16).width(Length::Fill))
        .style(move |_, status| {
            let (bg_color, border_color) = match status {
                button::Status::Hovered => (bg_hover, border_hover),
                _ => (bg, border),
            };
            button::Style {
                background: Some(Background::Color(bg_color)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS),
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .into()
}

/// Renders a worker card with status, info, and action buttons.
///
/// # Arguments
/// - `worker`: The worker to display
/// - `is_local`: Whether this worker is in the current workspace (affects directory display)
/// - `palette`: The color palette
fn worker_card<'a>(
    worker: &'a Worker,
    is_local: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let status = worker.status_enum();

    // Status indicator color
    let status_color = match status {
        WorkerStatus::Running => palette.status_done,
        WorkerStatus::Pending => palette.warning,
        WorkerStatus::Stopped => palette.text_muted,
        WorkerStatus::Error => palette.status_blocked,
    };

    let status_dot = container(Space::new(8, 8)).style(move |_| container::Style {
        background: Some(Background::Color(status_color)),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Worker ID and runner name
    let runner_label = worker
        .runner_name
        .as_ref()
        .map(|r| format!(" ({})", r))
        .unwrap_or_default();
    let worker_name = text(format!("{}{}", worker.id, runner_label))
        .size(14)
        .color(palette.text);

    // Status badge
    let status_text = text(status.as_str()).size(11).color(status_color);

    // Command line preview
    let args: Vec<String> = serde_json::from_str(&worker.args).unwrap_or_default();
    let command_preview = if args.is_empty() {
        worker.command.clone()
    } else {
        format!("{} {}", worker.command, args.join(" "))
    };
    let command_text = text(command_preview)
        .size(12)
        .color(palette.text_muted)
        .font(iced::Font::MONOSPACE);

    // Directory path (show for non-local workers)
    let directory_element: Element<'a, Message> = if !is_local {
        text(format!("📁 {}", worker.instance_path))
            .size(11)
            .color(palette.text_muted)
            .font(iced::Font::MONOSPACE)
            .into()
    } else {
        Space::new(0, 0).into()
    };

    // Event type and concurrency info
    let info_text = text(format!(
        "Event: {} • Concurrency: {}",
        worker.event_type, worker.concurrency
    ))
    .size(11)
    .color(palette.text_muted);

    // Error message if present
    let error_element: Element<'a, Message> = if let Some(ref err) = worker.error_message {
        let error_color = palette.status_blocked;
        container(text(format!("Error: {}", err)).size(11).color(error_color))
            .padding(Padding::from([4, 8]))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgba(
                    error_color.r,
                    error_color.g,
                    error_color.b,
                    0.1,
                ))),
                border: Border {
                    radius: Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    } else {
        Space::new(0, 0).into()
    };

    // Action buttons based on status
    let worker_id = worker.id.clone();
    let mut action_btns: Vec<Element<'a, Message>> = Vec::new();

    // View Logs button
    action_btns.push(widget::action_button(
        "View Logs",
        Message::OpenWorkerLogs(worker_id.clone()),
        palette,
    ));

    // Status-specific buttons
    match status {
        WorkerStatus::Running => {
            // Stop button for running workers
            action_btns.push(action_button_danger(
                "Stop",
                Message::StopWorker(worker_id),
                palette,
            ));
        }
        WorkerStatus::Stopped | WorkerStatus::Error => {
            // Delete button for stopped/errored workers
            action_btns.push(action_button_danger(
                "Delete",
                Message::DeleteWorker(worker_id),
                palette,
            ));
        }
        WorkerStatus::Pending => {
            // No additional buttons for pending workers
        }
    }

    let actions_row = Row::from_vec(action_btns).spacing(8);

    let content = column![
        row![
            status_dot,
            Space::with_width(8),
            worker_name,
            horizontal_space(),
            status_text,
        ]
        .align_y(iced::Alignment::Center),
        command_text,
        directory_element,
        info_text,
        error_element,
        Space::with_height(8),
        actions_row,
    ]
    .spacing(4)
    .width(Length::Fill);

    // Card styling
    let bg = palette.card;
    let bg_hover = palette.card_hover;
    let border = palette.border;
    let border_hover = palette.border_hover;

    button(container(content).padding(16).width(Length::Fill))
        .style(move |_, btn_status| {
            let (bg_color, border_color) = match btn_status {
                button::Status::Hovered => (bg_hover, border_hover),
                _ => (bg, border),
            };
            button::Style {
                background: Some(Background::Color(bg_color)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS),
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .into()
}

/// Helper function to create the new worker button.
fn new_worker_button<'a>(label: &'a str, palette: &'a Palette) -> Element<'a, Message> {
    let text_color = palette.text;
    let bg_normal = palette.accent;
    let bg_hover = palette.border_hover;

    button(container(text(label).size(13.0).color(text_color)).padding(Padding::from([8, 16])))
        .on_press(Message::OpenStartWorker)
        .style(move |_: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => bg_hover,
                _ => bg_normal,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Primary action button (for start actions).
fn action_button_primary<'a, Message: Clone + 'a>(
    label: &'a str,
    msg: Message,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let text_color = palette.text;
    let bg_normal = palette.accent;
    let bg_hover = palette.border_hover;

    button(container(text(label).size(12).color(text_color)).padding(Padding::from([6, 14])))
        .on_press(msg)
        .style(move |_: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => bg_hover,
                _ => bg_normal,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Danger action button (for stop/delete actions).
fn action_button_danger<'a, Message: Clone + 'a>(
    label: &'a str,
    msg: Message,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let text_color = palette.danger_light;
    let bg_normal = palette.card;
    let bg_hover = palette.card_hover;
    let border_normal = palette.border;
    let border_hover = palette.danger;

    button(container(text(label).size(12).color(text_color)).padding(Padding::from([6, 14])))
        .on_press(msg)
        .style(move |_: &Theme, status| {
            let (bg, border) = match status {
                button::Status::Hovered => (bg_hover, border_hover),
                _ => (bg_normal, border_normal),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                },
                ..Default::default()
            }
        })
        .into()
}
