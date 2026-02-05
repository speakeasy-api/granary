//! Log viewer widget with follow mode
//!
//! A scrollable text viewer for displaying log output with
//! auto-scroll (follow mode) capability.

use crate::appearance::{CORNER_RADIUS_SMALL, PADDING, SPACING, palette};
use iced::border::Radius;
use iced::widget::{Space, button, checkbox, column, container, row, scrollable, text};
use iced::{Background, Border, Element, Font, Length, Padding as IcedPadding, Theme};

/// Log viewer configuration
pub struct LogViewer<'a, Message> {
    lines: &'a [String],
    follow: bool,
    title: String,
    on_toggle_follow: Option<Box<dyn Fn(bool) -> Message + 'a>>,
    on_clear: Option<Message>,
}

impl<'a, Message: Clone + 'a> LogViewer<'a, Message> {
    /// Create a new log viewer with the given lines
    pub fn new(lines: &'a [String]) -> Self {
        Self {
            lines,
            follow: false,
            title: String::from("Logs"),
            on_toggle_follow: None,
            on_clear: None,
        }
    }

    /// Set the title displayed in the header
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set whether follow mode is enabled
    pub fn follow(mut self, follow: bool) -> Self {
        self.follow = follow;
        self
    }

    /// Set the callback for when follow mode is toggled
    pub fn on_toggle_follow<F>(mut self, f: F) -> Self
    where
        F: Fn(bool) -> Message + 'a,
    {
        self.on_toggle_follow = Some(Box::new(f));
        self
    }

    /// Set the message to emit when clear is pressed
    pub fn on_clear(mut self, msg: Message) -> Self {
        self.on_clear = Some(msg);
        self
    }

    /// Build the log viewer element
    pub fn view(self) -> Element<'a, Message> {
        let p = palette();
        let line_count = self.lines.len();

        // Header: title, line count, follow checkbox, clear button
        let title_text = text(self.title).size(14.0).color(p.text);
        let count_text = text(format!("{} lines", line_count))
            .size(12.0)
            .color(p.text_secondary);

        let mut header_row = row![
            title_text,
            Space::with_width(Length::Fixed(12.0)),
            count_text,
        ]
        .align_y(iced::Alignment::Center);

        // Add flexible space to push controls to the right
        header_row = header_row.push(Space::with_width(Length::Fill));

        // Follow mode checkbox
        if let Some(on_toggle) = self.on_toggle_follow {
            let follow_checkbox = checkbox("Follow", self.follow)
                .on_toggle(on_toggle)
                .text_size(12.0)
                .spacing(4);
            header_row = header_row.push(follow_checkbox);
            header_row = header_row.push(Space::with_width(Length::Fixed(12.0)));
        }

        // Clear button
        if let Some(clear_msg) = self.on_clear {
            let clear_btn = clear_button("Clear", clear_msg, p);
            header_row = header_row.push(clear_btn);
        }

        let header = container(header_row.spacing(SPACING))
            .padding(IcedPadding::from([8, PADDING]))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(p.card)),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                },
                ..Default::default()
            });

        // Log content - scrollable container with monospace text
        let log_content: Element<'a, Message> = if self.lines.is_empty() {
            container(
                text("No logs yet...")
                    .size(12.0)
                    .color(p.text_muted)
                    .font(Font::MONOSPACE),
            )
            .padding(PADDING)
            .into()
        } else {
            let lines_col = column(self.lines.iter().map(|line| {
                text(line)
                    .size(12.0)
                    .color(p.text_secondary)
                    .font(Font::MONOSPACE)
                    .into()
            }))
            .spacing(2)
            .padding(PADDING);

            let mut scroller = scrollable(lines_col)
                .width(Length::Fill)
                .height(Length::Fill);

            // Auto-scroll to bottom when follow mode is enabled
            if self.follow {
                scroller = scroller.anchor_bottom();
            }

            scroller.into()
        };

        let content_container = container(log_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(p.card)),
                border: Border {
                    color: p.border,
                    width: 1.0,
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                },
                ..Default::default()
            });

        // Combine header and content
        column![header, content_container]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Helper function to create a clear button with consistent styling
fn clear_button<'a, Message: Clone + 'a>(
    label: &'a str,
    msg: Message,
    p: &'static crate::appearance::Palette,
) -> Element<'a, Message> {
    let text_color = p.text_secondary;
    let bg_normal = p.card;
    let bg_hover = p.card_hover;
    let border_normal = p.border;

    button(container(text(label).size(11.0).color(text_color)).padding(IcedPadding::from([4, 10])))
        .on_press(msg)
        .style(move |_: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => bg_hover,
                _ => bg_normal,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_normal,
                    width: 1.0,
                    radius: Radius::from(CORNER_RADIUS_SMALL),
                },
                ..Default::default()
            }
        })
        .into()
}

/// Convenience function for creating log viewers
pub fn log_viewer<'a, Message: Clone + 'a>(lines: &'a [String]) -> LogViewer<'a, Message> {
    LogViewer::new(lines)
}
