//! Composable icon widget using lucide-icons
//!
//! Usage:
//! ```
//! use crate::widget::icon;
//! use lucide_icons::Icon;
//!
//! icon(Icon::Settings).size(16).color(palette.text)
//! ```

use iced::Font;
use iced::widget::Text;
use lucide_icons::Icon;

/// Font for lucide icons
pub const LUCIDE_FONT: Font = Font::with_name("lucide");

/// Default icon size
pub const DEFAULT_SIZE: f32 = 16.0;

/// Create a composable icon widget
pub fn icon(icon: Icon) -> Text<'static> {
    let icon_char: char = icon.into();
    Text::new(icon_char.to_string())
        .font(LUCIDE_FONT)
        .size(DEFAULT_SIZE)
}
