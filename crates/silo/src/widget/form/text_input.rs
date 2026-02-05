//! Labeled text input with validation support
//!
//! A text input field with label, placeholder, and error state display.

use super::{ValidationState, field_label};
use crate::appearance::{
    CORNER_RADIUS_SMALL, FONT_SIZE_BODY, FONT_SIZE_SMALL, palette, with_alpha,
};
use iced::widget::{column, text, text_input as iced_text_input};
use iced::{Background, Border, Element, Length, Padding, Theme};

/// Labeled text input configuration
pub struct LabeledTextInput<'a, Message> {
    label: &'a str,
    value: &'a str,
    placeholder: &'a str,
    on_input: Box<dyn Fn(String) -> Message + 'a>,
    validation: ValidationState,
    required: bool,
    password: bool,
}

impl<'a, Message: Clone + 'a> LabeledTextInput<'a, Message> {
    pub fn new(label: &'a str, value: &'a str, on_input: impl Fn(String) -> Message + 'a) -> Self {
        Self {
            label,
            value,
            placeholder: "",
            on_input: Box::new(on_input),
            validation: ValidationState::None,
            required: false,
            password: false,
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

    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }

    pub fn view(self) -> Element<'a, Message> {
        let p = palette();
        let is_invalid = self.validation.is_invalid();

        // Extract error message before consuming validation
        let error_msg_owned: Option<String> = match &self.validation {
            ValidationState::Invalid(msg) => Some(msg.clone()),
            _ => None,
        };

        let border_color = if is_invalid { p.danger } else { p.border };

        let mut input = iced_text_input(self.placeholder, self.value)
            .on_input(self.on_input)
            .size(FONT_SIZE_BODY)
            .padding(Padding::from([8, 12]))
            .width(Length::Fill)
            .style(move |_theme: &Theme, status| {
                let border = match status {
                    iced_text_input::Status::Focused => p.accent,
                    iced_text_input::Status::Hovered => p.border_hover,
                    _ => border_color,
                };
                iced_text_input::Style {
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

        if self.password {
            input = input.secure(true);
        }

        let mut col = column![field_label(self.label, self.required), input,].spacing(4);

        if let Some(error_msg) = error_msg_owned {
            col = col.push(text(error_msg).size(FONT_SIZE_SMALL).color(p.danger));
        }

        col.into()
    }
}

/// Convenience function for creating labeled text inputs
pub fn labeled_text_input<'a, Message: Clone + 'a>(
    label: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
) -> LabeledTextInput<'a, Message> {
    LabeledTextInput::new(label, value, on_input)
}
