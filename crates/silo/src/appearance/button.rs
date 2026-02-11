//! Button style variants for Silo
//!
//! Provides button style functions for different visual variants.

use iced::widget::button;
use iced::{Background, Border, Color, Theme};

use super::{CORNER_RADIUS, CORNER_RADIUS_SMALL, darken, palette, with_alpha};

/// Button style variants
#[derive(Debug, Clone, Copy, Default)]
pub enum ButtonStyle {
    /// Primary action button (filled with accent color)
    #[default]
    Primary,
    /// Secondary action button (subtle background)
    Secondary,
    /// Ghost button (transparent background)
    Ghost,
    /// Icon-only button
    Icon,
    /// Danger button for destructive actions (red styling)
    Danger,
    /// Card-style button (used for selectable cards)
    Card,
    /// Selected card variant
    CardSelected,
    /// Action button in task rows
    Action,
}

impl ButtonStyle {
    /// Returns a style function for use with button::style()
    pub fn style_fn(self) -> impl Fn(&Theme, button::Status) -> button::Style {
        move |_theme, status| self.style(status)
    }

    /// Get the button style for the given status
    pub fn style(self, status: button::Status) -> button::Style {
        let p = palette();

        match self {
            ButtonStyle::Primary => {
                let bg = match status {
                    button::Status::Active => p.text,
                    button::Status::Hovered => p.accent_light,
                    button::Status::Pressed => p.accent,
                    button::Status::Disabled => with_alpha(p.text, 0.5),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: p.background,
                    border: Border {
                        radius: CORNER_RADIUS.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }

            ButtonStyle::Secondary => {
                let (bg, border) = match status {
                    button::Status::Hovered => (p.card_hover, p.accent),
                    button::Status::Pressed => (p.card, p.accent),
                    button::Status::Active => (p.card, p.border),
                    button::Status::Disabled => (with_alpha(p.card, 0.5), p.border),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: p.text,
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: CORNER_RADIUS.into(),
                    },
                    ..Default::default()
                }
            }

            ButtonStyle::Ghost => {
                let bg = match status {
                    button::Status::Hovered => p.card_hover,
                    button::Status::Pressed => p.card,
                    _ => Color::TRANSPARENT,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: p.text_secondary,
                    border: Border {
                        radius: CORNER_RADIUS_SMALL.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }

            ButtonStyle::Icon => {
                let bg = match status {
                    button::Status::Hovered => p.card_hover,
                    button::Status::Pressed => p.card,
                    _ => Color::TRANSPARENT,
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: p.text_secondary,
                    border: Border {
                        radius: CORNER_RADIUS_SMALL.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }

            ButtonStyle::Card => {
                let (bg, border) = match status {
                    button::Status::Hovered => (p.card_hover, p.border_hover),
                    button::Status::Pressed => (p.card, p.accent),
                    button::Status::Active => (p.card, p.border),
                    button::Status::Disabled => (with_alpha(p.card, 0.5), p.border),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: p.text,
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: CORNER_RADIUS.into(),
                    },
                    ..Default::default()
                }
            }

            ButtonStyle::CardSelected => {
                let (bg, border) = match status {
                    button::Status::Hovered => (p.card_hover, p.border_hover),
                    _ => (p.card_hover, p.accent),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: p.text,
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: CORNER_RADIUS.into(),
                    },
                    ..Default::default()
                }
            }

            ButtonStyle::Action => {
                let (bg, border) = match status {
                    button::Status::Hovered => (p.card_hover, p.accent),
                    button::Status::Pressed => (p.card, p.accent),
                    button::Status::Active => (p.card, p.border),
                    button::Status::Disabled => (with_alpha(p.card, 0.5), p.border),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: p.text,
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: CORNER_RADIUS_SMALL.into(),
                    },
                    ..Default::default()
                }
            }

            ButtonStyle::Danger => {
                let (bg, text_color) = match status {
                    button::Status::Active => (p.danger, p.text),
                    button::Status::Hovered => (p.danger_light, p.background),
                    button::Status::Pressed => (darken(p.danger, 0.1), p.text),
                    button::Status::Disabled => {
                        (with_alpha(p.danger, 0.5), with_alpha(p.text, 0.7))
                    }
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color,
                    border: Border {
                        radius: CORNER_RADIUS.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }
        }
    }
}
