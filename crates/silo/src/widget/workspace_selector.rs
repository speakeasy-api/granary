//! Workspace selector widget for Silo
//!
//! Shows current workspace with dropdown for recent workspaces and browse option.
//! The dropdown is rendered as an overlay that floats above other content.

use std::path::{Path, PathBuf};

use iced::border::Radius;
use iced::widget::{Space, button, column, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Length, Padding, Shadow, Theme, Vector};
use lucide_icons::Icon;

use crate::appearance::{CORNER_RADIUS, CORNER_RADIUS_SMALL, FONT_SIZE_SMALL, Palette};
use crate::message::Message;
use crate::widget::icon;

/// State for the workspace selector widget
pub struct WorkspaceSelectorState<'a> {
    /// Currently selected workspace path
    pub current_workspace: Option<&'a PathBuf>,
    /// List of recent workspace paths
    pub recent_workspaces: &'a [PathBuf],
    /// Whether the dropdown menu is open
    pub dropdown_open: bool,
}

/// Renders just the workspace selector button (no dropdown)
pub fn view<'a>(state: WorkspaceSelectorState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let path_display = state
        .current_workspace
        .map(|p| truncate_path(p, 30))
        .unwrap_or_else(|| "No workspace".to_string());

    workspace_button(path_display, state.dropdown_open, palette)
}

/// Renders the dropdown menu as an overlay element
///
/// This should be rendered at the app level using `stack!` to overlay content.
/// Position it absolutely at the header location.
pub fn view_dropdown<'a>(
    state: &WorkspaceSelectorState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    if !state.dropdown_open {
        return Space::new(0, 0).into();
    }

    let dropdown = dropdown_menu(state.current_workspace, state.recent_workspaces, palette);

    // Wrap dropdown in a mouse_area to detect clicks outside
    let backdrop = mouse_area(
        container(Space::new(Length::Fill, Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .on_press(Message::ToggleWorkspaceDropdown);

    // Position the dropdown at the top-left with some offset for the header
    let positioned_dropdown = container(column![
        // Vertical spacer to position below header (header is ~52px with padding)
        Space::with_height(Length::Fixed(52.0)),
        // Horizontal positioning
        row![
            // Left sidebar offset (~64px)
            Space::with_width(Length::Fixed(64.0)),
            // Left padding (16px from header)
            Space::with_width(Length::Fixed(16.0)),
            dropdown,
        ]
    ])
    .width(Length::Fill)
    .height(Length::Fill);

    // Stack backdrop behind dropdown
    iced::widget::stack![backdrop, positioned_dropdown,].into()
}

/// Creates the main workspace button that toggles the dropdown
fn workspace_button<'a>(
    path_display: String,
    is_open: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let text_color = palette.text;
    let text_secondary = palette.text_secondary;
    let bg_normal = palette.card;
    let bg_hover = palette.card_hover;
    let border_normal = palette.border;
    let border_hover = palette.accent;
    let border_active = palette.accent;

    let arrow = if is_open { "▴" } else { "▾" };

    let content = row![
        text(path_display).size(12).color(text_color),
        Space::with_width(Length::Fixed(8.0)),
        text(arrow).size(10).color(text_secondary),
    ]
    .align_y(iced::Alignment::Center);

    button(container(content).padding(Padding::from([6, 12])))
        .on_press(Message::ToggleWorkspaceDropdown)
        .style(move |_: &Theme, status| {
            let (bg, border) = if is_open {
                (bg_hover, border_active)
            } else {
                match status {
                    button::Status::Hovered => (bg_hover, border_hover),
                    _ => (bg_normal, border_normal),
                }
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

/// Creates the dropdown menu with recent workspaces and browse option
fn dropdown_menu<'a>(
    current_workspace: Option<&'a PathBuf>,
    recent_workspaces: &'a [PathBuf],
    palette: &'a Palette,
) -> Element<'a, Message> {
    let bg = palette.surface;
    let border_color = palette.border;
    let text_color = palette.text;
    let text_secondary = palette.text_secondary;
    let accent = palette.accent;

    let mut items: Vec<Element<'a, Message>> = Vec::new();

    // Header
    items.push(
        container(
            text("Recent Workspaces")
                .size(FONT_SIZE_SMALL)
                .color(text_secondary),
        )
        .padding(Padding::from([8, 12]))
        .into(),
    );

    // Divider
    items.push(divider(palette));

    // Recent workspaces (up to 5)
    let workspaces_to_show: Vec<_> = recent_workspaces.iter().take(5).collect();

    if workspaces_to_show.is_empty() {
        items.push(
            container(
                text("No recent workspaces")
                    .size(FONT_SIZE_SMALL)
                    .color(text_secondary),
            )
            .padding(Padding::from([8, 12]))
            .into(),
        );
    } else {
        for workspace in workspaces_to_show {
            let is_current = current_workspace == Some(workspace);
            items.push(workspace_item(
                workspace, is_current, text_color, accent, palette,
            ));
        }
    }

    // Divider before browse
    items.push(divider(palette));

    // Browse option
    items.push(browse_item(text_color, palette));

    // Container for dropdown with shadow for floating appearance
    let dropdown_content = column(items).width(Length::Fixed(240.0));

    container(dropdown_content)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(CORNER_RADIUS),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        })
        .into()
}

/// Creates a workspace item in the dropdown
fn workspace_item<'a>(
    workspace: &'a Path,
    is_current: bool,
    text_color: Color,
    accent: Color,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let display = truncate_path(workspace, 25);
    let hover_bg = palette.card_hover;

    let checkmark: Element<'a, Message> = if is_current {
        icon(Icon::Check).size(12).color(accent).into()
    } else {
        Space::with_width(12).into()
    };

    let content = row![
        text(display)
            .size(FONT_SIZE_SMALL)
            .color(if is_current { accent } else { text_color }),
        Space::with_width(Length::Fill),
        checkmark,
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill);

    button(container(content).padding(Padding::from([6, 12])))
        .on_press(Message::SelectRecentWorkspace(workspace.to_path_buf()))
        .width(Length::Fill)
        .style(move |_: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => hover_bg,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color,
                ..Default::default()
            }
        })
        .into()
}

/// Creates the "Browse..." option in the dropdown
fn browse_item<'a>(text_color: Color, palette: &'a Palette) -> Element<'a, Message> {
    let hover_bg = palette.card_hover;

    let content = text("Browse...").size(FONT_SIZE_SMALL).color(text_color);

    button(container(content).padding(Padding::from([6, 12])))
        .on_press(Message::SelectWorkspace)
        .width(Length::Fill)
        .style(move |_: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => hover_bg,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color,
                ..Default::default()
            }
        })
        .into()
}

/// Creates a horizontal divider
fn divider<'a>(palette: &'a Palette) -> Element<'a, Message> {
    let border_color = palette.border;

    container(Space::with_height(Length::Fixed(1.0)))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(border_color)),
            ..Default::default()
        })
        .into()
}

/// Truncates a path to fit within max_len characters
fn truncate_path(path: &Path, max_len: usize) -> String {
    let s = path.display().to_string();

    // Replace home directory with ~
    let home = dirs::home_dir();
    let display = if let Some(home_path) = home {
        if let Ok(relative) = path.strip_prefix(&home_path) {
            format!("~/{}", relative.display())
        } else {
            s
        }
    } else {
        s
    };

    if display.len() > max_len {
        format!("...{}", &display[display.len() - max_len + 3..])
    } else {
        display
    }
}
