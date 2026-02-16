//! Project detail screen with task list.
//!
//! This screen displays full project information and its associated tasks
//! with expandable task rows for details.

use crate::appearance::{self, Palette};
use crate::message::Message;
use crate::screen::Screen;
use crate::widget::{self, TaskStats, icon};
use granary_types::{Project, Task as GranaryTask, TaskPriority, TaskStatus};
use iced::border::Radius;
use iced::widget::{
    Column, Space, button, column, container, horizontal_space, row, scrollable, text,
};
use iced::{Background, Border, Color, Element, Length, Padding};
use lucide_icons::Icon;
use std::collections::HashSet;

/// State passed to the project detail screen view function.
///
/// This struct contains all the data needed to render the project detail screen,
/// borrowed from the main application state.
pub struct ProjectDetailState<'a> {
    /// The project being displayed
    pub project: &'a Project,
    /// List of tasks for this project
    pub tasks: &'a [GranaryTask],
    /// Set of expanded task IDs
    pub expanded_tasks: &'a HashSet<String>,
    /// New task title input value
    pub new_task_title: &'a str,
    /// Whether data is currently loading
    pub loading: bool,
    /// Status/error message to display
    pub status_message: Option<&'a String>,
}

/// Renders the project detail screen with project info and task list.
pub fn view<'a>(state: ProjectDetailState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let project_info = view_project_info(&state, palette);
    let tasks_section = view_tasks_section(&state, palette);

    let content = column![
        header,
        Space::with_height(24),
        project_info,
        Space::with_height(24),
        tasks_section,
    ]
    .spacing(0)
    .padding(32)
    .width(Length::Fill)
    .height(Length::Fill);

    content.into()
}

/// Renders the header with back button, project name, status badge, and archive/unarchive button.
fn view_header<'a>(state: &ProjectDetailState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    // Back button
    let back_btn = view_back_button(palette);

    // Project name
    let project_name = text(state.project.name.clone())
        .size(24)
        .color(palette.text);

    // Status badge (active/archived)
    let is_active = state.project.status_enum().is_active();
    let (status_label, status_color) = if is_active {
        ("active", palette.status_done)
    } else {
        ("archived", palette.text_muted)
    };
    let status_badge = view_status_badge(status_label, status_color, palette);

    // Loading/error indicators
    let loading_indicator = if state.loading {
        text("syncing...")
            .size(12)
            .color(palette.accent)
            .font(iced::Font::MONOSPACE)
    } else {
        text("").size(12)
    };

    let error_msg = if let Some(err) = state.status_message {
        text(err.as_str()).size(12).color(palette.status_blocked)
    } else {
        text("").size(12)
    };

    // Edit button
    let edit_btn = widget::action_button(
        "Edit",
        Message::ShowEditProject(state.project.id.clone()),
        palette,
    );

    // Archive/Unarchive button based on current status
    let archive_btn = if is_active {
        widget::icon_button(
            Icon::Archive,
            Message::ArchiveProject(state.project.id.clone()),
            palette,
        )
    } else {
        widget::icon_button(
            Icon::ArchiveRestore,
            Message::UnarchiveProject(state.project.id.clone()),
            palette,
        )
    };

    let refresh_btn = widget::icon_button(Icon::RefreshCw, Message::RefreshTasks, palette);

    row![
        back_btn,
        Space::with_width(16),
        project_name,
        Space::with_width(12),
        status_badge,
        horizontal_space(),
        column![loading_indicator, error_msg]
            .spacing(4)
            .align_x(iced::Alignment::End),
        Space::with_width(16),
        edit_btn,
        Space::with_width(8),
        archive_btn,
        Space::with_width(8),
        refresh_btn,
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

/// Renders the back button that returns to projects list.
fn view_back_button<'a>(palette: &'a Palette) -> Element<'a, Message> {
    let text_color = palette.text_secondary;
    let hover_bg = palette.card_hover;
    let accent = palette.accent;

    button(
        row![
            icon(Icon::ArrowLeft).size(14).color(text_color),
            text(" Projects").size(14).color(text_color)
        ]
        .spacing(4),
    )
    .on_press(Message::GoBack)
    .style(move |_, status| {
        let (bg, txt) = match status {
            button::Status::Hovered => (hover_bg, accent),
            _ => (Color::TRANSPARENT, text_color),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: txt,
            border: Border {
                radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .padding(Padding::from([6, 12]))
    .into()
}

/// Renders a status badge pill.
fn view_status_badge<'a>(
    label: &'a str,
    color: Color,
    _palette: &'a Palette,
) -> Element<'a, Message> {
    container(
        text(label)
            .size(10)
            .color(color)
            .font(iced::Font::MONOSPACE),
    )
    .padding(Padding::from([3, 8]))
    .style(move |_| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            color.r, color.g, color.b, 0.15,
        ))),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Renders the project information section.
fn view_project_info<'a>(
    state: &ProjectDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let project = state.project;
    let mut info_items: Vec<Element<'a, Message>> = Vec::new();

    // Project ID
    info_items.push(
        row![
            text("ID:").size(11).color(palette.text_muted),
            text(project.id.clone())
                .size(11)
                .color(palette.text_secondary)
                .font(iced::Font::MONOSPACE),
        ]
        .spacing(8)
        .into(),
    );

    // Full description (not truncated)
    if let Some(desc) = &project.description
        && !desc.is_empty()
    {
        info_items.push(
            column![
                text("Description").size(11).color(palette.text_muted),
                text(desc.clone()).size(13).color(palette.text_secondary),
            ]
            .spacing(4)
            .into(),
        );
    }

    // Owner
    if let Some(owner) = &project.owner
        && !owner.is_empty()
    {
        info_items.push(
            row![
                text("Owner:").size(11).color(palette.text_muted),
                text(owner.clone()).size(11).color(palette.text_secondary),
            ]
            .spacing(8)
            .into(),
        );
    }

    // Tags
    let tags = project.tags_vec();
    if !tags.is_empty() {
        let tags_row = view_tags_row(&tags, palette);
        info_items.push(
            row![text("Tags:").size(11).color(palette.text_muted), tags_row]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
        );
    }

    // Created/Updated dates
    info_items.push(
        row![
            text("Created:").size(10).color(palette.text_muted),
            text(format_date(&project.created_at))
                .size(10)
                .color(palette.text_muted),
            Space::with_width(16),
            text("Updated:").size(10).color(palette.text_muted),
            text(format_date(&project.updated_at))
                .size(10)
                .color(palette.text_muted),
        ]
        .spacing(4)
        .into(),
    );

    // Task progress bar
    let stats = calculate_task_stats(state.tasks);
    if stats.total > 0 {
        info_items.push(view_progress_section(&stats, palette));
    }

    let info_column = Column::from_vec(info_items).spacing(12);

    widget::card(info_column, palette)
}

/// Calculate task statistics from the task list.
fn calculate_task_stats(tasks: &[GranaryTask]) -> TaskStats {
    let mut stats = TaskStats {
        total: tasks.len() as u32,
        ..Default::default()
    };

    for task in tasks {
        match task.status_enum() {
            TaskStatus::Done => stats.done += 1,
            TaskStatus::InProgress => stats.in_progress += 1,
            _ => {}
        }
    }

    stats
}

/// Render progress section with bar and stats.
fn view_progress_section<'a>(stats: &TaskStats, palette: &'a Palette) -> Element<'a, Message> {
    let progress = stats.progress();
    let track_bg = palette.card;
    let fill_bg = palette.accent;

    // Progress bar track with fill - ensure FillPortion is at least 1
    let fill_portion = ((progress * 100.0) as u16).max(1);
    let progress_bar = container(
        container("")
            .width(Length::FillPortion(fill_portion))
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: if progress > 0.0 {
                    Some(Background::Color(fill_bg))
                } else {
                    None
                },
                border: Border {
                    radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                    ..Default::default()
                },
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(6.0))
    .style(move |_| container::Style {
        background: Some(Background::Color(track_bg)),
        border: Border {
            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
            ..Default::default()
        },
        ..Default::default()
    });

    // Stats text
    let stats_text = text(format!(
        "{}/{} done{}",
        stats.done,
        stats.total,
        if stats.in_progress > 0 {
            format!(", {} in progress", stats.in_progress)
        } else {
            String::new()
        }
    ))
    .size(11)
    .color(palette.text_secondary);

    column![
        row![
            text("Progress").size(11).color(palette.text_muted),
            horizontal_space(),
            stats_text,
        ]
        .align_y(iced::Alignment::Center),
        progress_bar,
    ]
    .spacing(6)
    .into()
}

/// Render tags as horizontal pills.
fn view_tags_row<'a>(tags: &[String], palette: &'a Palette) -> Element<'a, Message> {
    let accent = palette.accent;

    let tag_pills: Vec<Element<'a, Message>> = tags
        .iter()
        .map(|tag| {
            container(
                text(tag.clone())
                    .size(10)
                    .color(accent)
                    .font(iced::Font::MONOSPACE),
            )
            .padding(Padding::from([3, 8]))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgba(
                    accent.r, accent.g, accent.b, 0.15,
                ))),
                border: Border {
                    radius: Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
        })
        .collect();

    let mut tags_row = row![].spacing(6);
    for pill in tag_pills {
        tags_row = tags_row.push(pill);
    }

    tags_row.into()
}

/// Renders the tasks section with task list.
fn view_tasks_section<'a>(
    state: &ProjectDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let header = row![
        text("Tasks").size(14).color(palette.text_secondary),
        horizontal_space(),
        widget::action_button(
            "+ Add Task",
            Message::OpenCreateTaskScreen {
                project_id: Some(state.project.id.clone()),
            },
            palette,
        ),
        Space::with_width(8),
        widget::icon_button(Icon::RefreshCw, Message::RefreshTasks, palette),
    ]
    .align_y(iced::Alignment::Center);

    // Tasks list
    let tasks_list = view_tasks_list(state, palette);

    let content = column![header, Space::with_height(16), tasks_list,]
        .width(Length::Fill)
        .height(Length::Fill);

    widget::card(content, palette)
}

/// Renders the tasks list (empty state or scrollable list).
fn view_tasks_list<'a>(
    state: &ProjectDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    if state.tasks.is_empty() {
        container(
            column![
                text("No tasks yet").size(16).color(palette.text_secondary),
                Space::with_height(8),
                text("Create your first task above")
                    .size(13)
                    .color(palette.text_muted),
            ]
            .align_x(iced::Alignment::Center),
        )
        .padding(48)
        .center_x(Length::Fill)
        .into()
    } else {
        let items: Vec<Element<'a, Message>> = state
            .tasks
            .iter()
            .map(|t| view_task_row(t, state.expanded_tasks, palette))
            .collect();

        scrollable(Column::from_vec(items).spacing(6).width(Length::Fill))
            .height(Length::Fill)
            .into()
    }
}

/// Renders a single task row with status, priority, and expandable details.
fn view_task_row<'a>(
    task: &'a GranaryTask,
    expanded_tasks: &'a HashSet<String>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let title = task.title.clone();
    let task_id = task.id.clone();
    let status = task.status_enum();
    let priority = task.priority_enum();
    let is_expanded = expanded_tasks.contains(&task_id);

    let (status_color, status_label): (Color, &'static str) = match status {
        TaskStatus::Done => (palette.status_done, "done"),
        TaskStatus::InProgress => (palette.status_progress, "in progress"),
        TaskStatus::Blocked => (palette.status_blocked, "blocked"),
        TaskStatus::Draft => (palette.text_muted, "draft"),
        TaskStatus::Todo => (palette.status_todo, "todo"),
    };

    let priority_color = match priority {
        TaskPriority::P0 => palette.status_blocked,
        TaskPriority::P1 => palette.accent,
        _ => palette.text_muted,
    };
    let priority_label = priority.as_str();

    let status_badge = container(
        text(status_label)
            .size(10)
            .color(status_color)
            .font(iced::Font::MONOSPACE),
    )
    .padding(Padding::from([3, 8]))
    .style(move |_| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            status_color.r,
            status_color.g,
            status_color.b,
            0.15,
        ))),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    });

    let priority_badge = container(
        text(priority_label)
            .size(10)
            .color(priority_color)
            .font(iced::Font::MONOSPACE),
    )
    .padding(Padding::from([3, 6]));

    // Expand/collapse indicator
    let expand_indicator = icon(if is_expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    })
    .size(10)
    .color(palette.text_muted);

    let header_row = row![expand_indicator, text(title).size(14).color(palette.text),]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    let badges_row = row![status_badge, priority_badge,]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    let action_btn: Element<'a, Message> = match status {
        TaskStatus::Todo | TaskStatus::Draft => {
            widget::action_button("Start", Message::StartTask(task_id.clone()), palette)
        }
        TaskStatus::InProgress => {
            widget::action_button("Complete", Message::CompleteTask(task_id.clone()), palette)
        }
        _ => Space::new(0, 0).into(),
    };

    // Build the content based on expanded state
    let content: Element<'a, Message> = if is_expanded {
        // Expanded view with full details
        let mut details: Vec<Element<'a, Message>> = Vec::new();

        // Description
        if let Some(desc) = &task.description
            && !desc.is_empty()
        {
            details.push(
                column![
                    text("Description").size(11).color(palette.text_muted),
                    text(desc.clone()).size(13).color(palette.text_secondary),
                ]
                .spacing(4)
                .into(),
            );
        }

        // Owner
        if let Some(owner) = &task.owner {
            details.push(
                row![
                    text("Owner:").size(11).color(palette.text_muted),
                    text(owner.clone()).size(11).color(palette.text_secondary),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Tags
        if let Some(tags) = &task.tags
            && !tags.is_empty()
        {
            details.push(
                row![
                    text("Tags:").size(11).color(palette.text_muted),
                    text(tags.clone()).size(11).color(palette.accent),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Blocked reason
        if let Some(reason) = &task.blocked_reason
            && !reason.is_empty()
        {
            details.push(
                row![
                    text("Blocked:").size(11).color(palette.status_blocked),
                    text(reason.clone()).size(11).color(palette.status_blocked),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Due date
        if let Some(due) = &task.due_at {
            details.push(
                row![
                    text("Due:").size(11).color(palette.text_muted),
                    text(due.clone()).size(11).color(palette.text_secondary),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Started at
        if let Some(started) = &task.started_at {
            details.push(
                row![
                    text("Started:").size(11).color(palette.text_muted),
                    text(started.clone()).size(11).color(palette.text_secondary),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Completed at
        if let Some(completed) = &task.completed_at {
            details.push(
                row![
                    text("Completed:").size(11).color(palette.text_muted),
                    text(completed.clone()).size(11).color(palette.status_done),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Workers
        let worker_ids = task.worker_ids_vec();
        if !worker_ids.is_empty() {
            let mut worker_row_items: Vec<Element<'a, Message>> = Vec::new();
            worker_row_items.push(text("Workers:").size(11).color(palette.text_muted).into());
            for wid in worker_ids {
                let accent = palette.accent;
                let nav_screen = Screen::WorkerDetail { id: wid.clone() };
                worker_row_items.push(
                    button(text(wid).size(11).color(accent).font(iced::Font::MONOSPACE))
                        .on_press(Message::Navigate(nav_screen))
                        .padding(0)
                        .style(move |_, _| button::Style {
                            background: None,
                            ..Default::default()
                        })
                        .into(),
                );
            }
            details.push(
                iced::widget::Row::from_vec(worker_row_items)
                    .spacing(8)
                    .into(),
            );
        }

        // Runs
        let run_ids = task.run_ids_vec();
        if !run_ids.is_empty() {
            let mut run_row_items: Vec<Element<'a, Message>> = Vec::new();
            run_row_items.push(text("Runs:").size(11).color(palette.text_muted).into());
            for rid in run_ids {
                let accent = palette.accent;
                let nav_screen = Screen::RunDetail { id: rid.clone() };
                run_row_items.push(
                    button(text(rid).size(11).color(accent).font(iced::Font::MONOSPACE))
                        .on_press(Message::Navigate(nav_screen))
                        .padding(0)
                        .style(move |_, _| button::Style {
                            background: None,
                            ..Default::default()
                        })
                        .into(),
                );
            }
            details.push(iced::widget::Row::from_vec(run_row_items).spacing(8).into());
        }

        // Task ID (always show in expanded view)
        details.push(
            row![
                text("ID:").size(10).color(palette.text_muted),
                text(task_id.clone())
                    .size(10)
                    .color(palette.text_muted)
                    .font(iced::Font::MONOSPACE),
            ]
            .spacing(8)
            .into(),
        );

        // Created/Updated
        let bg_secondary = palette.background;
        details.push(
            row![
                text("Created:").size(10).color(palette.text_muted),
                text(task.created_at.clone())
                    .size(10)
                    .color(palette.text_muted),
                Space::with_width(16),
                text("Updated:").size(10).color(palette.text_muted),
                text(task.updated_at.clone())
                    .size(10)
                    .color(palette.text_muted),
            ]
            .spacing(4)
            .into(),
        );

        let details_section = if details.is_empty() {
            column![
                text("No additional details")
                    .size(12)
                    .color(palette.text_muted)
            ]
        } else {
            Column::from_vec(details).spacing(8)
        };

        column![
            row![
                column![header_row, Space::with_height(4), badges_row,].width(Length::Fill),
                action_btn,
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            Space::with_height(12),
            container(details_section)
                .padding(Padding::from([12, 16]))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(bg_secondary)),
                    border: Border {
                        radius: Radius::from(6.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .into()
    } else {
        // Collapsed view
        row![
            column![header_row, Space::with_height(4), badges_row,].width(Length::Fill),
            action_btn,
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .into()
    };

    let bg_card = palette.card;
    let bg_card_hover = palette.card_hover;
    let border_color = palette.border;
    let border_hover = palette.border_hover;

    button(container(content).padding(16).width(Length::Fill))
        .on_press(Message::ToggleTaskExpand(task_id))
        .style(move |_, status| {
            let (bg, border) = match status {
                button::Status::Hovered => (bg_card_hover, border_hover),
                _ => (bg_card, border_color),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS),
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .into()
}

/// Format an ISO timestamp to just the date portion.
fn format_date(timestamp: &str) -> &str {
    // ISO timestamps are like "2024-01-15T10:30:00Z"
    // Just take the date part
    if timestamp.len() >= 10 {
        &timestamp[..10]
    } else {
        timestamp
    }
}
