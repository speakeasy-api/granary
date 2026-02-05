//! SelectWorkspace screen - initial workspace selection view.
//!
//! This screen is displayed when the application starts and allows
//! users to select a granary workspace folder.

use crate::appearance::{self, FONT_SIZE_HERO, Palette};
use crate::message::Message;
use iced::border::Radius;
use iced::widget::{Space, button, column, container, text};
use iced::{Background, Border, Element, Length, Padding};

/// Renders the workspace selection screen.
///
/// Displays the Silo logo, tagline, and an "Open Workspace" button.
pub fn view(palette: &Palette) -> Element<'static, Message> {
    let logo = text("silo")
        .size(FONT_SIZE_HERO)
        .color(palette.text)
        .font(iced::Font::MONOSPACE);

    let tagline = text("graphical interface for granary")
        .size(18)
        .color(palette.text_secondary);

    let open_btn = button(
        container(text("Open Workspace").size(16).color(palette.background))
            .padding(Padding::from([12, 32])),
    )
    .on_press(Message::SelectWorkspace)
    .style({
        let text_color = palette.text;
        let accent_light = palette.accent_light;
        move |_, status| {
            let bg = match status {
                button::Status::Hovered => accent_light,
                _ => text_color,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: Radius::from(appearance::CORNER_RADIUS),
                    ..Default::default()
                },
                ..Default::default()
            }
        }
    });

    let hint = text("Select a folder containing a .granary workspace")
        .size(13)
        .color(palette.text_muted);

    let content = column![
        Space::with_height(Length::FillPortion(2)),
        logo,
        Space::with_height(8),
        tagline,
        Space::with_height(48),
        open_btn,
        Space::with_height(16),
        hint,
        Space::with_height(Length::FillPortion(3)),
    ]
    .align_x(iced::Alignment::Center)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .into()
}
