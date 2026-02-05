//! Select dropdown widget
//!
//! Dropdown with single select support using pick_list.

use super::{ValidationState, field_label};
use crate::appearance::{CORNER_RADIUS_SMALL, FONT_SIZE_SMALL, palette};
use iced::overlay::menu;
use iced::widget::{column, pick_list, text};
use iced::{Background, Border, Element, Length, Theme};
use std::fmt::Display;

/// Single-select dropdown configuration
pub struct Select<'a, T, Message>
where
    T: ToString + PartialEq + Clone + 'static,
{
    label: &'a str,
    options: &'a [T],
    selected: Option<T>,
    on_select: Box<dyn Fn(T) -> Message + 'a>,
    placeholder: &'a str,
    validation: ValidationState,
    required: bool,
}

impl<'a, T, Message> Select<'a, T, Message>
where
    T: ToString + PartialEq + Clone + Display + 'static,
    Message: Clone + 'a,
{
    pub fn new(
        label: &'a str,
        options: &'a [T],
        selected: Option<T>,
        on_select: impl Fn(T) -> Message + 'a,
    ) -> Self {
        Self {
            label,
            options,
            selected,
            on_select: Box::new(on_select),
            placeholder: "Select...",
            validation: ValidationState::None,
            required: false,
        }
    }

    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn validation(mut self, state: ValidationState) -> Self {
        self.validation = state;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn view(self) -> Element<'a, Message> {
        let p = palette();
        let is_invalid = self.validation.is_invalid();

        let border_color = if is_invalid { p.danger } else { p.border };

        // Extract error message before moving self.validation
        let error_msg = self.validation.error_message().map(|s| s.to_string());

        let picker = pick_list(self.options, self.selected, self.on_select)
            .placeholder(self.placeholder)
            .width(Length::Fill)
            .padding(8)
            .style(move |_theme: &Theme, status| {
                let border = match status {
                    pick_list::Status::Hovered => p.border_hover,
                    pick_list::Status::Opened => p.accent,
                    _ => border_color,
                };
                pick_list::Style {
                    text_color: p.text,
                    placeholder_color: p.text_muted,
                    handle_color: p.text_secondary,
                    background: Background::Color(p.input),
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: CORNER_RADIUS_SMALL.into(),
                    },
                }
            })
            .menu_style(move |_theme: &Theme| menu::Style {
                text_color: p.text,
                background: Background::Color(p.surface),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: CORNER_RADIUS_SMALL.into(),
                },
                selected_text_color: p.text,
                selected_background: Background::Color(p.card_hover),
            });

        let mut col = column![field_label(self.label, self.required), picker,].spacing(4);

        if let Some(msg) = error_msg {
            col = col.push(text(msg).size(FONT_SIZE_SMALL).color(p.status_blocked));
        }

        col.into()
    }
}

/// Convenience function for creating selects
pub fn select<'a, T, Message>(
    label: &'a str,
    options: &'a [T],
    selected: Option<T>,
    on_select: impl Fn(T) -> Message + 'a,
) -> Select<'a, T, Message>
where
    T: ToString + PartialEq + Clone + Display + 'static,
    Message: Clone + 'a,
{
    Select::new(label, options, selected, on_select)
}
