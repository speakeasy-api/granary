//! Initiative detail screen - single initiative with projects and tasks.
//!
//! This screen displays a single initiative with its associated projects and tasks,
//! including progress tracking and blocker information.

use crate::appearance::{self, Palette};
use crate::message::Message;
use crate::widget::{self, icon};
use granary_types::{
    Initiative, InitiativeBlockerInfo, InitiativeStatus, InitiativeSummary, ProjectSummary,
    Task as GranaryTask,
};
use iced::border::Radius;
use iced::widget::{
    Column, Space, button, column, container, horizontal_space, row, scrollable, text,
};
use iced::{Background, Border, Color, Element, Length, Padding};
use lucide_icons::Icon;
use std::collections::HashSet;

/// State for initiative detail view.
pub struct InitiativeDetailState<'a> {
    pub initiative: &'a Initiative,
    pub summary: Option<&'a InitiativeSummary>,
    pub tasks: &'a [GranaryTask],
    pub expanded_tasks: &'a HashSet<String>,
    pub loading: bool,
    pub status_message: Option<&'a String>,
}

/// Renders the initiative detail screen.
pub fn view<'a>(state: InitiativeDetailState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let info_section = view_info_section(&state, palette);
    let progress_section = view_progress_section(&state, palette);
    let projects_section = view_projects_section(&state, palette);
    let blockers_section = view_blockers_section(&state, palette);
    let tasks_section = view_tasks_section(&state, palette);

    let content = column![
        header,
        Space::with_height(24),
        info_section,
        Space::with_height(24),
        progress_section,
        Space::with_height(24),
        projects_section,
        Space::with_height(24),
        blockers_section,
        Space::with_height(24),
        tasks_section,
    ]
    .spacing(0)
    .padding(32)
    .width(Length::Fill);

    scrollable(content).height(Length::Fill).into()
}

/// Renders the header with back button, initiative name, status badge, and archive button.
fn view_header<'a>(
    state: &InitiativeDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    // Back button
    let back_btn = view_back_button(palette);

    // Initiative name
    let initiative_name = text(state.initiative.name.clone())
        .size(24)
        .color(palette.text);

    // Status badge
    let status = state.initiative.status_enum();
    let (status_label, status_color) = match status {
        InitiativeStatus::Active => ("active", palette.status_done),
        InitiativeStatus::Archived => ("archived", palette.text_muted),
    };
    let status_badge = view_status_badge(status_label, status_color);

    // Loading indicator
    let loading_indicator = if state.loading {
        text("syncing...")
            .size(12)
            .color(palette.accent)
            .font(iced::Font::MONOSPACE)
    } else {
        text("").size(12)
    };

    // Error message
    let error_msg = if let Some(err) = state.status_message {
        text(err.as_str()).size(12).color(palette.status_blocked)
    } else {
        text("").size(12)
    };

    // Archive button
    let archive_btn = widget::icon_button(
        Icon::Archive,
        Message::ArchiveInitiative(state.initiative.id.clone()),
        palette,
    );

    let refresh_btn = widget::icon_button(Icon::RefreshCw, Message::RefreshInitiatives, palette);

    row![
        back_btn,
        Space::with_width(16),
        initiative_name,
        Space::with_width(12),
        status_badge,
        horizontal_space(),
        column![loading_indicator, error_msg]
            .spacing(4)
            .align_x(iced::Alignment::End),
        Space::with_width(16),
        archive_btn,
        Space::with_width(8),
        refresh_btn,
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

/// Renders the back button that navigates to initiatives list.
fn view_back_button<'a>(palette: &'a Palette) -> Element<'a, Message> {
    let text_color = palette.text_secondary;
    let hover_bg = palette.card_hover;
    let accent = palette.accent;

    button(
        row![
            icon(Icon::ArrowLeft).size(14).color(text_color),
            text(" Initiatives").size(14).color(text_color)
        ]
        .spacing(4),
    )
    .on_press(Message::NavigateToInitiatives)
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
fn view_status_badge<'a>(label: &'a str, color: Color) -> Element<'a, Message> {
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

/// Renders the initiative information section (description, owner).
fn view_info_section<'a>(
    state: &InitiativeDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let mut info_items: Vec<Element<'a, Message>> = Vec::new();

    // Initiative ID
    info_items.push(
        row![
            text("ID:").size(11).color(palette.text_muted),
            text(state.initiative.id.clone())
                .size(11)
                .color(palette.text_secondary)
                .font(iced::Font::MONOSPACE),
        ]
        .spacing(8)
        .into(),
    );

    // Description
    if let Some(desc) = &state.initiative.description
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
    if let Some(owner) = &state.initiative.owner
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
    let tags = state.initiative.tags_vec();
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
            text(format_date(&state.initiative.created_at))
                .size(10)
                .color(palette.text_muted),
            Space::with_width(16),
            text("Updated:").size(10).color(palette.text_muted),
            text(format_date(&state.initiative.updated_at))
                .size(10)
                .color(palette.text_muted),
        ]
        .spacing(4)
        .into(),
    );

    let info_column = Column::from_vec(info_items).spacing(12);
    widget::card(info_column, palette)
}

/// Renders tags as horizontal pills.
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

/// Renders the progress section with large progress bar and percentage.
fn view_progress_section<'a>(
    state: &InitiativeDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let (percent, stats_text) = if let Some(summary) = state.summary {
        let percent = summary.status.percent_complete / 100.0; // Convert to 0.0-1.0
        let stats = format!(
            "{}/{} tasks done, {} in progress, {} blocked",
            summary.status.tasks_done,
            summary.status.total_tasks,
            summary.status.tasks_in_progress,
            summary.status.tasks_blocked
        );
        (percent, stats)
    } else {
        // Fallback: calculate from tasks
        let total = state.tasks.len();
        let done = state.tasks.iter().filter(|t| t.status == "done").count();
        let percent = if total > 0 {
            done as f32 / total as f32
        } else {
            0.0
        };
        let stats = format!("{}/{} tasks done", done, total);
        (percent, stats)
    };

    let percentage_text = format!("{}%", (percent * 100.0) as u32);

    let track_bg = palette.card;
    let fill_bg = palette.accent;

    // Large progress bar - ensure FillPortion is at least 1 to avoid layout issues
    let fill_portion = ((percent * 100.0) as u16).max(1);
    let progress_bar = container(
        container("")
            .width(Length::FillPortion(fill_portion))
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: if percent > 0.0 {
                    Some(Background::Color(fill_bg))
                } else {
                    None
                },
                border: Border {
                    radius: Radius::from(appearance::CORNER_RADIUS),
                    ..Default::default()
                },
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(12.0))
    .style(move |_| container::Style {
        background: Some(Background::Color(track_bg)),
        border: Border {
            radius: Radius::from(appearance::CORNER_RADIUS),
            ..Default::default()
        },
        ..Default::default()
    });

    let content = column![
        row![
            text("Progress").size(14).color(palette.text_secondary),
            horizontal_space(),
            text(percentage_text)
                .size(20)
                .color(palette.accent)
                .font(iced::Font::MONOSPACE),
        ]
        .align_y(iced::Alignment::Center),
        Space::with_height(8),
        progress_bar,
        Space::with_height(8),
        text(stats_text).size(11).color(palette.text_muted),
    ]
    .spacing(0);

    widget::card(content, palette)
}

/// Renders the projects section with project summary cards.
fn view_projects_section<'a>(
    state: &InitiativeDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let header = row![
        text("Projects").size(14).color(palette.text_secondary),
        horizontal_space(),
    ]
    .align_y(iced::Alignment::Center);

    let projects_content: Element<'a, Message> = if let Some(summary) = state.summary {
        if summary.projects.is_empty() {
            container(
                text("No projects in this initiative")
                    .size(14)
                    .color(palette.text_muted),
            )
            .padding(24)
            .center_x(Length::Fill)
            .into()
        } else {
            // Create a vertical list of project cards
            let mut items: Vec<Element<'a, Message>> = Vec::new();
            for project in &summary.projects {
                items.push(view_project_card(project, palette));
            }

            Column::from_vec(items)
                .spacing(8)
                .width(Length::Fill)
                .into()
        }
    } else {
        container(
            text("Loading projects...")
                .size(14)
                .color(palette.text_muted),
        )
        .padding(24)
        .center_x(Length::Fill)
        .into()
    };

    let content = column![header, Space::with_height(16), projects_content].width(Length::Fill);

    widget::card(content, palette)
}

/// Renders a single project summary card.
fn view_project_card<'a>(
    project: &'a ProjectSummary,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let progress = if project.task_count > 0 {
        project.done_count as f32 / project.task_count as f32
    } else {
        0.0
    };

    let track_bg = palette.background;
    let fill_bg = if project.blocked {
        palette.status_blocked
    } else {
        palette.accent
    };

    // Mini progress bar - ensure FillPortion is at least 1 to avoid layout issues
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
                    radius: Radius::from(2.0),
                    ..Default::default()
                },
                ..Default::default()
            }),
    )
    .width(Length::Fill)
    .height(Length::Fixed(4.0))
    .style(move |_| container::Style {
        background: Some(Background::Color(track_bg)),
        border: Border {
            radius: Radius::from(2.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Blocked indicator
    let blocked_indicator: Element<'a, Message> = if project.blocked {
        container(
            text("⊘ blocked")
                .size(10)
                .color(palette.status_blocked)
                .font(iced::Font::MONOSPACE),
        )
        .padding(Padding::from([2, 6]))
        .style(move |_| {
            let blocked_color = palette.status_blocked;
            container::Style {
                background: Some(Background::Color(Color::from_rgba(
                    blocked_color.r,
                    blocked_color.g,
                    blocked_color.b,
                    0.15,
                ))),
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

    let stats_text = format!("{}/{}", project.done_count, project.task_count);

    let card_bg = palette.card;
    let card_hover = palette.card_hover;
    let border = palette.border;
    let border_hover = palette.border_hover;
    let text_color = palette.text;
    let project_id = project.id.clone();

    button(
        container(
            column![
                row![
                    text(project.name.clone()).size(13).color(palette.text),
                    horizontal_space(),
                    blocked_indicator,
                ]
                .align_y(iced::Alignment::Center),
                Space::with_height(8),
                progress_bar,
                Space::with_height(6),
                row![
                    text(stats_text)
                        .size(10)
                        .color(palette.text_muted)
                        .font(iced::Font::MONOSPACE),
                    text(" tasks").size(10).color(palette.text_muted),
                ],
            ]
            .spacing(0)
            .width(Length::Fill),
        )
        .padding(12)
        .width(Length::Fill),
    )
    .on_press(Message::ViewProjectDetail(project_id))
    .style(move |_, status| {
        let (bg, bdr) = match status {
            button::Status::Hovered => (card_hover, border_hover),
            _ => (card_bg, border),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: text_color,
            border: Border {
                color: bdr,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS),
            },
            ..Default::default()
        }
    })
    .width(Length::Fill)
    .into()
}

/// Renders the blockers section if any blockers exist.
fn view_blockers_section<'a>(
    state: &InitiativeDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let blockers = if let Some(summary) = state.summary {
        &summary.blockers
    } else {
        return Space::new(0, 0).into();
    };

    if blockers.is_empty() {
        return Space::new(0, 0).into();
    }

    let header = row![
        icon(Icon::Ban).size(14).color(palette.status_blocked),
        Space::with_width(8),
        text("Blockers").size(14).color(palette.status_blocked),
        horizontal_space(),
    ]
    .align_y(iced::Alignment::Center);

    let blocker_items: Vec<Element<'a, Message>> = blockers
        .iter()
        .map(|b| view_blocker_item(b, palette))
        .collect();

    let content = column![
        header,
        Space::with_height(12),
        Column::from_vec(blocker_items).spacing(8),
    ]
    .width(Length::Fill);

    // Use blocked styling for the card
    let blocked_color = palette.status_blocked;
    let bg = palette.surface;
    let border_color = Color::from_rgba(blocked_color.r, blocked_color.g, blocked_color.b, 0.5);

    container(content)
        .padding(appearance::PADDING_LARGE)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS_LARGE),
            },
            ..Default::default()
        })
        .into()
}

/// Renders a single blocker item.
fn view_blocker_item<'a>(
    blocker: &'a InitiativeBlockerInfo,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let blocked_bg = Color::from_rgba(
        palette.status_blocked.r,
        palette.status_blocked.g,
        palette.status_blocked.b,
        0.1,
    );

    container(
        column![
            row![
                text(blocker.project_name.clone())
                    .size(12)
                    .color(palette.text),
                Space::with_width(8),
                container(
                    text(blocker.blocker_type.clone())
                        .size(9)
                        .color(palette.status_blocked)
                        .font(iced::Font::MONOSPACE)
                )
                .padding(Padding::from([2, 6]))
                .style(move |_| container::Style {
                    background: Some(Background::Color(blocked_bg)),
                    border: Border {
                        radius: Radius::from(4.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ]
            .align_y(iced::Alignment::Center),
            text(blocker.description.clone())
                .size(11)
                .color(palette.text_secondary),
        ]
        .spacing(4),
    )
    .padding(Padding::from([8, 12]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(blocked_bg)),
        border: Border {
            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

/// Renders the tasks section with task list.
fn view_tasks_section<'a>(
    state: &InitiativeDetailState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let header = row![
        text("Tasks").size(14).color(palette.text_secondary),
        horizontal_space(),
        widget::icon_button(Icon::RefreshCw, Message::RefreshTasks, palette),
    ]
    .align_y(iced::Alignment::Center);

    let tasks_list: Element<'a, Message> = if state.tasks.is_empty() {
        container(
            column![
                text("No tasks yet").size(16).color(palette.text_secondary),
                Space::with_height(8),
                text("Tasks from projects will appear here")
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

        Column::from_vec(items)
            .spacing(6)
            .width(Length::Fill)
            .into()
    };

    let content = column![header, Space::with_height(16), tasks_list].width(Length::Fill);

    widget::card(content, palette)
}

/// Renders a single task row with status badge and quick action.
fn view_task_row<'a>(
    task: &'a GranaryTask,
    expanded_tasks: &'a HashSet<String>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let task_id = task.id.clone();
    let status = task.status_enum();
    let priority = task.priority_enum();
    let is_expanded = expanded_tasks.contains(&task_id);

    // Status icon
    let status_icon = widget::status_icon_text(&status, palette);

    // Priority badge
    let priority_color = match priority {
        granary_types::TaskPriority::P0 => palette.status_blocked,
        granary_types::TaskPriority::P1 => palette.accent,
        _ => palette.text_muted,
    };
    let priority_bg = Color::from_rgba(priority_color.r, priority_color.g, priority_color.b, 0.15);

    let priority_badge = container(
        text(priority.as_str())
            .size(10)
            .color(priority_color)
            .font(iced::Font::MONOSPACE),
    )
    .padding(Padding::from([3, 6]))
    .style(move |_| container::Style {
        background: Some(Background::Color(priority_bg)),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Expand indicator
    let expand_icon = icon(if is_expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    })
    .size(10)
    .color(palette.text_muted);

    // Header row
    let header_row = row![
        expand_icon,
        Space::with_width(8),
        status_icon,
        Space::with_width(8),
        text(task.title.clone()).size(14).color(palette.text),
        horizontal_space(),
        priority_badge,
    ]
    .align_y(iced::Alignment::Center);

    // Quick action button
    let quick_action: Element<'a, Message> = match status {
        granary_types::TaskStatus::Todo | granary_types::TaskStatus::Draft => {
            widget::action_button("Start", Message::StartTask(task_id.clone()), palette)
        }
        granary_types::TaskStatus::InProgress => {
            widget::action_button("Done", Message::CompleteTask(task_id.clone()), palette)
        }
        _ => Space::new(0, 0).into(),
    };

    // Build content based on expanded state
    let content: Element<'a, Message> = if is_expanded {
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

        // Task ID
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
            row![column![header_row,].width(Length::Fill), quick_action,]
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
        row![header_row, quick_action]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .into()
    };

    // Card wrapper
    let bg = palette.card;
    let bg_hover = palette.card_hover;
    let border = palette.border;
    let border_hov = palette.border_hover;
    let text_color = palette.text;

    button(container(content).padding(16).width(Length::Fill))
        .on_press(Message::ToggleTaskExpand(task_id))
        .style(move |_, btn_status| {
            let (bg_color, border_color) = match btn_status {
                button::Status::Hovered => (bg_hover, border_hov),
                _ => (bg, border),
            };
            button::Style {
                background: Some(Background::Color(bg_color)),
                text_color: text_color,
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

/// Format an ISO timestamp to just the date portion.
fn format_date(timestamp: &str) -> &str {
    if timestamp.len() >= 10 {
        &timestamp[..10]
    } else {
        timestamp
    }
}
