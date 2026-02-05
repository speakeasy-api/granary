//! Theme and appearance module for Silo
//!
//! This module contains the color palette, theme constants, and styling utilities
//! following the Iced development best practices.

pub mod button;

use iced::Color;
use std::sync::LazyLock;

// Layout constants
pub const CORNER_RADIUS: f32 = 8.0;
pub const CORNER_RADIUS_SMALL: f32 = 6.0;
pub const CORNER_RADIUS_LARGE: f32 = 12.0;
pub const BORDER_WIDTH: f32 = 1.0;
pub const SPACING: u16 = 8;
pub const SPACING_LARGE: u16 = 16;
pub const PADDING: u16 = 12;
pub const PADDING_LARGE: u16 = 20;

// Font sizes
pub const FONT_SIZE_SMALL: f32 = 11.0;
pub const FONT_SIZE_BODY: f32 = 14.0;
pub const FONT_SIZE_TITLE: f32 = 28.0;
pub const FONT_SIZE_HERO: f32 = 72.0;

/// Color palette for the application theme
#[derive(Debug, Clone)]
pub struct Palette {
    // Backgrounds
    pub background: Color,
    pub surface: Color,
    pub card: Color,
    pub card_hover: Color,
    pub input: Color,

    // Text
    pub text: Color,
    pub text_secondary: Color,
    pub text_muted: Color,

    // Borders
    pub border: Color,
    pub border_hover: Color,

    // Accent colors
    pub accent: Color,
    pub accent_light: Color,

    // Status colors
    pub status_done: Color,
    pub status_progress: Color,
    pub status_blocked: Color,
    pub status_todo: Color,

    // Danger/error colors (for validation, destructive actions)
    pub danger: Color,
    pub danger_light: Color,

    // Success colors (for confirmation, positive states)
    pub success: Color,
    pub success_light: Color,

    // Warning colors
    pub warning: Color,
    pub warning_light: Color,

    // Focus state
    pub focus_ring: Color,
}

/// Dark theme palette inspired by granary-web (zinc + amber)
pub static DARK: LazyLock<Palette> = LazyLock::new(|| Palette {
    // Backgrounds - zinc scale
    background: Color::from_rgb(0.035, 0.035, 0.043), // zinc-900 #09090b
    surface: Color::from_rgb(0.094, 0.094, 0.106),    // zinc-950 #18181b
    card: Color::from_rgba(0.153, 0.153, 0.165, 0.6), // zinc-800/60
    card_hover: Color::from_rgba(0.153, 0.153, 0.165, 0.8), // zinc-800/80
    input: Color::from_rgba(0.153, 0.153, 0.165, 0.4), // zinc-800/40

    // Text - zinc scale
    text: Color::from_rgb(0.957, 0.957, 0.961), // zinc-100 #f4f4f5
    text_secondary: Color::from_rgb(0.631, 0.631, 0.667), // zinc-400 #a1a1aa
    text_muted: Color::from_rgb(0.443, 0.443, 0.475), // zinc-500

    // Borders
    border: Color::from_rgba(0.153, 0.153, 0.165, 0.5), // zinc-800/50
    border_hover: Color::from_rgb(0.706, 0.325, 0.035), // amber-600 #b45309

    // Accent - amber scale
    accent: Color::from_rgb(0.706, 0.325, 0.035), // amber-600 #b45309
    accent_light: Color::from_rgb(0.961, 0.827, 0.596), // amber-200

    // Status colors
    status_done: Color::from_rgb(0.34, 0.80, 0.46), // green-400
    status_progress: Color::from_rgb(0.38, 0.62, 0.98), // blue-400
    status_blocked: Color::from_rgb(0.98, 0.45, 0.45), // red-400
    status_todo: Color::from_rgb(0.631, 0.631, 0.667), // zinc-400

    // Danger - red scale
    danger: Color::from_rgb(0.86, 0.25, 0.25), // red-600
    danger_light: Color::from_rgb(0.98, 0.45, 0.45), // red-400

    // Success - green scale
    success: Color::from_rgb(0.13, 0.53, 0.33), // green-600
    success_light: Color::from_rgb(0.34, 0.80, 0.46), // green-400

    // Warning - amber scale
    warning: Color::from_rgb(0.92, 0.58, 0.0), // amber-500
    warning_light: Color::from_rgb(0.99, 0.78, 0.23), // amber-300

    // Focus ring
    focus_ring: Color::from_rgba(0.38, 0.62, 0.98, 0.5), // blue-400/50
});

// Color utility functions

/// Lighten a color by the specified amount (0.0 - 1.0)
pub fn lighten(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r + amount).min(1.0),
        g: (color.g + amount).min(1.0),
        b: (color.b + amount).min(1.0),
        a: color.a,
    }
}

/// Darken a color by the specified amount (0.0 - 1.0)
pub fn darken(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r - amount).max(0.0),
        g: (color.g - amount).max(0.0),
        b: (color.b - amount).max(0.0),
        a: color.a,
    }
}

/// Create a new color with the specified alpha value
pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}

/// Get the current palette (currently always dark theme)
pub fn palette() -> &'static Palette {
    &DARK
}
