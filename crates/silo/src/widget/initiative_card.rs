//! Initiative card widget for displaying initiative summary.
//!
//! This module provides a reusable initiative card widget that displays
//! initiative information including status badge, progress bar, and stats.

use crate::appearance::{CORNER_RADIUS, Palette};
use crate::message::Message;
use crate::widget::progress_bar;
use granary_types::{Initiative, InitiativeStatus, InitiativeSummary};
use iced::border::Radius;
use iced::widget::{Space, button, column, container, horizontal_space, row, text};
use iced::{Background, Border, Color, Element, Length, Padding};

/// Renders an initiative card with summary stats.
///
/// The card displays:
/// - Initiative name (large, primary text)
/// - Status badge (active/archived)
/// - Progress bar showing completion percentage
/// - Stats row: project count, task count, blocker count
/// - Clicking the card triggers SelectInitiative
pub fn initiative_card<'a>(
    initiative: &'a Initiative,
    summary: Option<&'a InitiativeSummary>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let id = initiative.id.clone();
    let name = initiative.name.clone();
    let status = initiative.status_enum();

    // Status badge color based on initiative status
    let status_color = if status == InitiativeStatus::Active {
        palette.status_done
    } else {
        palette.text_muted
    };

    // Status badge (similar to task status badge pattern)
    let status_badge = container(
        text(status.as_str())
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

    // Header row: name + status badge
    let header_row = row![
        text(name).size(16).color(palette.text),
        horizontal_space(),
        status_badge
    ]
    .align_y(iced::Alignment::Center);

    // Progress bar and stats (if summary available)
    // When summary is not available (list view), show description or a placeholder
    let (progress_element, stats_row): (Element<'a, Message>, Element<'a, Message>) =
        if let Some(s) = summary {
            let progress = progress_bar(s.status.percent_complete / 100.0)
                .height(6.0)
                .show_percentage(false)
                .view();

            // Build stats row with project count, task count, blocker count
            let blocked_color = if s.status.tasks_blocked > 0 {
                palette.status_blocked
            } else {
                palette.text_muted
            };

            let stats = row![
                text(format!("{} projects", s.status.total_projects))
                    .size(11)
                    .color(palette.text_muted),
                Space::with_width(16),
                text(format!("{} tasks", s.status.total_tasks))
                    .size(11)
                    .color(palette.text_muted),
                Space::with_width(16),
                text(format!("{} blocked", s.status.tasks_blocked))
                    .size(11)
                    .color(blocked_color),
            ];

            (progress, stats.into())
        } else {
            // No summary available - show description or created date
            let description_text = initiative
                .description
                .as_ref()
                .filter(|d| !d.is_empty())
                .map(|d| d.as_str())
                .unwrap_or("Click to view details");

            (
                Space::new(0, 0).into(),
                row![text(description_text).size(11).color(palette.text_muted)].into(),
            )
        };

    // Card content layout
    let content = column![
        header_row,
        Space::with_height(8),
        progress_element,
        Space::with_height(8),
        stats_row
    ]
    .spacing(4);

    // Border and background colors
    let bg_color = palette.card;
    let hover_bg = palette.card_hover;
    let border_color = palette.border;
    let hover_border = palette.border_hover;

    // Wrap in clickable button with card styling
    button(container(content).padding(16).width(Length::Fill))
        .on_press(Message::SelectInitiative(id))
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
        .width(Length::Fill)
        .into()
}
