//! Reusable widget builders for Silo
//!
//! This module provides generic, reusable widget functions that accept
//! a palette for theming consistency.

pub mod form;
pub mod icon;
pub mod initiative_card;
pub mod log_viewer;
pub mod progress_bar;
pub mod project_card;
pub mod sidebar;
pub mod status_icon;
pub mod task_graph;
pub mod workspace_selector;
pub use icon::icon;
pub use initiative_card::initiative_card;
pub use log_viewer::{LogViewer, log_viewer};
pub use progress_bar::{ProgressBar, progress_bar};
pub use project_card::{ProjectCardData, TaskStats, project_card};
pub use status_icon::{IconSize, StatusIcon, status_icon, status_icon_text};
pub use task_graph::{TaskGraph, TaskNode, task_graph};
pub use workspace_selector::{
    WorkspaceSelectorState, view as workspace_selector, view_dropdown as workspace_dropdown,
};

use crate::appearance::{
    CORNER_RADIUS, CORNER_RADIUS_LARGE, CORNER_RADIUS_SMALL, PADDING_LARGE, Palette,
};
use iced::border::Radius;
use iced::widget::{button, container, horizontal_space, row, text};
use iced::{Background, Border, Color, Element, Font, Length, Padding, Shadow, Theme, Vector};

/// Fixed height for page headers to ensure consistent spacing across all screens
pub const PAGE_HEADER_HEIGHT: f32 = 40.0;

/// Spinner animation frames (braille pattern)
const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Returns the current spinner character for the given frame
pub fn spinner_char(frame: usize) -> &'static str {
    SPINNER_FRAMES[frame % SPINNER_FRAMES.len()]
}

/// Renders an animated spinner element
pub fn spinner<'a, Message: 'a>(frame: usize, palette: &'a Palette) -> Element<'a, Message> {
    text(spinner_char(frame))
        .size(14)
        .color(palette.accent)
        .font(Font::MONOSPACE)
        .into()
}

/// Card container with shadow and border
///
/// Creates a styled container with padding, rounded corners, border, and shadow.
/// Used for main content areas.
pub fn card<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    palette: &Palette,
) -> Element<'a, Message> {
    let bg = palette.surface;
    let border_color = palette.border;

    container(content)
        .padding(PADDING_LARGE)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(CORNER_RADIUS_LARGE),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 16.0,
            },
            ..Default::default()
        })
        .into()
}

/// Icon button (small, icon-only)
///
/// Creates a compact button with just an icon, suitable for toolbars
/// and action areas. Has transparent background that highlights on hover.
pub fn icon_button<Message: Clone + 'static>(
    lucide_icon: lucide_icons::Icon,
    msg: Message,
    palette: &Palette,
) -> Element<'static, Message> {
    let text_color = palette.text_secondary;
    let hover_bg = palette.card_hover;

    button(container(icon(lucide_icon).size(14).color(text_color)).padding(Padding::from([6, 10])))
        .on_press(msg)
        .style(move |_, status| {
            let bg = match status {
                button::Status::Hovered => hover_bg,
                _ => Color::TRANSPARENT,
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

/// Action button (labeled)
///
/// Creates a labeled button with border styling, suitable for
/// secondary actions. Has subtle border that accents on hover.
pub fn action_button<'a, Message: Clone + 'a>(
    label: &'a str,
    msg: Message,
    palette: &Palette,
) -> Element<'a, Message> {
    let text_color = palette.text;
    let bg_normal = palette.card;
    let bg_hover = palette.card_hover;
    let border_normal = palette.border;
    let border_hover = palette.accent;

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

/// Add button (+)
///
/// Creates a button with a "+" icon for adding new items.
/// Has border styling that accents on hover.
pub fn add_button<'a, Message: Clone + 'a>(
    msg: Message,
    palette: &Palette,
) -> Element<'a, Message> {
    let text_color = palette.text;
    let bg_normal = palette.card;
    let bg_hover = palette.card_hover;
    let border_normal = palette.border;
    let border_hover = palette.accent;

    button(container(text("+").size(18).color(text_color)).padding(Padding::from([8, 16])))
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
                    radius: Radius::from(CORNER_RADIUS),
                },
                ..Default::default()
            }
        })
        .into()
}

/// Page header with consistent styling
///
/// Creates a page header with:
/// - Fixed height for consistent spacing across all screens
/// - Title on the left (size 24, monospace font)
/// - Trailing elements on the right (buttons, status indicators)
pub fn page_header<'a, Message: 'a>(
    title: &'a str,
    trailing: impl Into<Element<'a, Message>>,
    palette: &Palette,
) -> Element<'a, Message> {
    let title_text = text(title)
        .size(24)
        .color(palette.text)
        .font(Font::MONOSPACE);

    container(
        row![title_text, horizontal_space(), trailing.into()].align_y(iced::Alignment::Center),
    )
    .height(Length::Fixed(PAGE_HEADER_HEIGHT))
    .width(Length::Fill)
    .into()
}

/// Page header with just a title (no trailing elements)
pub fn page_header_simple<'a, Message: 'a>(
    title: &'a str,
    palette: &Palette,
) -> Element<'a, Message> {
    let title_text = text(title)
        .size(24)
        .color(palette.text)
        .font(Font::MONOSPACE);

    container(row![title_text].align_y(iced::Alignment::Center))
        .height(Length::Fixed(PAGE_HEADER_HEIGHT))
        .width(Length::Fill)
        .into()
}
