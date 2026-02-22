//! Enhanced project card component with full metadata display
//!
//! This module provides a reusable project card widget that displays
//! project information including status, tags, progress, and metadata.

use crate::appearance::{CORNER_RADIUS, CORNER_RADIUS_SMALL, Palette};
use crate::message::Message;
use crate::widget::icon;
use granary_types::Project;
use iced::border::Radius;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Padding};
use lucide_icons::Icon;

/// Task statistics for a project
#[derive(Debug, Clone, Default)]
pub struct TaskStats {
    /// Total number of tasks
    pub total: u32,
    /// Number of completed tasks
    pub done: u32,
    /// Number of tasks in progress
    pub in_progress: u32,
}

impl TaskStats {
    /// Calculate progress as a value from 0.0 to 1.0
    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.done as f32 / self.total as f32
        }
    }
}

/// Data required to render a project card
pub struct ProjectCardData<'a> {
    /// The project to display
    pub project: &'a Project,
    /// Whether this card is currently selected
    pub is_selected: bool,
    /// Optional task statistics for progress display
    pub task_stats: Option<TaskStats>,
}

/// Create an enhanced project card element
///
/// # Card Layout
/// 1. Header row: status dot + name + archive button
/// 2. ID/slug row (monospace, muted)
/// 3. Description (truncated to 2 lines with "..." if longer)
/// 4. Owner row (if set): "Owner: name"
/// 5. Tags row (if set): horizontal pills with accent background
/// 6. Progress bar (if TaskStats provided)
/// 7. Footer: created date + updated date (small, muted)
pub fn project_card<'a>(data: ProjectCardData<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let project = data.project;
    let is_selected = data.is_selected;
    let is_active = project.status_enum().is_active();

    // Status dot color based on project status
    let status_color = if is_active {
        palette.status_done
    } else {
        palette.text_muted
    };

    // Status dot
    let status_dot = container(Space::new(8, 8)).style(move |_| container::Style {
        background: Some(Background::Color(status_color)),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Project name
    let name_text = text(project.name.clone()).size(14).color(palette.text);

    // Edit button
    let edit_btn = view_edit_button(project.id.clone(), palette);

    // Archive/Unarchive button based on project status
    let archive_btn = view_archive_button(project.id.clone(), is_active, palette);

    // Header row: status dot + name + edit button + archive button
    let header_row = row![
        status_dot,
        name_text,
        Space::with_width(Length::Fill),
        edit_btn,
        archive_btn
    ]
    .spacing(12)
    .align_y(iced::Alignment::Center);

    // ID/slug row (monospace, muted)
    let id_text = text(project.id.clone())
        .size(11)
        .color(palette.text_muted)
        .font(iced::Font::MONOSPACE);

    // Build content column
    let mut content_column = column![header_row, id_text].spacing(4);

    // Description (truncated to 2 lines with "..." if longer)
    if let Some(desc) = &project.description
        && !desc.is_empty()
    {
        let truncated = truncate_description(desc, 100);
        content_column = content_column.push(
            text(truncated)
                .size(12)
                .color(palette.text_secondary)
                .width(Length::Fill),
        );
    }

    // Owner row (if set)
    if let Some(owner) = &project.owner
        && !owner.is_empty()
    {
        let owner_row = row![
            text("Owner:").size(11).color(palette.text_muted),
            text(owner.clone()).size(11).color(palette.text_secondary)
        ]
        .spacing(4);
        content_column = content_column.push(owner_row);
    }

    // Tags row (if set)
    let tags = project.tags_vec();
    if !tags.is_empty() {
        let tags_row = view_tags_row(&tags, palette);
        content_column = content_column.push(tags_row);
    }

    // Progress bar (if TaskStats provided)
    if let Some(stats) = data.task_stats
        && stats.total > 0
    {
        let progress_element = view_progress_bar(&stats, palette);
        content_column = content_column.push(progress_element);
    }

    // Footer: created date + updated date
    let footer = view_footer(project, palette);
    content_column = content_column.push(footer);

    // Border and background colors based on selection state
    let border_color = if is_selected {
        palette.accent
    } else {
        palette.border
    };

    let bg_color = if is_selected {
        palette.card_hover
    } else {
        palette.card
    };

    let hover_bg = palette.card_hover;
    let hover_border = palette.border_hover;

    // Wrap in button for click handling
    button(container(content_column).padding(12).width(Length::Fill))
        .on_press(Message::SelectProject(project.id.clone()))
        .style(move |_, status| {
            let (bg, border) = match status {
                button::Status::Hovered => (hover_bg, hover_border),
                _ => (bg_color, border_color),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: Radius::from(CORNER_RADIUS),
                },
                ..Default::default()
            }
        })
        .into()
}

/// Edit button for the project card
fn view_edit_button<'a>(project_id: String, palette: &'a Palette) -> Element<'a, Message> {
    let text_color = palette.text_muted;
    let hover_bg = palette.card_hover;
    let accent_color = palette.accent;

    button(container(text("Edit").size(11).color(text_color)).padding(Padding::from([4, 8])))
        .on_press(Message::ShowEditProject(project_id))
        .style(move |_, status| {
            let (bg, txt) = match status {
                button::Status::Hovered => (hover_bg, accent_color),
                _ => (Color::TRANSPARENT, text_color),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: txt,
                border: Border {
                    radius: Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Archive/Unarchive button for the project card
fn view_archive_button<'a>(
    project_id: String,
    is_active: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let text_color = palette.text_muted;
    let hover_bg = palette.card_hover;
    let danger_color = palette.status_blocked;

    let (lucide_icon, message) = if is_active {
        (Icon::Archive, Message::ArchiveProject(project_id))
    } else {
        (Icon::ArchiveRestore, Message::UnarchiveProject(project_id))
    };

    button(container(icon(lucide_icon).size(14).color(text_color)).padding(Padding::from([4, 8])))
        .on_press(message)
        .style(move |_, status| {
            let (bg, txt) = match status {
                button::Status::Hovered => (hover_bg, danger_color),
                _ => (Color::TRANSPARENT, text_color),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: txt,
                border: Border {
                    radius: Radius::from(4.0),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Render tags as horizontal pills
fn view_tags_row<'a>(tags: &[String], palette: &'a Palette) -> Element<'a, Message> {
    let accent = palette.accent;

    let tag_pills: Vec<Element<'a, Message>> = tags
        .iter()
        .take(5) // Limit to 5 tags to avoid overflow
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

    // Show "+N more" if there are more than 5 tags
    if tags.len() > 5 {
        let more_text = text(format!("+{} more", tags.len() - 5))
            .size(10)
            .color(palette.text_muted);
        tags_row = tags_row.push(more_text);
    }

    tags_row.into()
}

/// Render progress bar with task stats
fn view_progress_bar<'a>(stats: &TaskStats, palette: &'a Palette) -> Element<'a, Message> {
    let progress = stats.progress();
    let track_bg = palette.card;
    let fill_bg = palette.accent;

    // Progress bar track with fill - conditionally add fill/remaining so 0% and 100% render correctly
    let fill_portion = ((progress * 100.0) as u16).max(1);
    let remaining_portion = (((1.0 - progress) * 100.0) as u16).max(1);
    let mut bar_row = iced::widget::Row::new().height(Length::Fill);
    if progress > 0.0 {
        bar_row = bar_row.push(
            container("")
                .width(Length::FillPortion(fill_portion))
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(fill_bg)),
                    border: Border {
                        radius: Radius::from(CORNER_RADIUS_SMALL),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        );
    }
    if progress < 1.0 {
        bar_row = bar_row.push(
            container("")
                .width(Length::FillPortion(remaining_portion))
                .height(Length::Fill),
        );
    }
    let progress_bar = container(bar_row)
        .width(Length::Fill)
        .height(Length::Fixed(4.0))
        .style(move |_| container::Style {
            background: Some(Background::Color(track_bg)),
            border: Border {
                radius: Radius::from(CORNER_RADIUS_SMALL),
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
    .size(10)
    .color(palette.text_muted);

    column![progress_bar, stats_text].spacing(4).into()
}

/// Render footer with created and updated dates
fn view_footer<'a>(project: &Project, palette: &'a Palette) -> Element<'a, Message> {
    // Format dates - just show the date part if it's an ISO timestamp
    let created = format_date(&project.created_at);
    let updated = format_date(&project.updated_at);

    row![
        text(format!("Created: {}", created))
            .size(10)
            .color(palette.text_muted),
        Space::with_width(Length::Fill),
        text(format!("Updated: {}", updated))
            .size(10)
            .color(palette.text_muted),
    ]
    .into()
}

/// Truncate description to approximately N characters, ending at word boundary
fn truncate_description(desc: &str, max_chars: usize) -> String {
    if desc.len() <= max_chars {
        return desc.to_string();
    }

    // Find a good break point (space) before max_chars
    let truncated: String = desc.chars().take(max_chars).collect();
    if let Some(last_space) = truncated.rfind(' ') {
        format!("{}...", &truncated[..last_space])
    } else {
        format!("{}...", truncated)
    }
}

/// Format an ISO timestamp to just the date portion
fn format_date(timestamp: &str) -> &str {
    // ISO timestamps are like "2024-01-15T10:30:00Z"
    // Just take the date part
    if timestamp.len() >= 10 {
        &timestamp[..10]
    } else {
        timestamp
    }
}
