//! Runs screen - list all runs with filtering and detail panel
//!
//! Displays runs from workers with status indicators, filtering by worker
//! and status, and a detail panel for the selected run.

use crate::appearance::{CORNER_RADIUS_SMALL, Palette};
use crate::message::Message;
use crate::widget::{self, icon};
use granary_types::{Run, RunStatus};
use iced::border::Radius;
use iced::widget::{
    Column, Row, Space, button, column, container, horizontal_space, pick_list, row, scrollable,
    text,
};
use iced::{Background, Border, Color, Element, Length, Padding as IcedPadding};
use lucide_icons::Icon;

/// State needed to render the runs screen
pub struct RunsScreenState<'a> {
    /// All runs to display
    pub runs: &'a [Run],
    /// Currently selected run (for detail panel)
    pub selected_run: Option<&'a Run>,
    /// Filter by worker ID (None = show all)
    pub worker_filter: Option<&'a String>,
    /// Filter by status (None = show all)
    pub status_filter: Option<&'a String>,
    /// Whether data is being loaded
    pub loading: bool,
}

/// Available status options for filtering (display labels)
const STATUS_OPTIONS: &[&str] = &[
    "All",
    "Pending",
    "Running",
    "Completed",
    "Failed",
    "Paused",
    "Cancelled",
];

/// Renders the runs screen with two-panel layout.
///
/// Left panel (1/3 width): Runs list with filter controls at top
/// Right panel (2/3 width): Selected run detail (or placeholder if none selected)
pub fn view<'a>(state: RunsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let runs_panel = view_runs_panel(&state, palette);
    let detail_panel = view_detail_panel(&state, palette);

    let main_content = row![
        container(runs_panel).width(Length::FillPortion(1)),
        container(detail_panel).width(Length::FillPortion(2)),
    ]
    .spacing(24)
    .height(Length::Fill);

    let content = column![header, main_content]
        .spacing(24)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill);

    content.into()
}

/// Renders the header with title, loading indicator, and refresh button.
fn view_header<'a>(state: &RunsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let loading_indicator = if state.loading {
        text("syncing...")
            .size(12)
            .color(palette.accent)
            .font(iced::Font::MONOSPACE)
    } else {
        text("").size(12)
    };

    let trailing = row![
        loading_indicator,
        Space::with_width(16),
        widget::icon_button(Icon::RefreshCw, Message::RefreshRuns, palette),
    ]
    .align_y(iced::Alignment::Center);

    widget::page_header("Runs", trailing, palette)
}

/// Renders the runs list panel with filters.
fn view_runs_panel<'a>(state: &RunsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    // Filter controls
    let filters = view_filters(state, palette);

    // Runs list
    let runs_list: Element<'a, Message> = if state.runs.is_empty() {
        container(text("No runs found").size(14).color(palette.text_muted))
            .padding(24)
            .center_x(Length::Fill)
            .into()
    } else {
        // Filter runs based on current filters
        let filtered_runs: Vec<&Run> = state
            .runs
            .iter()
            .filter(|r| {
                // Apply worker filter
                if let Some(worker_id) = state.worker_filter
                    && &r.worker_id != worker_id
                {
                    return false;
                }
                // Apply status filter
                if let Some(status) = state.status_filter
                    && &r.status != status
                {
                    return false;
                }
                true
            })
            .collect();

        if filtered_runs.is_empty() {
            container(
                text("No runs match filters")
                    .size(14)
                    .color(palette.text_muted),
            )
            .padding(24)
            .center_x(Length::Fill)
            .into()
        } else {
            let selected_id = state.selected_run.map(|r| &r.id);
            let items: Vec<Element<'a, Message>> = filtered_runs
                .iter()
                .map(|r| view_run_item(r, selected_id, palette))
                .collect();

            scrollable(Column::from_vec(items).spacing(4).width(Length::Fill))
                .height(Length::Fill)
                .into()
        }
    };

    let panel = column![filters, Space::with_height(12), runs_list]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    widget::card(panel, palette)
}

/// Convert display label to stored status value
fn status_label_to_value(label: &str) -> Option<String> {
    match label {
        "All" => None,
        "Pending" => Some("pending".to_string()),
        "Running" => Some("running".to_string()),
        "Completed" => Some("completed".to_string()),
        "Failed" => Some("failed".to_string()),
        "Paused" => Some("paused".to_string()),
        "Cancelled" => Some("cancelled".to_string()),
        _ => None,
    }
}

/// Convert stored status value to display label
fn status_value_to_label(value: &str) -> &'static str {
    match value {
        "pending" => "Pending",
        "running" => "Running",
        "completed" => "Completed",
        "failed" => "Failed",
        "paused" => "Paused",
        "cancelled" => "Cancelled",
        _ => "All",
    }
}

/// Renders the filter dropdowns.
fn view_filters<'a>(state: &RunsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    // Collect unique worker IDs from runs
    let worker_ids: Vec<String> = {
        let mut ids: Vec<String> = state.runs.iter().map(|r| r.worker_id.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    };

    let worker_options: Vec<String> = worker_ids;
    let status_options: Vec<&str> = STATUS_OPTIONS.to_vec();

    let selected_worker = state.worker_filter.cloned();
    // Convert stored status value to display label for the pick_list
    let selected_status_label: &str = state
        .status_filter
        .as_ref()
        .map(|s| status_value_to_label(s))
        .unwrap_or("All");

    let accent = palette.accent;
    let border = palette.border;
    let bg = palette.input;
    let text_color = palette.text;
    let text_muted = palette.text_muted;

    // Worker filter dropdown
    let worker_picker: Element<'a, Message> =
        pick_list(worker_options.clone(), selected_worker, |w| {
            Message::FilterRunsByWorker(Some(w))
        })
        .placeholder("All workers")
        .text_size(12)
        .padding(8)
        .style(move |_, status| {
            let border_color = match status {
                pick_list::Status::Hovered | pick_list::Status::Opened => accent,
                _ => border,
            };
            pick_list::Style {
                background: Background::Color(bg),
                text_color,
                placeholder_color: text_muted,
                handle_color: text_muted,
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                },
            }
        })
        .into();

    // Status filter dropdown - always shows a selected value (defaults to "All")
    let status_picker: Element<'a, Message> =
        pick_list(status_options, Some(selected_status_label), |s| {
            Message::FilterRunsByStatus(status_label_to_value(s))
        })
        .text_size(12)
        .padding(8)
        .style(move |_, status| {
            let border_color = match status {
                pick_list::Status::Hovered | pick_list::Status::Opened => accent,
                _ => border,
            };
            pick_list::Style {
                background: Background::Color(bg),
                text_color,
                placeholder_color: text_muted,
                handle_color: text_muted,
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                },
            }
        })
        .into();

    // Clear filters button (only show if filters are active)
    let clear_btn: Element<'a, Message> =
        if state.worker_filter.is_some() || state.status_filter.is_some() {
            let text_color = palette.text_muted;
            let hover_bg = palette.card_hover;
            button(text("Clear").size(11).color(text_color))
                .on_press(Message::FilterRunsByWorker(None))
                .padding(IcedPadding::from([6, 10]))
                .style(move |_, btn_status| {
                    let bg = match btn_status {
                        button::Status::Hovered => hover_bg,
                        _ => Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            radius: Radius::from(4.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                })
                .into()
        } else {
            Space::new(0, 0).into()
        };

    row![
        text("Filters:").size(12).color(palette.text_secondary),
        Space::with_width(8),
        worker_picker,
        Space::with_width(8),
        status_picker,
        horizontal_space(),
        clear_btn,
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

/// Renders a single run item in the list.
///
/// Format: run-abc123    worker-xyz...    task.next    proj-task-5    [status_icon] Running
fn view_run_item<'a>(
    run: &'a Run,
    selected_id: Option<&'a String>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let is_selected = selected_id == Some(&run.id);
    let status = run.status_enum();

    // Run ID (truncated)
    let run_id_display = if run.id.len() > 12 {
        format!("{}...", &run.id[..12])
    } else {
        run.id.clone()
    };

    // Worker ID (truncated)
    let worker_display = if run.worker_id.len() > 12 {
        format!("{}...", &run.worker_id[..12])
    } else {
        run.worker_id.clone()
    };

    // Entity ID (truncated)
    let entity_display = if run.entity_id.len() > 14 {
        format!("{}...", &run.entity_id[..14])
    } else {
        run.entity_id.clone()
    };

    // Status icon and text
    let (lucide_icon, icon_color) = status_icon(status, palette);
    let status_text = status.as_str();

    let content = row![
        text(run_id_display)
            .size(11)
            .color(palette.text_secondary)
            .font(iced::Font::MONOSPACE),
        Space::with_width(8),
        text(worker_display)
            .size(11)
            .color(palette.text_muted)
            .font(iced::Font::MONOSPACE),
        Space::with_width(8),
        text(&run.event_type).size(11).color(palette.text_secondary),
        Space::with_width(8),
        text(entity_display)
            .size(11)
            .color(palette.text_muted)
            .font(iced::Font::MONOSPACE),
        Space::with_width(8),
        text(format!("{}/{}", run.attempt, run.max_attempts))
            .size(11)
            .color(palette.text_muted)
            .font(iced::Font::MONOSPACE),
        horizontal_space(),
        icon(lucide_icon).size(12).color(icon_color),
        Space::with_width(4),
        text(status_text).size(11).color(icon_color),
    ]
    .align_y(iced::Alignment::Center);

    let bg = if is_selected {
        palette.card_hover
    } else {
        palette.card
    };
    let bg_hover = palette.card_hover;
    let border_color = if is_selected {
        palette.accent
    } else {
        palette.border
    };
    let border_hover = palette.border_hover;

    button(
        container(content)
            .padding(IcedPadding::from([8, 12]))
            .width(Length::Fill),
    )
    .on_press(Message::SelectRun(run.id.clone()))
    .style(move |_, btn_status| {
        let (background, border) = match btn_status {
            button::Status::Hovered => (bg_hover, border_hover),
            _ => (bg, border_color),
        };
        button::Style {
            background: Some(Background::Color(background)),
            border: Border {
                color: border,
                width: 1.0,
                radius: Radius::from(CORNER_RADIUS_SMALL),
            },
            ..Default::default()
        }
    })
    .width(Length::Fill)
    .into()
}

/// Returns the status icon and color for a run status.
fn status_icon(status: RunStatus, palette: &Palette) -> (Icon, Color) {
    match status {
        RunStatus::Pending => (Icon::Circle, palette.text_muted),
        RunStatus::Running => (Icon::CircleDot, palette.accent),
        RunStatus::Completed => (Icon::CircleCheck, palette.status_done),
        RunStatus::Failed => (Icon::CircleX, palette.status_blocked),
        RunStatus::Paused => (Icon::CirclePause, palette.status_progress),
        RunStatus::Cancelled => (Icon::Ban, palette.text_muted),
    }
}

/// Renders the detail panel for the selected run.
fn view_detail_panel<'a>(
    state: &RunsScreenState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = match state.selected_run {
        Some(run) => view_run_detail(run, palette),
        None => view_no_selection(palette),
    };

    widget::card(content, palette)
}

/// Renders a placeholder when no run is selected.
fn view_no_selection<'a>(palette: &'a Palette) -> Element<'a, Message> {
    container(
        column![
            text("No run selected")
                .size(16)
                .color(palette.text_secondary),
            Space::with_height(8),
            text("Select a run from the list to view details")
                .size(13)
                .color(palette.text_muted),
        ]
        .align_x(iced::Alignment::Center),
    )
    .padding(48)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

/// Renders the detail view for a selected run.
fn view_run_detail<'a>(run: &'a Run, palette: &'a Palette) -> Element<'a, Message> {
    let status = run.status_enum();
    let (lucide_icon, icon_color) = status_icon(status, palette);

    // Header with ID and status
    let header = row![
        text(&run.id)
            .size(18)
            .color(palette.text)
            .font(iced::Font::MONOSPACE),
        horizontal_space(),
        icon(lucide_icon).size(16).color(icon_color),
        Space::with_width(6),
        text(status.as_str()).size(14).color(icon_color),
    ]
    .align_y(iced::Alignment::Center);

    // Detail sections
    let mut sections: Vec<Element<'a, Message>> = vec![header.into()];

    // Worker info
    sections.push(view_detail_row("Worker", &run.worker_id, palette, true).into());

    // Event info
    sections.push(view_detail_row("Event Type", &run.event_type, palette, false).into());
    sections.push(
        row![
            text("Event ID:").size(12).color(palette.text_muted),
            Space::with_width(8),
            text(run.event_id.to_string())
                .size(12)
                .color(palette.text_secondary)
                .font(iced::Font::MONOSPACE),
        ]
        .into(),
    );
    sections.push(view_detail_row("Entity", &run.entity_id, palette, true).into());

    // PID (if running)
    if let Some(pid) = run.pid {
        sections.push(
            row![
                text("PID:").size(12).color(palette.text_muted),
                Space::with_width(8),
                text(pid.to_string())
                    .size(12)
                    .color(palette.text_secondary)
                    .font(iced::Font::MONOSPACE),
            ]
            .into(),
        );
    }

    // Command info
    let bg_code = palette.background;
    let command_display = container(
        column![
            text("Command").size(11).color(palette.text_muted),
            container(
                text(&run.command)
                    .size(12)
                    .color(palette.text_secondary)
                    .font(iced::Font::MONOSPACE)
            )
            .padding(IcedPadding::from([8, 12]))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(bg_code)),
                border: Border {
                    radius: Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ]
        .spacing(4),
    );
    sections.push(command_display.into());

    // Args
    let args = run.args_vec();
    if !args.is_empty() {
        let args_display = args.join(" ");
        let bg_args = palette.background;
        let args_section = container(
            column![
                text("Arguments").size(11).color(palette.text_muted),
                container(
                    text(args_display)
                        .size(12)
                        .color(palette.text_secondary)
                        .font(iced::Font::MONOSPACE)
                )
                .padding(IcedPadding::from([8, 12]))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(bg_args)),
                    border: Border {
                        radius: Radius::from(4.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ]
            .spacing(4),
        );
        sections.push(args_section.into());
    }

    // Exit code (if present)
    if let Some(exit_code) = run.exit_code {
        let exit_color = if exit_code == 0 {
            palette.status_done
        } else {
            palette.status_blocked
        };
        sections.push(
            row![
                text("Exit Code:").size(12).color(palette.text_muted),
                Space::with_width(8),
                text(exit_code.to_string()).size(12).color(exit_color),
            ]
            .into(),
        );
    }

    // Error message (if present)
    if let Some(error) = &run.error_message {
        let bg_error = Color::from_rgba(
            palette.status_blocked.r,
            palette.status_blocked.g,
            palette.status_blocked.b,
            0.1,
        );
        let error_section = container(
            row![
                text("✗").size(12).color(palette.status_blocked),
                Space::with_width(6),
                text(format!("Error: {}", error))
                    .size(12)
                    .color(palette.status_blocked),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(IcedPadding::from([8, 12]))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg_error)),
            border: Border {
                radius: Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        });
        sections.push(error_section.into());
    }

    // Retry info
    sections.push(
        row![
            text("Attempt:").size(11).color(palette.text_muted),
            Space::with_width(4),
            text(format!("{} / {}", run.attempt, run.max_attempts))
                .size(11)
                .color(palette.text_secondary),
        ]
        .into(),
    );

    if let Some(next_retry) = &run.next_retry_at {
        sections.push(
            row![
                text("Next retry:").size(11).color(palette.text_muted),
                Space::with_width(4),
                text(next_retry).size(11).color(palette.accent),
            ]
            .into(),
        );
    }

    // Timestamps
    let mut timestamps: Vec<Element<'a, Message>> = vec![
        row![
            text("Created:").size(10).color(palette.text_muted),
            Space::with_width(4),
            text(&run.created_at).size(10).color(palette.text_muted),
        ]
        .into(),
    ];

    if let Some(started) = &run.started_at {
        timestamps.push(
            row![
                text("Started:").size(10).color(palette.text_muted),
                Space::with_width(4),
                text(started).size(10).color(palette.status_progress),
            ]
            .into(),
        );
    }

    if let Some(completed) = &run.completed_at {
        timestamps.push(
            row![
                text("Completed:").size(10).color(palette.text_muted),
                Space::with_width(4),
                text(completed).size(10).color(palette.status_done),
            ]
            .into(),
        );
    }

    sections.push(Row::from_vec(timestamps).spacing(16).into());

    // Action buttons
    let mut action_btns: Vec<Element<'a, Message>> = Vec::new();

    // View logs button
    action_btns.push(widget::action_button(
        "View Logs",
        Message::OpenRunLogs(run.id.clone()),
        palette,
    ));

    // Status-dependent actions
    match status {
        RunStatus::Running => {
            action_btns.push(widget::action_button(
                "Pause",
                Message::PauseRun(run.id.clone()),
                palette,
            ));
            action_btns.push(widget::action_button(
                "Stop",
                Message::StopRun(run.id.clone()),
                palette,
            ));
        }
        RunStatus::Paused => {
            action_btns.push(widget::action_button(
                "Resume",
                Message::ResumeRun(run.id.clone()),
                palette,
            ));
            action_btns.push(widget::action_button(
                "Stop",
                Message::StopRun(run.id.clone()),
                palette,
            ));
        }
        _ => {}
    }

    let actions_row = Row::from_vec(action_btns).spacing(8);

    sections.push(Space::with_height(8).into());
    sections.push(actions_row.into());

    scrollable(Column::from_vec(sections).spacing(12).width(Length::Fill))
        .height(Length::Fill)
        .into()
}

/// Helper to render a detail row with label and value.
fn view_detail_row<'a>(
    label: &'a str,
    value: &'a str,
    palette: &'a Palette,
    monospace: bool,
) -> Row<'a, Message> {
    let value_text = if monospace {
        text(value)
            .size(12)
            .color(palette.text_secondary)
            .font(iced::Font::MONOSPACE)
    } else {
        text(value).size(12).color(palette.text_secondary)
    };

    row![
        text(format!("{}:", label))
            .size(12)
            .color(palette.text_muted),
        Space::with_width(8),
        value_text,
    ]
}
