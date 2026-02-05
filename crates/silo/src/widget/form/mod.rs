//! Form components for Silo
//!
//! Provides styled form widgets including text inputs, text areas,
//! selects, and date pickers with consistent theming and validation states.

pub mod date_picker;
pub mod select;
pub mod text_area;
pub mod text_input;

pub use date_picker::{DatePicker, DateValue, date_picker, validate_date};
pub use select::{Select, select};
pub use text_area::{TextArea, text_area};
pub use text_input::{LabeledTextInput, labeled_text_input};

use crate::appearance::{FONT_SIZE_SMALL, palette};
use iced::Element;
use iced::widget::text;

/// Form field validation state
#[derive(Debug, Clone, Default)]
pub enum ValidationState {
    #[default]
    None,
    Valid,
    Invalid(String),
}

impl ValidationState {
    pub fn is_invalid(&self) -> bool {
        matches!(self, ValidationState::Invalid(_))
    }

    pub fn error_message(&self) -> Option<&str> {
        match self {
            ValidationState::Invalid(msg) => Some(msg),
            _ => None,
        }
    }
}

/// Helper to create a form field label
pub fn field_label<'a, Message: 'a>(label: &'a str, required: bool) -> Element<'a, Message> {
    let p = palette();
    let label_text = if required {
        format!("{} *", label)
    } else {
        label.to_string()
    };

    text(label_text)
        .size(FONT_SIZE_SMALL)
        .color(p.text_secondary)
        .into()
}

/// Helper to create an error message element
pub fn error_message<'a, Message: 'a>(message: &'a str) -> Element<'a, Message> {
    let p = palette();
    text(message)
        .size(FONT_SIZE_SMALL)
        .color(p.status_blocked) // Using status_blocked as danger/error color
        .into()
}
