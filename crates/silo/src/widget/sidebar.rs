//! Sidebar navigation widget for Silo
//!
//! Displays icon buttons for main navigation areas.

use crate::appearance::{CORNER_RADIUS_SMALL, Palette};
use crate::message::Message;
use crate::screen::Screen;
use crate::widget::icon;
use iced::border::Radius;
use iced::widget::{Column, button, container, text};
use iced::{Background, Border, Color, Element, Length, Padding};
use lucide_icons::Icon;

/// Sidebar navigation item
struct NavItem {
    icon: lucide_icons::Icon,
    label: &'static str,
    screen: Screen,
}

const NAV_ITEMS: &[NavItem] = &[
    NavItem {
        icon: Icon::BadgeInfo,
        label: "Initiatives",
        screen: Screen::Initiatives,
    },
    NavItem {
        icon: Icon::Projector,
        label: "Projects",
        screen: Screen::Projects,
    },
    NavItem {
        icon: Icon::Workflow,
        label: "Workers",
        screen: Screen::Workers,
    },
    NavItem {
        icon: Icon::SquarePlay,
        label: "Runs",
        screen: Screen::Runs,
    },
    NavItem {
        icon: Icon::Settings2,
        label: "Settings",
        screen: Screen::Settings,
    },
];

/// Sidebar width in pixels
const SIDEBAR_WIDTH: f32 = 160.0;

/// Renders the sidebar navigation
pub fn view<'a>(current_screen: &Screen, palette: &'a Palette) -> Element<'a, Message> {
    let bg = palette.surface;

    let items: Vec<Element<'a, Message>> = NAV_ITEMS
        .iter()
        .map(|item| nav_button(item, current_screen, palette))
        .collect();

    container(Column::from_vec(items).spacing(6).padding(12))
        .width(Length::Fixed(SIDEBAR_WIDTH))
        .height(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            ..Default::default()
        })
        .into()
}

/// Check if a screen matches the current screen (for highlighting)
fn is_active(item_screen: &Screen, current: &Screen) -> bool {
    // Match top-level screens, including detail variants
    matches!(
        (item_screen, current),
        (Screen::Initiatives, Screen::Initiatives)
            | (Screen::Initiatives, Screen::InitiativeDetail { .. })
            | (Screen::Projects, Screen::Projects)
            | (Screen::Projects, Screen::ProjectDetail { .. })
            | (Screen::Projects, Screen::CreateProject)
            | (Screen::Projects, Screen::EditProject { .. })
            | (Screen::Projects, Screen::Tasks)
            | (Screen::Projects, Screen::TaskDetail { .. })
            | (Screen::Projects, Screen::CreateTask { .. })
            | (Screen::Projects, Screen::EditTask { .. })
            | (Screen::Workers, Screen::Workers)
            | (Screen::Workers, Screen::WorkerDetail { .. })
            | (Screen::Workers, Screen::StartWorker)
            | (Screen::Runs, Screen::Runs)
            | (Screen::Runs, Screen::RunDetail { .. })
            | (Screen::Runs, Screen::Logs { .. })
            | (Screen::Settings, Screen::Settings)
    )
}

fn nav_button<'a>(item: &NavItem, current: &Screen, palette: &'a Palette) -> Element<'a, Message> {
    let is_active = is_active(&item.screen, current);

    let text_color = if is_active {
        palette.accent
    } else {
        palette.text_secondary
    };

    let icon_color = if is_active {
        palette.accent
    } else {
        palette.text_muted
    };

    let hover_bg = palette.card_hover;
    let active_bg = palette.card;
    let accent = palette.accent;
    let msg = Message::Navigate(item.screen.clone());

    let icon_widget = icon(item.icon).color(icon_color);

    let label = text(item.label).size(13).color(text_color);

    let content = iced::widget::row![icon_widget, label]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    button(content)
        .on_press(msg)
        .padding(Padding::from([10, 16]))
        .style(move |_, status| {
            let bg = match status {
                button::Status::Hovered => hover_bg,
                _ if is_active => active_bg,
                _ => Color::TRANSPARENT,
            };

            let border = if is_active {
                Border {
                    color: accent,
                    width: 1.0,
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                }
            } else {
                Border {
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                    ..Default::default()
                }
            };

            button::Style {
                background: Some(Background::Color(bg)),
                border,
                ..Default::default()
            }
        })
        .into()
}
