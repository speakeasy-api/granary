//! Projects list screen with enhanced project cards.
//!
//! This screen displays a fullscreen list of projects with enhanced cards
//! showing full metadata including description, owner, tags, and task progress.

use crate::appearance::Palette;
use crate::message::Message;
use crate::widget::{self, ProjectCardData, TaskStats};
use granary_types::Project;
use iced::widget::{Column, Space, column, container, row, scrollable, text};
use iced::{Element, Length};
use lucide_icons::Icon;
use std::collections::HashMap;

/// State passed to the projects screen view function.
///
/// This struct contains all the data needed to render the projects screen,
/// borrowed from the main application state.
pub struct ProjectsScreenState<'a> {
    /// List of all projects
    pub projects: &'a [Project],
    /// Currently selected project ID (if any)
    pub selected_project: Option<&'a String>,
    /// Task statistics per project (project_id -> stats)
    pub task_stats: &'a HashMap<String, TaskStats>,
    /// Whether data is currently loading
    pub loading: bool,
    /// Status/error message to display
    pub status_message: Option<&'a String>,
}

/// Renders the projects screen with a header and scrollable list of enhanced project cards.
pub fn view<'a>(state: ProjectsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let content = view_content(&state, palette);

    let screen = column![header, Space::with_height(24), content]
        .spacing(0)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill);

    screen.into()
}

/// Renders the header with title, create button, and refresh button.
fn view_header<'a>(state: &ProjectsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
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

    let create_btn = widget::add_button(Message::ShowCreateProject, palette);
    let refresh_btn = widget::icon_button(Icon::RefreshCw, Message::RefreshProjects, palette);

    let trailing = row![
        status_column,
        Space::with_width(16),
        create_btn,
        Space::with_width(8),
        refresh_btn,
    ]
    .align_y(iced::Alignment::Center);

    widget::page_header("Projects", trailing, palette)
}

/// Renders the main content area with either project cards or empty state.
fn view_content<'a>(state: &ProjectsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    if state.projects.is_empty() {
        view_empty_state(palette)
    } else {
        view_projects_list(state, palette)
    }
}

/// Renders the empty state when no projects exist.
fn view_empty_state<'a>(palette: &'a Palette) -> Element<'a, Message> {
    let empty_content = column![
        text("No projects yet")
            .size(18)
            .color(palette.text_secondary),
        Space::with_height(8),
        text("Create your first project.")
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

/// Renders the scrollable list of enhanced project cards.
fn view_projects_list<'a>(
    state: &ProjectsScreenState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let cards: Vec<Element<'a, Message>> = state
        .projects
        .iter()
        .map(|project| {
            let is_selected = state.selected_project == Some(&project.id);
            let task_stats = state.task_stats.get(&project.id).cloned();

            let data = ProjectCardData {
                project,
                is_selected,
                task_stats,
            };

            widget::project_card(data, palette)
        })
        .collect();

    let cards_column = Column::from_vec(cards).spacing(12).width(Length::Fill);

    widget::card(
        scrollable(cards_column)
            .height(Length::Fill)
            .width(Length::Fill),
        palette,
    )
}
