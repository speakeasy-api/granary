//! Initiatives list screen - display all initiatives with summary cards.
//!
//! This screen displays a fullscreen list of initiatives with enhanced cards
//! showing status badge, progress bar, and summary stats (projects, tasks, blockers).

use crate::appearance::Palette;
use crate::message::Message;
use crate::widget::{self, initiative_card};
use granary_types::Initiative;
use iced::widget::{Column, Space, column, container, row, scrollable, text};
use iced::{Element, Length};
use lucide_icons::Icon;

/// State passed to the initiatives screen view function.
pub struct InitiativesScreenState<'a> {
    pub initiatives: &'a [Initiative],
    pub loading: bool,
    pub status_message: Option<&'a String>,
}

/// Renders the initiatives screen with a header and scrollable list of initiative cards.
pub fn view<'a>(state: InitiativesScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let content = view_content(&state, palette);

    let screen = column![header, Space::with_height(24), content]
        .spacing(0)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill);

    screen.into()
}

/// Renders the header with title and refresh button.
fn view_header<'a>(
    state: &InitiativesScreenState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let loading_indicator = if state.loading {
        text("syncing...")
            .size(12)
            .color(palette.accent)
            .font(iced::Font::MONOSPACE)
    } else {
        text("").size(12)
    };

    let error_msg = if let Some(err) = state.status_message {
        text(err.as_str()).size(12).color(palette.status_blocked)
    } else {
        text("").size(12)
    };

    let status_column = column![loading_indicator, error_msg]
        .spacing(4)
        .align_x(iced::Alignment::End);

    let refresh_btn = widget::icon_button(Icon::RefreshCw, Message::RefreshInitiatives, palette);

    let trailing =
        row![status_column, Space::with_width(16), refresh_btn].align_y(iced::Alignment::Center);

    widget::page_header("Initiatives", trailing, palette)
}

/// Renders the main content area with either initiative cards or empty state.
fn view_content<'a>(
    state: &InitiativesScreenState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    if state.initiatives.is_empty() {
        view_empty_state(palette)
    } else {
        view_initiatives_list(state, palette)
    }
}

/// Renders the empty state when no initiatives exist.
fn view_empty_state<'a>(palette: &'a Palette) -> Element<'a, Message> {
    let empty_content = column![
        text("No initiatives found")
            .size(18)
            .color(palette.text_secondary),
        Space::with_height(8),
        text("Initiatives coordinate work across multiple projects.")
            .size(14)
            .color(palette.text_muted),
    ]
    .align_x(iced::Alignment::Center);

    container(empty_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

/// Renders the scrollable list of initiative cards.
fn view_initiatives_list<'a>(
    state: &InitiativesScreenState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let cards: Vec<Element<'a, Message>> = state
        .initiatives
        .iter()
        .map(|initiative| {
            // Note: summaries are loaded on-demand when selecting an initiative
            // The list view shows cards without detailed summary stats initially
            initiative_card(initiative, None, palette)
        })
        .collect();

    let cards_column = Column::from_vec(cards)
        .spacing(12)
        .width(Length::Fill)
        .height(Length::Shrink);

    widget::card(
        scrollable(cards_column)
            .height(Length::Fill)
            .width(Length::Fill),
        palette,
    )
}
