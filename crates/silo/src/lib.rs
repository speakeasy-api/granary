pub mod app;
pub mod appearance;
pub mod config;
pub mod granary_cli;
pub mod message;
pub mod screen;
pub mod util;
pub mod widget;

pub use app::Silo;
pub use message::Message;

pub fn run() -> iced::Result {
    iced::application("Silo", Silo::update, Silo::view)
        .subscription(Silo::subscription)
        .theme(|_| iced::Theme::Dark)
        .antialiasing(true)
        .font(lucide_icons::LUCIDE_FONT_BYTES)
        .run_with(Silo::new)
}
