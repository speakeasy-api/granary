//! Date picker widget
//!
//! A date input field with format validation.

use super::{ValidationState, field_label};
use crate::appearance::{CORNER_RADIUS_SMALL, FONT_SIZE_BODY, SPACING, palette, with_alpha};
use iced::widget::{column, row, text, text_input as iced_text_input};
use iced::{Alignment, Background, Border, Element, Length, Padding, Theme};
use std::fmt;
use std::str::FromStr;

/// Date value representation
#[derive(Debug, Clone, Default)]
pub struct DateValue {
    pub year: Option<i32>,
    pub month: Option<u32>,
    pub day: Option<u32>,
}

impl DateValue {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self {
            year: Some(year),
            month: Some(month),
            day: Some(day),
        }
    }

    pub fn is_valid(&self) -> bool {
        match (self.year, self.month, self.day) {
            (Some(y), Some(m), Some(d)) => {
                (1..=12).contains(&m) && (1..=31).contains(&d) && (1900..=2100).contains(&y)
            }
            _ => false,
        }
    }
}

impl fmt::Display for DateValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.year, self.month, self.day) {
            (Some(y), Some(m), Some(d)) => write!(f, "{:04}-{:02}-{:02}", y, m, d),
            _ => Ok(()),
        }
    }
}

impl FromStr for DateValue {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse YYYY-MM-DD format
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 3 {
            let year = parts[0].parse().map_err(|_| ())?;
            let month = parts[1].parse().map_err(|_| ())?;
            let day = parts[2].parse().map_err(|_| ())?;
            Ok(Self::new(year, month, day))
        } else {
            Err(())
        }
    }
}

/// Date picker configuration
pub struct DatePicker<'a, Message> {
    label: &'a str,
    value: &'a str,
    on_change: Box<dyn Fn(String) -> Message + 'a>,
    placeholder: &'a str,
    validation: ValidationState,
    required: bool,
}

impl<'a, Message: Clone + 'a> DatePicker<'a, Message> {
    pub fn new(label: &'a str, value: &'a str, on_change: impl Fn(String) -> Message + 'a) -> Self {
        Self {
            label,
            value,
            on_change: Box::new(on_change),
            placeholder: "YYYY-MM-DD",
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

        let input = iced_text_input(self.placeholder, self.value)
            .on_input(self.on_change)
            .size(FONT_SIZE_BODY)
            .padding(Padding::from([8, 12]))
            .width(Length::Fixed(150.0))
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

        // Calendar icon hint
        let calendar_icon = text("📅").size(16.0).color(p.text_secondary);

        let input_row = row![input, calendar_icon]
            .spacing(SPACING)
            .align_y(Alignment::Center);

        let mut col = column![field_label(self.label, self.required), input_row,].spacing(4);

        if let ValidationState::Invalid(msg) = self.validation {
            col = col.push(
                text(msg)
                    .size(crate::appearance::FONT_SIZE_SMALL)
                    .color(p.status_blocked),
            );
        }

        col.into()
    }
}

/// Convenience function for creating date pickers
pub fn date_picker<'a, Message: Clone + 'a>(
    label: &'a str,
    value: &'a str,
    on_change: impl Fn(String) -> Message + 'a,
) -> DatePicker<'a, Message> {
    DatePicker::new(label, value, on_change)
}

/// Validate a date string
pub fn validate_date(value: &str) -> ValidationState {
    if value.is_empty() {
        return ValidationState::None;
    }

    match value.parse::<DateValue>() {
        Ok(date) if date.is_valid() => ValidationState::Valid,
        _ => ValidationState::Invalid("Invalid date format (use YYYY-MM-DD)".to_string()),
    }
}
