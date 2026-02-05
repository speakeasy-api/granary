//! Multi-line text area widget
//!
//! A text area for longer content with label support.

use super::{error_message, field_label};
use crate::appearance::{CORNER_RADIUS_SMALL, palette, with_alpha};
use iced::widget::text_editor::{Action, Content};
use iced::widget::{column, text_editor};
use iced::{Background, Border, Element, Length, Theme};

/// Multi-line text area configuration
pub struct TextArea<'a, Message> {
    label: &'a str,
    content: &'a Content,
    on_action: Box<dyn Fn(Action) -> Message + 'a>,
    placeholder: &'a str,
    height: Length,
    validation: Option<&'a str>,
    required: bool,
}

impl<'a, Message: Clone + 'a> TextArea<'a, Message> {
    pub fn new(
        label: &'a str,
        content: &'a Content,
        on_action: impl Fn(Action) -> Message + 'a,
    ) -> Self {
        Self {
            label,
            content,
            on_action: Box::new(on_action),
            placeholder: "",
            height: Length::Fixed(120.0),
            validation: None,
            required: false,
        }
    }

    pub fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn error(mut self, error: Option<&'a str>) -> Self {
        self.validation = error;
        self
    }

    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    pub fn view(self) -> Element<'a, Message> {
        let p = palette();
        let is_invalid = self.validation.is_some();

        let border_color = if is_invalid {
            p.status_blocked
        } else {
            p.border
        };

        let editor = text_editor(self.content)
            .on_action(self.on_action)
            .height(self.height)
            .padding(8)
            .style(move |_theme: &Theme, status| {
                let border = match status {
                    text_editor::Status::Focused => p.accent,
                    text_editor::Status::Hovered => p.border_hover,
                    _ => border_color,
                };
                text_editor::Style {
                    background: Background::Color(p.input),
                    border: Border {
                        color: border,
                        width: 1.0,
                        radius: CORNER_RADIUS_SMALL.into(),
                    },
                    icon: p.text_muted,
                    placeholder: p.text_muted,
                    value: p.text,
                    selection: with_alpha(p.accent, 0.3),
                }
            });

        let mut col = column![field_label(self.label, self.required), editor,].spacing(4);

        if let Some(error_msg) = self.validation {
            col = col.push(error_message(error_msg));
        }

        col.into()
    }
}

/// Convenience function for creating text areas
pub fn text_area<'a, Message: Clone + 'a>(
    label: &'a str,
    content: &'a Content,
    on_action: impl Fn(Action) -> Message + 'a,
) -> TextArea<'a, Message> {
    TextArea::new(label, content, on_action)
}
