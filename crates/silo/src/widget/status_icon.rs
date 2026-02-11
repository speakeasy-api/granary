//! Status icon widget with animation support
//!
//! Provides animated status indicators for task states.

use crate::appearance::{Palette, palette};
use crate::widget::icon;
use granary_types::TaskStatus;
use iced::widget::canvas::Canvas;
use iced::widget::canvas::{self, Cache, Geometry, Path, Stroke};
use iced::{Element, Length, Point, Radians, Rectangle, Renderer, Theme, mouse};
use lucide_icons::Icon;
use std::f32::consts::PI;

/// Status icon size variants
#[derive(Debug, Clone, Copy, Default)]
pub enum IconSize {
    Small, // 12px
    #[default]
    Medium, // 16px
    Large, // 24px
}

impl IconSize {
    pub fn size(&self) -> f32 {
        match self {
            IconSize::Small => 12.0,
            IconSize::Medium => 16.0,
            IconSize::Large => 24.0,
        }
    }
}

/// Animated status icon widget
pub struct StatusIcon {
    status: TaskStatus,
    size: IconSize,
    rotation: f32, // For in_progress animation (0.0 to 2*PI)
    cache: Cache,
}

impl StatusIcon {
    pub fn new(status: TaskStatus) -> Self {
        Self {
            status,
            size: IconSize::default(),
            rotation: 0.0,
            cache: Cache::new(),
        }
    }

    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    pub fn rotation(mut self, rotation: f32) -> Self {
        self.rotation = rotation;
        self.cache.clear();
        self
    }
}

#[derive(Debug, Clone, Copy)]
enum IconType {
    Circle,
    HalfCircle,
    Checkmark,
    Blocked,
}

impl<Message> canvas::Program<Message> for StatusIcon {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let center = frame.center();
            let radius = bounds.width.min(bounds.height) / 2.0 - 2.0;
            let p = palette();

            let (color, icon_type) = match self.status {
                TaskStatus::Draft => (p.text_muted, IconType::Circle),
                TaskStatus::Todo => (p.status_todo, IconType::Circle),
                TaskStatus::InProgress => (p.status_progress, IconType::HalfCircle),
                TaskStatus::Done => (p.status_done, IconType::Checkmark),
                TaskStatus::Blocked => (p.status_blocked, IconType::Blocked),
            };

            match icon_type {
                IconType::Circle => {
                    let circle = Path::circle(center, radius);
                    frame.stroke(&circle, Stroke::default().with_color(color).with_width(2.0));
                }
                IconType::HalfCircle => {
                    // Rotating half-filled circle for in_progress
                    let circle = Path::circle(center, radius);
                    frame.stroke(&circle, Stroke::default().with_color(color).with_width(2.0));

                    // Draw half fill with rotation
                    let half = Path::new(|builder| {
                        builder.move_to(center);
                        builder.arc(canvas::path::Arc {
                            center,
                            radius,
                            start_angle: Radians(self.rotation),
                            end_angle: Radians(self.rotation + PI),
                        });
                        builder.close();
                    });
                    frame.fill(&half, color);
                }
                IconType::Checkmark => {
                    let check = Path::new(|builder| {
                        let offset = radius * 0.5;
                        builder.move_to(Point::new(center.x - offset, center.y));
                        builder
                            .line_to(Point::new(center.x - offset * 0.3, center.y + offset * 0.6));
                        builder.line_to(Point::new(center.x + offset, center.y - offset * 0.5));
                    });
                    frame.stroke(&check, Stroke::default().with_color(color).with_width(2.0));
                }
                IconType::Blocked => {
                    let circle = Path::circle(center, radius);
                    frame.stroke(&circle, Stroke::default().with_color(color).with_width(2.0));
                    // Diagonal line through circle
                    let line = Path::line(
                        Point::new(center.x - radius * 0.6, center.y + radius * 0.6),
                        Point::new(center.x + radius * 0.6, center.y - radius * 0.6),
                    );
                    frame.stroke(&line, Stroke::default().with_color(color).with_width(2.0));
                }
            }
        });

        vec![geometry]
    }
}

/// Helper function to create status icon element
pub fn status_icon<'a, Message: 'a>(
    status: TaskStatus,
    size: IconSize,
    rotation: f32,
) -> Element<'a, Message> {
    let icon = StatusIcon::new(status).size(size).rotation(rotation);
    let dim = size.size();

    Canvas::new(icon)
        .width(Length::Fixed(dim))
        .height(Length::Fixed(dim))
        .into()
}

/// Simple text-based status icon (fallback)
pub fn status_icon_text<'a, Message: 'a>(
    status: &TaskStatus,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let (lucide_icon, color) = match status {
        TaskStatus::Draft => (Icon::Circle, palette.text_muted),
        TaskStatus::Todo => (Icon::Circle, palette.status_todo),
        TaskStatus::InProgress => (Icon::CircleDashed, palette.status_progress),
        TaskStatus::Done => (Icon::CircleCheck, palette.status_done),
        TaskStatus::Blocked => (Icon::Ban, palette.status_blocked),
    };

    icon(lucide_icon).size(14).color(color).into()
}
