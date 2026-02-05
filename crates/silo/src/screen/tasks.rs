//! Tasks screen - list and graph views with filtering
//!
//! Displays tasks for the selected project with:
//! - Toggle between list view and dependency graph view
//! - Status filter dropdown
//! - Priority filter
//! - Search/filter input
//! - Task cards with expand/collapse

use crate::appearance::{self, Palette};
use crate::message::{Message, TaskFilter};
use crate::widget::{self, icon, task_graph};
use granary_types::{Task as GranaryTask, TaskDependency, TaskPriority, TaskStatus};
use iced::border::Radius;
use iced::widget::{
    Column, Space, button, column, container, horizontal_space, pick_list, row, scrollable, text,
    text_input,
};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};
use lucide_icons::Icon;
use std::collections::HashSet;
use std::path::PathBuf;

/// View mode for tasks
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TaskViewMode {
    #[default]
    List,
    Graph,
}

/// State for tasks screen
pub struct TasksScreenState<'a> {
    pub workspace: Option<&'a PathBuf>,
    pub project_id: Option<&'a String>,
    pub project_name: &'a str,
    pub tasks: &'a [GranaryTask],
    pub dependencies: &'a [TaskDependency],
    pub expanded_tasks: &'a HashSet<String>,
    pub filter: &'a TaskFilter,
    pub view_mode: TaskViewMode,
    pub new_task_title: &'a str,
    pub loading: bool,
}

/// Main view function for tasks screen
pub fn view<'a>(state: TasksScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let toolbar = view_toolbar(&state, palette);

    let content: Element<'a, Message> = match state.view_mode {
        TaskViewMode::List => view_list(&state, palette),
        TaskViewMode::Graph => view_graph(&state, palette),
    };

    column![header, toolbar, content]
        .spacing(16)
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_header<'a>(state: &TasksScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let back_btn = widget::icon_button(Icon::ArrowLeft, Message::BackToTaskList, palette);

    let title = text(format!("{} - Tasks", state.project_name))
        .size(24)
        .color(palette.text)
        .font(iced::Font::MONOSPACE);

    let loading_indicator: Element<'a, Message> = if state.loading {
        text("syncing...")
            .size(12)
            .color(palette.accent)
            .font(iced::Font::MONOSPACE)
            .into()
    } else {
        Space::new(0, 0).into()
    };

    let refresh_btn = widget::icon_button(Icon::RefreshCw, Message::RefreshTasks, palette);

    let create_btn = widget::action_button(
        "+ New Task",
        Message::OpenCreateTaskScreen {
            project_id: state.project_id.cloned(),
        },
        palette,
    );

    row![
        back_btn,
        Space::with_width(12),
        title,
        horizontal_space(),
        loading_indicator,
        Space::with_width(8),
        refresh_btn,
        Space::with_width(8),
        create_btn,
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

fn view_toolbar<'a>(state: &TasksScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    // View mode toggle
    let list_btn = view_mode_button("List", TaskViewMode::List, state.view_mode, palette);
    let graph_btn = view_mode_button("Graph", TaskViewMode::Graph, state.view_mode, palette);

    // Status filter options
    let status_options: Vec<&'static str> = vec!["All", "Todo", "In Progress", "Done", "Blocked"];

    let current_status = state
        .filter
        .status
        .as_ref()
        .map(|s| match s {
            TaskStatus::Todo => "Todo",
            TaskStatus::InProgress => "In Progress",
            TaskStatus::Done => "Done",
            TaskStatus::Blocked => "Blocked",
            TaskStatus::Draft => "Draft",
        })
        .unwrap_or("All");

    let filter_clone = state.filter.clone();
    let status_filter = pick_list(status_options, Some(current_status), move |selected| {
        let status = match selected {
            "Todo" => Some(TaskStatus::Todo),
            "In Progress" => Some(TaskStatus::InProgress),
            "Done" => Some(TaskStatus::Done),
            "Blocked" => Some(TaskStatus::Blocked),
            _ => None,
        };
        Message::TaskFilterChanged(TaskFilter {
            status,
            priority: filter_clone.priority.clone(),
            owner: filter_clone.owner.clone(),
            tag: filter_clone.tag.clone(),
            search: filter_clone.search.clone(),
        })
    })
    .placeholder("Status");

    // Priority filter options
    let priority_options: Vec<&'static str> = vec!["All", "P0", "P1", "P2", "P3"];

    let current_priority = state
        .filter
        .priority
        .as_ref()
        .map(|p| p.as_str())
        .unwrap_or("All");

    let filter_clone2 = state.filter.clone();
    let priority_filter = pick_list(priority_options, Some(current_priority), move |selected| {
        let priority = match selected {
            "P0" => Some(TaskPriority::P0),
            "P1" => Some(TaskPriority::P1),
            "P2" => Some(TaskPriority::P2),
            "P3" => Some(TaskPriority::P3),
            _ => None,
        };
        Message::TaskFilterChanged(TaskFilter {
            status: filter_clone2.status.clone(),
            priority,
            owner: filter_clone2.owner.clone(),
            tag: filter_clone2.tag.clone(),
            search: filter_clone2.search.clone(),
        })
    })
    .placeholder("Priority");

    // Search input
    let search_value = state.filter.search.as_deref().unwrap_or("");
    let filter_clone3 = state.filter.clone();

    let accent = palette.accent;
    let border_hover = palette.border_hover;
    let border = palette.border;
    let bg_input = palette.input;
    let text_muted = palette.text_muted;
    let text_primary = palette.text;

    let search_input = text_input("Search tasks...", search_value)
        .on_input(move |s| {
            Message::TaskFilterChanged(TaskFilter {
                status: filter_clone3.status.clone(),
                priority: filter_clone3.priority.clone(),
                owner: filter_clone3.owner.clone(),
                tag: filter_clone3.tag.clone(),
                search: if s.is_empty() { None } else { Some(s) },
            })
        })
        .padding(8)
        .width(Length::Fixed(200.0))
        .style(move |_: &Theme, status| {
            let border_color = match status {
                text_input::Status::Focused => accent,
                text_input::Status::Hovered => border_hover,
                _ => border,
            };
            text_input::Style {
                background: Background::Color(bg_input),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS),
                },
                icon: text_muted,
                placeholder: text_muted,
                value: text_primary,
                selection: accent,
            }
        });

    row![
        list_btn,
        graph_btn,
        Space::with_width(24),
        text("Filter:").size(12).color(palette.text_secondary),
        Space::with_width(8),
        status_filter,
        Space::with_width(8),
        priority_filter,
        horizontal_space(),
        search_input,
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

fn view_mode_button<'a>(
    label: &'a str,
    mode: TaskViewMode,
    current: TaskViewMode,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let is_active = mode == current;
    let bg = if is_active {
        palette.accent
    } else {
        palette.card
    };
    let text_color = if is_active {
        palette.background
    } else {
        palette.text
    };

    button(container(text(label).size(12).color(text_color)).padding(Padding::from([6, 12])))
        .on_press(Message::ToggleTaskGraphView)
        .style(move |_, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// Check if a task matches the current filter
fn matches_filter(task: &GranaryTask, filter: &TaskFilter) -> bool {
    // Status filter
    if let Some(status) = &filter.status
        && task.status_enum() != *status
    {
        return false;
    }

    // Priority filter
    if let Some(priority) = &filter.priority
        && task.priority_enum() != *priority
    {
        return false;
    }

    // Owner filter
    if let Some(owner_filter) = &filter.owner {
        if let Some(owner) = &task.owner {
            if !owner.to_lowercase().contains(&owner_filter.to_lowercase()) {
                return false;
            }
        } else {
            return false;
        }
    }

    // Tag filter
    if let Some(tag_filter) = &filter.tag {
        if let Some(tags) = &task.tags {
            if !tags.to_lowercase().contains(&tag_filter.to_lowercase()) {
                return false;
            }
        } else {
            return false;
        }
    }

    // Search filter (title, description, tags)
    if let Some(search) = &filter.search {
        let search_lower = search.to_lowercase();
        let title_match = task.title.to_lowercase().contains(&search_lower);
        let desc_match = task
            .description
            .as_ref()
            .is_some_and(|d| d.to_lowercase().contains(&search_lower));
        let tags_match = task
            .tags
            .as_ref()
            .is_some_and(|t| t.to_lowercase().contains(&search_lower));

        if !title_match && !desc_match && !tags_match {
            return false;
        }
    }

    true
}

fn view_list<'a>(state: &TasksScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let filtered_tasks: Vec<&GranaryTask> = state
        .tasks
        .iter()
        .filter(|t| matches_filter(t, state.filter))
        .collect();

    if filtered_tasks.is_empty() {
        return container(
            column![
                text("No tasks found")
                    .size(16)
                    .color(palette.text_secondary),
                text("Adjust filters or create a new task")
                    .size(13)
                    .color(palette.text_muted),
            ]
            .align_x(iced::Alignment::Center)
            .spacing(8),
        )
        .padding(48)
        .center_x(Length::Fill)
        .into();
    }

    let task_cards: Vec<Element<'a, Message>> = filtered_tasks
        .iter()
        .map(|t| view_task_card(t, state.expanded_tasks, palette))
        .collect();

    scrollable(Column::from_vec(task_cards).spacing(8).width(Length::Fill))
        .height(Length::Fill)
        .into()
}

fn view_graph<'a>(state: &TasksScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let selected = state.expanded_tasks.iter().next().cloned();

    if state.tasks.is_empty() {
        return container(
            column![
                text("No tasks to display")
                    .size(16)
                    .color(palette.text_secondary),
                text("Create tasks to see the dependency graph")
                    .size(13)
                    .color(palette.text_muted),
            ]
            .align_x(iced::Alignment::Center)
            .spacing(8),
        )
        .padding(48)
        .center_x(Length::Fill)
        .into();
    }

    scrollable(
        container(task_graph(state.tasks, state.dependencies, selected))
            .padding(24)
            .center_x(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}

fn view_task_card<'a>(
    task: &'a GranaryTask,
    expanded: &'a HashSet<String>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let is_expanded = expanded.contains(&task.id);
    let status = task.status_enum();
    let priority = task.priority_enum();

    let status_icon = widget::status_icon_text(&status, palette);

    let (status_color, status_label): (Color, &'static str) = match status {
        TaskStatus::Done => (palette.status_done, "done"),
        TaskStatus::InProgress => (palette.status_progress, "in progress"),
        TaskStatus::Blocked => (palette.status_blocked, "blocked"),
        TaskStatus::Draft => (palette.text_muted, "draft"),
        TaskStatus::Todo => (palette.status_todo, "todo"),
    };

    let priority_color = match priority {
        TaskPriority::P0 => palette.status_blocked,
        TaskPriority::P1 => palette.accent,
        _ => palette.text_muted,
    };
    let priority_label = priority.as_str();

    // Expand/collapse indicator
    let expand_indicator = icon(if is_expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    })
    .size(10)
    .color(palette.text_muted);

    let title_row = row![
        expand_indicator,
        Space::with_width(4),
        status_icon,
        Space::with_width(8),
        text(&task.title).size(14).color(palette.text),
    ]
    .align_y(iced::Alignment::Center);

    // Status badge
    let status_badge = container(
        text(status_label)
            .size(10)
            .color(status_color)
            .font(iced::Font::MONOSPACE),
    )
    .padding(Padding::from([3, 8]))
    .style(move |_| container::Style {
        background: Some(Background::Color(Color::from_rgba(
            status_color.r,
            status_color.g,
            status_color.b,
            0.15,
        ))),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Priority badge
    let priority_badge = container(
        text(priority_label)
            .size(10)
            .color(priority_color)
            .font(iced::Font::MONOSPACE),
    )
    .padding(Padding::from([3, 6]));

    let badges_row = row![status_badge, priority_badge,]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    // Action buttons
    let task_id = task.id.clone();
    let task_id_edit = task.id.clone();
    let task_id_action = task.id.clone();

    let edit_btn =
        widget::action_button("Edit", Message::OpenEditTaskScreen(task_id_edit), palette);

    let action_btn: Element<'a, Message> = match status {
        TaskStatus::Todo | TaskStatus::Draft => {
            widget::action_button("Start", Message::StartTask(task_id_action), palette)
        }
        TaskStatus::InProgress => {
            widget::action_button("Complete", Message::CompleteTask(task_id_action), palette)
        }
        _ => Space::new(0, 0).into(),
    };

    let action_buttons: Element<'a, Message> = row![edit_btn, action_btn,].spacing(8).into();

    // Build the content based on expanded state
    let content: Element<'a, Message> = if is_expanded {
        // Expanded view with full details
        let mut details: Vec<Element<'a, Message>> = Vec::new();

        // Description
        if let Some(desc) = &task.description
            && !desc.is_empty()
        {
            details.push(
                column![
                    text("Description").size(11).color(palette.text_muted),
                    text(desc.clone()).size(13).color(palette.text_secondary),
                ]
                .spacing(4)
                .into(),
            );
        }

        // Owner
        if let Some(owner) = &task.owner {
            details.push(
                row![
                    text("Owner:").size(11).color(palette.text_muted),
                    text(owner.clone()).size(11).color(palette.text_secondary),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Tags
        if let Some(tags) = &task.tags
            && !tags.is_empty()
        {
            details.push(
                row![
                    text("Tags:").size(11).color(palette.text_muted),
                    text(tags.clone()).size(11).color(palette.accent),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Blocked reason
        if let Some(reason) = &task.blocked_reason
            && !reason.is_empty()
        {
            details.push(
                row![
                    text("Blocked:").size(11).color(palette.status_blocked),
                    text(reason.clone()).size(11).color(palette.status_blocked),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Due date
        if let Some(due) = &task.due_at {
            details.push(
                row![
                    text("Due:").size(11).color(palette.text_muted),
                    text(due.clone()).size(11).color(palette.text_secondary),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Started at
        if let Some(started) = &task.started_at {
            details.push(
                row![
                    text("Started:").size(11).color(palette.text_muted),
                    text(started.clone()).size(11).color(palette.text_secondary),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Completed at
        if let Some(completed) = &task.completed_at {
            details.push(
                row![
                    text("Completed:").size(11).color(palette.text_muted),
                    text(completed.clone()).size(11).color(palette.status_done),
                ]
                .spacing(8)
                .into(),
            );
        }

        // Task ID
        details.push(
            row![
                text("ID:").size(10).color(palette.text_muted),
                text(task_id.clone())
                    .size(10)
                    .color(palette.text_muted)
                    .font(iced::Font::MONOSPACE),
            ]
            .spacing(8)
            .into(),
        );

        // Created/Updated
        let bg_secondary = palette.background;
        details.push(
            row![
                text("Created:").size(10).color(palette.text_muted),
                text(task.created_at.clone())
                    .size(10)
                    .color(palette.text_muted),
                Space::with_width(16),
                text("Updated:").size(10).color(palette.text_muted),
                text(task.updated_at.clone())
                    .size(10)
                    .color(palette.text_muted),
            ]
            .spacing(4)
            .into(),
        );

        let details_section = if details.is_empty() {
            column![
                text("No additional details")
                    .size(12)
                    .color(palette.text_muted)
            ]
        } else {
            Column::from_vec(details).spacing(8)
        };

        column![
            row![
                column![title_row, Space::with_height(4), badges_row,].width(Length::Fill),
                action_buttons,
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            Space::with_height(12),
            container(details_section)
                .padding(Padding::from([12, 16]))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(bg_secondary)),
                    border: Border {
                        radius: Radius::from(6.0),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .into()
    } else {
        // Collapsed view
        row![
            column![title_row, Space::with_height(4), badges_row,].width(Length::Fill),
            action_buttons,
        ]
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .into()
    };

    let bg_card = palette.card;
    let bg_card_hover = palette.card_hover;
    let border_color = palette.border;
    let border_hover = palette.border_hover;

    button(container(content).padding(16).width(Length::Fill))
        .on_press(Message::ToggleTaskExpand(task_id))
        .style(move |_, status| {
            let (bg, border) = match status {
                button::Status::Hovered => (bg_card_hover, border_hover),
                _ => (bg_card, border_color),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS),
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .into()
}
