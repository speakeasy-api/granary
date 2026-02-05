//! Logs screen - full-screen log viewer for workers and runs
//!
//! Displays logs from either a worker or a run, with follow mode
//! and navigation back to main screen.

use super::LogSource;
use crate::appearance::{self, FONT_SIZE_SMALL, PADDING, PADDING_LARGE, Palette, SPACING};
use crate::message::Message;
use crate::widget::log_viewer::log_viewer;
use iced::border::Radius;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Element, Length, Padding as IcedPadding, Theme};

/// Get a display label for the log source
fn log_source_label(source: &LogSource) -> String {
    match source {
        LogSource::Worker { id } => format!("Worker: {}", id),
        LogSource::Run { id } => format!("Run: {}", id),
    }
}

/// State needed to render the logs screen
pub struct LogsScreenState<'a> {
    /// Source of the logs (worker or run)
    pub log_source: &'a LogSource,
    /// Log lines to display
    pub lines: &'a [String],
    /// Whether follow mode is enabled (auto-scroll to bottom)
    pub follow: bool,
    /// Whether logs are currently being loaded
    pub loading: bool,
}

/// Renders the logs screen.
///
/// Displays a full-screen log viewer with:
/// - Header with back button, source name, and controls
/// - Scrollable log content using the LogViewer widget
/// - Status bar showing line count and loading state
pub fn view<'a>(state: LogsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    // Back button
    let back_btn = back_button("<- Back", palette);

    // Source label
    let source_label = text(log_source_label(state.log_source))
        .size(24)
        .color(palette.text)
        .font(iced::Font::MONOSPACE);

    // Loading indicator
    let loading_indicator = if state.loading {
        text("Loading...")
            .size(FONT_SIZE_SMALL)
            .color(palette.accent)
    } else {
        text("").size(FONT_SIZE_SMALL)
    };

    // Header row
    let header = container(
        row![
            back_btn,
            Space::with_width(Length::Fixed(16.0)),
            source_label,
            Space::with_width(Length::Fill),
            loading_indicator,
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding(IcedPadding::from([PADDING, PADDING_LARGE]))
    .width(Length::Fill)
    .style({
        let bg = palette.surface;
        let border_color = palette.border;
        move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(0.0),
            },
            ..Default::default()
        }
    });

    // Log viewer widget
    let log_view = log_viewer(state.lines)
        .title(log_source_label(state.log_source))
        .follow(state.follow)
        .on_toggle_follow(Message::ToggleLogFollow)
        .on_clear(Message::ClearLogs)
        .view();

    // Status bar
    let line_count_text = text(format!("{} lines", state.lines.len()))
        .size(FONT_SIZE_SMALL)
        .color(palette.text_secondary);

    let streaming_text = if state.loading {
        text("Streaming...")
            .size(FONT_SIZE_SMALL)
            .color(palette.accent)
    } else {
        text("").size(FONT_SIZE_SMALL).color(palette.text_secondary)
    };

    let status_bar = container(
        row![
            line_count_text,
            Space::with_width(Length::Fill),
            streaming_text,
        ]
        .spacing(SPACING),
    )
    .padding(IcedPadding::from([8, PADDING_LARGE]))
    .width(Length::Fill)
    .style({
        let bg = palette.surface;
        let border_color = palette.border;
        move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(0.0),
            },
            ..Default::default()
        }
    });

    // Main layout: header, log content, status bar
    let content = column![header, log_view, status_bar]
        .width(Length::Fill)
        .height(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style({
            let bg = palette.background;
            move |_| container::Style {
                background: Some(Background::Color(bg)),
                ..Default::default()
            }
        })
        .into()
}

/// Helper function to create the back button with consistent styling
fn back_button<'a>(label: &'a str, palette: &'a Palette) -> Element<'a, Message> {
    let text_color = palette.text_secondary;
    let bg_normal = palette.card;
    let bg_hover = palette.card_hover;
    let border_color = palette.border;

    button(container(text(label).size(13.0).color(text_color)).padding(IcedPadding::from([6, 12])))
        .on_press(Message::CloseLogs)
        .style(move |_: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => bg_hover,
                _ => bg_normal,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                },
                ..Default::default()
            }
        })
        .into()
}
