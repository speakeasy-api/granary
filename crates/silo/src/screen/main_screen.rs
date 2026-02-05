//! Main screen module - projects and tasks panels view.
//!
//! This screen displays the main application interface with a projects panel
//! on the left and a tasks panel on the right.

use crate::appearance::{self, Palette};
use crate::message::Message;
use crate::widget::{self, icon};
use granary_types::{
    Comment, Project, Task as GranaryTask, TaskDependency, TaskPriority, TaskStatus,
};
use iced::border::Radius;
use iced::widget::{
    Column, Row, Space, button, column, container, horizontal_space, row, scrollable, text,
    text_input,
};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};
use lucide_icons::Icon;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

// Type alias to help with iced's type inference
type Renderer = iced::Renderer;

/// State passed to the main screen view function.
///
/// This struct contains all the data needed to render the main screen,
/// borrowed from the main application state.
pub struct MainScreenState<'a> {
    pub workspace: Option<&'a PathBuf>,
    pub projects: &'a [Project],
    pub tasks: &'a [GranaryTask],
    pub dependencies: &'a [TaskDependency],
    pub selected_project: Option<&'a String>,
    pub expanded_tasks: &'a HashSet<String>,
    pub new_task_title: &'a str,
    pub status_message: Option<&'a String>,
    pub loading: bool,
    pub task_comments: &'a HashMap<String, Vec<Comment>>,
    pub comment_input: &'a str,
    pub comments_loading: bool,
}

/// Renders the main screen with projects and tasks panels.
pub fn view<'a>(state: MainScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let projects_panel = view_projects_panel(&state, palette);
    let tasks_panel = view_tasks_panel(&state, palette);

    let main_content = row![
        container(projects_panel).width(Length::FillPortion(1)),
        container(tasks_panel).width(Length::FillPortion(2)),
    ]
    .spacing(24)
    .height(Length::Fill);

    let content = column![header, main_content]
        .spacing(24)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill);

    content.into()
}

/// Renders the header with title and status indicators.
fn view_header<'a>(state: &MainScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
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

    let trailing = column![loading_indicator, error_msg]
        .spacing(4)
        .align_x(iced::Alignment::End);

    widget::page_header("Projects", trailing, palette)
}

/// Renders the projects panel with project list and refresh button.
fn view_projects_panel<'a>(
    state: &MainScreenState<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let header = row![
        text("Projects").size(14).color(palette.text_secondary),
        horizontal_space(),
        widget::icon_button(Icon::Plus, Message::ShowCreateProject, palette),
        Space::with_width(8),
        widget::icon_button(Icon::RefreshCw, Message::RefreshProjects, palette),
    ]
    .align_y(iced::Alignment::Center);

    let projects_list: Element<'a, Message> = if state.projects.is_empty() {
        container(text("No projects found").size(14).color(palette.text_muted))
            .padding(24)
            .center_x(Length::Fill)
            .into()
    } else {
        let items: Vec<Element<'a, Message>> = state
            .projects
            .iter()
            .map(|p| view_project_card(p, state.selected_project, palette))
            .collect();

        scrollable(Column::from_vec(items).spacing(8).width(Length::Fill))
            .height(Length::Fill)
            .into()
    };

    let panel = column![header, Space::with_height(16), projects_list]
        .spacing(0)
        .width(Length::Fill)
        .height(Length::Fill);

    widget::card(panel, palette)
}

/// Renders a single project card with status indicator and selection state.
fn view_project_card<'a>(
    project: &'a Project,
    selected_project: Option<&'a String>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let is_selected = selected_project == Some(&project.id);
    let is_active = project.status_enum().is_active();

    let status_color = if is_active {
        palette.status_done
    } else {
        palette.text_muted
    };

    let status_dot = container(Space::new(8, 8)).style(move |_| container::Style {
        background: Some(Background::Color(status_color)),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    });

    let project_info = column![
        text(project.name.clone()).size(14).color(palette.text),
        text(project.id.clone())
            .size(11)
            .color(palette.text_muted)
            .font(iced::Font::MONOSPACE),
    ]
    .spacing(2)
    .width(Length::Fill);

    // Archive button
    let archive_btn = view_archive_button(project.id.clone(), palette);

    let content: Row<'a, Message> = row![status_dot, project_info, archive_btn]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    let border_color = if is_selected {
        palette.accent
    } else {
        palette.border
    };

    let bg_color = if is_selected {
        palette.card_hover
    } else {
        palette.card
    };

    let hover_bg = palette.card_hover;
    let hover_border = palette.border_hover;

    button(container(content).padding(12).width(Length::Fill))
        .on_press(Message::SelectProject(project.id.clone()))
        .style(move |_, status| {
            let (bg, border) = match status {
                button::Status::Hovered => (hover_bg, hover_border),
                _ => (bg_color, border_color),
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

/// Renders the archive button for a project.
fn view_archive_button<'a>(project_id: String, palette: &'a Palette) -> Element<'a, Message> {
    let text_color = palette.text_muted;
    let hover_bg = palette.card_hover;
    let danger_color = palette.status_blocked;

    button(
        container(icon(Icon::Archive).size(12).color(text_color))
            .padding(iced::Padding::from([4, 6])),
    )
    .on_press(Message::ArchiveProject(project_id))
    .style(move |_, status| {
        let (bg, txt) = match status {
            button::Status::Hovered => (hover_bg, danger_color),
            _ => (Color::TRANSPARENT, text_color),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            text_color: txt,
            border: Border {
                radius: Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        }
    })
    .into()
}

/// Renders the tasks panel with task list, input field, and refresh button.
fn view_tasks_panel<'a>(state: &MainScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let project_name = state
        .selected_project
        .and_then(|id| state.projects.iter().find(|p| &p.id == id))
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "Tasks".to_string());

    // Check if any tasks are in draft status
    let has_draft_tasks = state
        .tasks
        .iter()
        .any(|t| t.status_enum() == TaskStatus::Draft);

    let mut header_items: Vec<Element<'a, Message>> = vec![
        text(project_name)
            .size(14)
            .color(palette.text_secondary)
            .into(),
        horizontal_space().into(),
    ];

    // Show Ready button only if there are draft tasks and a project is selected
    if has_draft_tasks && let Some(project_id) = state.selected_project {
        header_items.push(widget::action_button(
            "Ready",
            Message::ReadyProject(project_id.clone()),
            palette,
        ));
        header_items.push(Space::with_width(8).into());
    }

    header_items.push(widget::icon_button(
        Icon::RefreshCw,
        Message::RefreshTasks,
        palette,
    ));

    let header = Row::from_vec(header_items)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);

    let add_task_btn: Element<'a, Message> = if let Some(project_id) = state.selected_project {
        let text_color = palette.text_secondary;
        let hover_color = palette.text;
        let hover_bg = palette.card_hover;

        button(
            container(text("+ Add task").size(14).color(text_color)).padding(Padding::from([8, 0])),
        )
        .on_press(Message::OpenCreateTaskScreen {
            project_id: Some(project_id.clone()),
        })
        .style(move |_, status| {
            let (txt, bg) = match status {
                button::Status::Hovered => (hover_color, hover_bg),
                _ => (text_color, Color::TRANSPARENT),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: txt,
                border: Border {
                    radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
    } else {
        container(
            text("Select a project to view tasks")
                .size(14)
                .color(palette.text_muted),
        )
        .padding(Padding::from([12, 0]))
        .into()
    };

    let tasks_list: Element<'a, Message> = if state.tasks.is_empty() {
        if state.selected_project.is_some() {
            container(text("No tasks yet").size(16).color(palette.text_secondary))
                .padding(48)
                .center_x(Length::Fill)
                .into()
        } else {
            Space::new(0, 0).into()
        }
    } else {
        let items: Vec<Element<'a, Message>> = state
            .tasks
            .iter()
            .map(|t| {
                view_task_row(
                    t,
                    state.expanded_tasks,
                    Some(state.dependencies),
                    state.tasks,
                    state.task_comments.get(&t.id),
                    state.comment_input,
                    state.comments_loading,
                    palette,
                )
            })
            .collect();

        scrollable(Column::from_vec(items).spacing(6).width(Length::Fill))
            .height(Length::Fill)
            .into()
    };

    let panel = column![
        header,
        Space::with_height(16),
        add_task_btn,
        Space::with_height(16),
        tasks_list,
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    widget::card(panel, palette)
}

/// Renders a single task row with status, priority, and expandable details.
#[allow(clippy::too_many_arguments)]
fn view_task_row<'a>(
    task: &'a GranaryTask,
    expanded_tasks: &'a HashSet<String>,
    dependencies: Option<&'a [TaskDependency]>,
    all_tasks: &'a [GranaryTask],
    comments: Option<&'a Vec<Comment>>,
    comment_input: &'a str,
    comments_loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let title = task.title.clone();
    let task_id = task.id.clone();
    let status = task.status_enum();
    let priority = task.priority_enum();
    let is_expanded = expanded_tasks.contains(&task_id);

    // Status icon from widget module
    let status_icon = widget::status_icon_text(&status, palette);

    // Priority badge with color coding
    let (priority_color, priority_bg) = match priority {
        TaskPriority::P0 => (
            palette.status_blocked,
            Color::from_rgba(
                palette.status_blocked.r,
                palette.status_blocked.g,
                palette.status_blocked.b,
                0.15,
            ),
        ),
        TaskPriority::P1 => (
            palette.accent,
            Color::from_rgba(palette.accent.r, palette.accent.g, palette.accent.b, 0.15),
        ),
        _ => (palette.text_muted, palette.card),
    };

    let priority_badge = container(
        text(priority.as_str())
            .size(10)
            .color(priority_color)
            .font(iced::Font::MONOSPACE),
    )
    .padding(Padding::from([3, 6]))
    .style(move |_| container::Style {
        background: Some(Background::Color(priority_bg)),
        border: Border {
            radius: Radius::from(4.0),
            ..Default::default()
        },
        ..Default::default()
    });

    // Expand indicator
    let expand_icon = icon(if is_expanded {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    })
    .size(10)
    .color(palette.text_muted);

    // Header row
    let header_row = row![
        expand_icon,
        Space::with_width(8),
        status_icon,
        Space::with_width(8),
        text(title).size(14).color(palette.text),
        horizontal_space(),
        priority_badge,
    ]
    .align_y(iced::Alignment::Center);

    // Build content based on expanded state
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

        // Owner and Due Date row
        let mut meta_items: Vec<Element<'a, Message>> = Vec::new();

        if let Some(owner) = &task.owner {
            meta_items.push(
                row![
                    text("Owner:").size(11).color(palette.text_muted),
                    text(owner.clone()).size(11).color(palette.text_secondary),
                ]
                .spacing(4)
                .into(),
            );
        }

        if let Some(due) = &task.due_at
            && !due.is_empty()
        {
            meta_items.push(
                row![
                    text("Due:").size(11).color(palette.text_muted),
                    text(due.clone()).size(11).color(palette.accent),
                ]
                .spacing(4)
                .into(),
            );
        }

        if !meta_items.is_empty() {
            details.push(Row::from_vec(meta_items).spacing(24).into());
        }

        // Tags
        if let Some(tags) = &task.tags
            && !tags.is_empty()
        {
            let accent = palette.accent;
            let tag_badges: Vec<Element<'a, Message>> = tags
                .split(',')
                .map(|tag| {
                    container(text(tag.trim()).size(10).color(accent))
                        .padding(Padding::from([2, 6]))
                        .style(move |_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                accent.r, accent.g, accent.b, 0.15,
                            ))),
                            border: Border {
                                radius: Radius::from(4.0),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .into()
                })
                .collect();

            details.push(
                row![
                    text("Tags:").size(11).color(palette.text_muted),
                    Row::from_vec(tag_badges).spacing(4),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into(),
            );
        }

        // Blocked reason
        if let Some(reason) = &task.blocked_reason
            && !reason.is_empty()
        {
            let blocked_color = palette.status_blocked;
            details.push(
                container(
                    row![
                        icon(Icon::Ban).size(12).color(blocked_color),
                        Space::with_width(6),
                        text(format!("Blocked: {}", reason))
                            .size(12)
                            .color(blocked_color),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding(Padding::from([8, 12]))
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(
                        blocked_color.r,
                        blocked_color.g,
                        blocked_color.b,
                        0.1,
                    ))),
                    border: Border {
                        radius: Radius::from(4.0),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .into(),
            );
        }

        // Dependencies
        if let Some(deps) = dependencies
            && !deps.is_empty()
        {
            let dep_items: Vec<Element<'a, Message>> = deps
                .iter()
                .filter(|d| d.task_id == task.id)
                .filter_map(|d| all_tasks.iter().find(|t| t.id == d.depends_on_task_id))
                .map(|dep_task| {
                    let dep_status = dep_task.status_enum();
                    let dep_title = dep_task.title.clone();
                    let dep_id = dep_task.id.clone();
                    let text_secondary = palette.text_secondary;
                    let hover_bg = palette.card_hover;

                    button(
                        row![
                            widget::status_icon_text(&dep_status, palette),
                            Space::with_width(6),
                            text(dep_title).size(11).color(text_secondary),
                        ]
                        .align_y(iced::Alignment::Center),
                    )
                    .on_press(Message::ToggleTaskExpand(dep_id))
                    .style(move |_, btn_status| {
                        let bg = match btn_status {
                            button::Status::Hovered => hover_bg,
                            _ => Color::TRANSPARENT,
                        };
                        button::Style {
                            background: Some(Background::Color(bg)),
                            ..Default::default()
                        }
                    })
                    .into()
                })
                .collect();

            if !dep_items.is_empty() {
                details.push(
                    column![
                        text("Depends on:").size(11).color(palette.text_muted),
                        Column::from_vec(dep_items).spacing(2),
                    ]
                    .spacing(4)
                    .into(),
                );
            }
        }

        // Timeline
        let mut timeline_items: Vec<Element<'a, Message>> = vec![
            row![
                text("Created:").size(10).color(palette.text_muted),
                text(&task.created_at).size(10).color(palette.text_muted),
            ]
            .spacing(4)
            .into(),
        ];

        if let Some(started) = &task.started_at {
            timeline_items.push(
                row![
                    text("Started:").size(10).color(palette.text_muted),
                    text(started).size(10).color(palette.status_progress),
                ]
                .spacing(4)
                .into(),
            );
        }

        if let Some(completed) = &task.completed_at {
            timeline_items.push(
                row![
                    text("Completed:").size(10).color(palette.text_muted),
                    text(completed).size(10).color(palette.status_done),
                ]
                .spacing(4)
                .into(),
            );
        }

        details.push(Row::from_vec(timeline_items).spacing(16).into());

        // Task ID
        details.push(
            row![
                text("ID:").size(10).color(palette.text_muted),
                text(&task.id)
                    .size(10)
                    .color(palette.text_muted)
                    .font(iced::Font::MONOSPACE),
            ]
            .spacing(4)
            .into(),
        );

        // Comments section
        details.push(view_comments_section(
            &task.id,
            comments,
            comment_input,
            comments_loading,
            palette,
        ));

        // Action buttons
        let mut action_btns: Vec<Element<'a, Message>> = vec![widget::action_button(
            "Edit",
            Message::OpenEditTaskScreen(task_id.clone()),
            palette,
        )];

        match status {
            TaskStatus::Todo => {
                action_btns.push(widget::action_button(
                    "Start",
                    Message::StartTask(task_id.clone()),
                    palette,
                ));
            }
            TaskStatus::Draft => {
                // Draft tasks cannot be started directly - use Ready to convert to Todo first
            }
            TaskStatus::InProgress => {
                action_btns.push(widget::action_button(
                    "Complete",
                    Message::CompleteTask(task_id.clone()),
                    palette,
                ));
                action_btns.push(widget::action_button(
                    "Block",
                    Message::BlockTask(task_id.clone()),
                    palette,
                ));
            }
            TaskStatus::Blocked => {
                action_btns.push(widget::action_button(
                    "Unblock",
                    Message::StartTask(task_id.clone()),
                    palette,
                ));
            }
            TaskStatus::Done => {
                action_btns.push(widget::action_button(
                    "Re-open",
                    Message::ReopenTask(task_id.clone()),
                    palette,
                ));
            }
        }

        let actions_row = Row::from_vec(action_btns).spacing(8);

        // Build expanded content
        let bg_details = palette.background;
        let details_container = container(Column::from_vec(details).spacing(12))
            .padding(Padding::from([12, 16]))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(bg_details)),
                border: Border {
                    radius: Radius::from(6.0),
                    ..Default::default()
                },
                ..Default::default()
            });

        column![
            header_row,
            Space::with_height(12),
            details_container,
            Space::with_height(12),
            actions_row,
        ]
        .into()
    } else {
        // Collapsed view - just header with quick action
        let quick_action: Element<'a, Message> = match status {
            TaskStatus::Todo => {
                widget::action_button("Start", Message::StartTask(task_id.clone()), palette)
            }
            TaskStatus::Draft => {
                // Draft tasks show no quick action - use Ready button to convert to Todo first
                text("draft").size(11).color(palette.text_muted).into()
            }
            TaskStatus::InProgress => {
                widget::action_button("Done", Message::CompleteTask(task_id.clone()), palette)
            }
            TaskStatus::Done => {
                widget::action_button("Re-open", Message::ReopenTask(task_id.clone()), palette)
            }
            TaskStatus::Blocked => {
                widget::action_button("Unblock", Message::StartTask(task_id.clone()), palette)
            }
        };

        row![header_row, quick_action]
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .into()
    };

    // Card wrapper
    let bg = palette.card;
    let bg_hover = palette.card_hover;
    let border = palette.border;
    let border_hov = palette.border_hover;

    button(container(content).padding(16).width(Length::Fill))
        .on_press(Message::ToggleTaskExpand(task_id))
        .style(move |_, btn_status| {
            let (bg_color, border_color) = match btn_status {
                button::Status::Hovered => (bg_hover, border_hov),
                _ => (bg, border),
            };
            button::Style {
                background: Some(Background::Color(bg_color)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS),
                },
                ..Default::default()
            }
        })
        .width(Length::Fill)
        .into()
}

/// Renders the comments section for an expanded task.
fn view_comments_section<'a>(
    _task_id: &'a str,
    comments: Option<&'a Vec<Comment>>,
    comment_input: &'a str,
    comments_loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let mut items: Vec<Element<'a, Message>> = Vec::new();

    // Section header
    items.push(text("Comments").size(11).color(palette.text_muted).into());

    // Loading indicator
    if comments_loading {
        items.push(
            text("Loading comments...")
                .size(11)
                .color(palette.text_muted)
                .into(),
        );
    } else if let Some(comments) = comments {
        if comments.is_empty() {
            items.push(
                text("No comments yet")
                    .size(11)
                    .color(palette.text_muted)
                    .into(),
            );
        } else {
            // Display each comment
            for comment in comments {
                let author = comment.author.as_deref().unwrap_or("Unknown");
                let timestamp = &comment.created_at;
                let content = &comment.content;
                let kind_badge_color = match comment.kind.as_str() {
                    "blocker" => palette.status_blocked,
                    "progress" => palette.status_progress,
                    "decision" => palette.accent,
                    _ => palette.text_muted,
                };

                let comment_card = container(
                    column![
                        row![
                            text(author).size(10).color(palette.text_secondary),
                            horizontal_space(),
                            container(text(&comment.kind).size(9).color(kind_badge_color))
                                .padding(Padding::from([1, 4]))
                                .style(move |_| container::Style {
                                    background: Some(Background::Color(Color::from_rgba(
                                        kind_badge_color.r,
                                        kind_badge_color.g,
                                        kind_badge_color.b,
                                        0.15,
                                    ))),
                                    border: Border {
                                        radius: Radius::from(3.0),
                                        ..Default::default()
                                    },
                                    ..Default::default()
                                }),
                            Space::with_width(8),
                            text(timestamp).size(9).color(palette.text_muted),
                        ]
                        .align_y(iced::Alignment::Center),
                        Space::with_height(4),
                        text(content).size(12).color(palette.text),
                    ]
                    .spacing(2),
                )
                .padding(Padding::from([8, 10]))
                .width(Length::Fill)
                .style(move |_| {
                    let bg = palette.card;
                    container::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            color: palette.border,
                            width: 1.0,
                            radius: Radius::from(4.0),
                        },
                        ..Default::default()
                    }
                });

                items.push(comment_card.into());
            }
        }
    }

    // Add comment input
    let accent = palette.accent;
    let border_hover = palette.border_hover;
    let border = palette.border;
    let bg_input = palette.input;
    let text_muted = palette.text_muted;
    let text_primary = palette.text;

    let input: text_input::TextInput<'a, Message, Theme, Renderer> =
        text_input("Add a comment...", comment_input)
            .on_input(Message::CommentInputChanged)
            .on_submit(Message::SubmitComment)
            .padding(8)
            .size(12)
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
                        radius: Radius::from(4.0),
                    },
                    icon: text_muted,
                    placeholder: text_muted,
                    value: text_primary,
                    selection: accent,
                }
            });

    let add_btn = widget::action_button("Add", Message::SubmitComment, palette);

    items.push(
        row![input, add_btn]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into(),
    );

    column(items).spacing(8).into()
}
