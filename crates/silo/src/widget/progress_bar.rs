//! Progress bar widget with percentage display
//!
//! A horizontal progress bar that shows completion percentage
//! with a text label.

use crate::appearance::{CORNER_RADIUS_SMALL, Palette, palette};
use iced::border::Radius;
use iced::widget::{container, row, text};
use iced::{Background, Border, Element, Length};

/// Progress bar configuration
#[derive(Debug, Clone)]
pub struct ProgressBar {
    /// Progress value from 0.0 to 1.0
    value: f32,
    /// Optional label (defaults to percentage)
    label: Option<String>,
    /// Bar height in pixels
    height: f32,
    /// Whether to show percentage text
    show_percentage: bool,
}

impl ProgressBar {
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            label: None,
            height: 8.0,
            show_percentage: true,
        }
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }

    /// Build the progress bar element
    pub fn view<'a, Message: 'a>(self) -> Element<'a, Message> {
        let p = palette();
        let percentage = (self.value * 100.0) as u32;

        let bar = progress_bar_inner(self.value, self.height, p);

        if self.show_percentage {
            let label_text = self.label.unwrap_or_else(|| format!("{}%", percentage));
            row![bar, text(label_text).size(12.0).color(p.text_secondary)]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into()
        } else {
            bar
        }
    }
}

fn progress_bar_inner<'a, Message: 'a>(
    value: f32,
    height: f32,
    p: &'static Palette,
) -> Element<'a, Message> {
    // Track (background)
    let track_bg = p.card;
    let fill_bg = p.accent;

    // Use FillPortion with two siblings so they split space proportionally
    let fill_portion = ((value * 100.0) as u16).max(1);
    let remaining_portion = (((1.0 - value) * 100.0) as u16).max(1);
    let mut bar_row = iced::widget::Row::new().height(Length::Fill);
    if value > 0.0 {
        bar_row = bar_row.push(
            container("")
                .width(Length::FillPortion(fill_portion))
                .height(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(fill_bg)),
                    border: Border {
                        radius: Radius::from(CORNER_RADIUS_SMALL),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        );
    }
    if value < 1.0 {
        bar_row = bar_row.push(
            container("")
                .width(Length::FillPortion(remaining_portion))
                .height(Length::Fill),
        );
    }
    container(bar_row)
        .width(Length::Fill)
        .height(Length::Fixed(height))
        .style(move |_| container::Style {
            background: Some(Background::Color(track_bg)),
            border: Border {
                radius: Radius::from(CORNER_RADIUS_SMALL),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Convenience function for creating progress bars
pub fn progress_bar(value: f32) -> ProgressBar {
    ProgressBar::new(value)
}
