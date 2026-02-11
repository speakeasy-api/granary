//! Edit task screen - form for modifying existing tasks
//!
//! Pre-populated form with all task fields plus blocking support.

use crate::appearance::{self, Palette};
use crate::message::Message;
use crate::widget::{self, form};
use granary_types::{Task as GranaryTask, TaskPriority, TaskStatus};
use iced::border::Radius;
use iced::widget::{
    Column, Space, button, column, container, horizontal_space, pick_list, row, scrollable, text,
    text_editor, text_input,
};
use iced::{Background, Border, Element, Length, Padding, Theme};
use lucide_icons::Icon;

/// Form state for task editing
#[derive(Debug, Clone, Default)]
pub struct EditTaskForm {
    pub task_id: String,
    pub title: String,
    pub description: String,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub owner: String,
    pub due_date: String,
    pub tags: String,
    pub dependency_input: String,
    pub dependencies: Vec<String>,
    pub block_reason: String,
    pub show_block_dialog: bool,
    pub validation_errors: std::collections::HashMap<String, String>,
    pub submitting: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl EditTaskForm {
    /// Create form from existing task
    pub fn from_task(task: &GranaryTask, existing_deps: Vec<String>) -> Self {
        Self {
            task_id: task.id.clone(),
            title: task.title.clone(),
            description: task.description.clone().unwrap_or_default(),
            priority: task.priority_enum(),
            status: task.status_enum(),
            owner: task.owner.clone().unwrap_or_default(),
            due_date: task.due_at.clone().unwrap_or_default(),
            tags: task.tags.clone().unwrap_or_default(),
            dependencies: existing_deps,
            created_at: task.created_at.clone(),
            updated_at: task.updated_at.clone(),
            ..Default::default()
        }
    }

    pub fn validate(&mut self) -> bool {
        self.validation_errors.clear();

        if self.title.trim().is_empty() {
            self.validation_errors
                .insert("title".to_string(), "Title is required".to_string());
        }

        self.validation_errors.is_empty()
    }
}

/// State passed to edit task view
pub struct EditTaskScreenState<'a> {
    pub project_name: &'a str,
    pub form: &'a EditTaskForm,
    pub description_content: &'a text_editor::Content,
    pub available_tasks: &'a [GranaryTask],
    pub loading: bool,
}

/// Main view function
pub fn view<'a>(state: EditTaskScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let form = view_form(&state, palette);

    // Block dialog overlay
    let content = column![header, form]
        .spacing(24)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill);

    if state.form.show_block_dialog {
        // Overlay block reason dialog
        iced::widget::stack![content, view_block_dialog(state.form, palette),].into()
    } else {
        content.into()
    }
}

fn view_header<'a>(state: &EditTaskScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let back_btn = widget::icon_button(Icon::ArrowLeft, Message::EditTaskFormCancel, palette);

    let title = text(format!("Edit Task - {}", state.project_name))
        .size(20)
        .color(palette.text);

    let task_id = text(&state.form.task_id)
        .size(12)
        .color(palette.text_muted)
        .font(iced::Font::MONOSPACE);

    row![
        back_btn,
        Space::with_width(12),
        column![title, task_id].spacing(4),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

fn view_form<'a>(state: &EditTaskScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let form = state.form;

    // Title field (required)
    let title_field = view_field(
        "Title",
        true,
        view_text_input(
            "Enter task title...",
            &form.title,
            Message::EditTaskFormTitle,
            palette,
        ),
        form.validation_errors.get("title"),
        palette,
    );

    // Description field
    let description_field = view_field(
        "Description",
        false,
        view_description_editor(state.description_content, palette),
        None,
        palette,
    );

    // Priority picker
    let priorities = vec![
        TaskPriority::P0,
        TaskPriority::P1,
        TaskPriority::P2,
        TaskPriority::P3,
        TaskPriority::P4,
    ];
    let priority_field = view_field(
        "Priority",
        false,
        view_pick_list(
            priorities,
            Some(form.priority.clone()),
            Message::EditTaskFormPriority,
            palette,
        ),
        None,
        palette,
    );

    // Status picker (all statuses for edit)
    let statuses = vec![
        TaskStatus::Draft,
        TaskStatus::Todo,
        TaskStatus::InProgress,
        TaskStatus::Done,
        TaskStatus::Blocked,
    ];
    let status_field = view_field(
        "Status",
        false,
        view_pick_list_wide(
            statuses,
            Some(form.status.clone()),
            Message::EditTaskFormStatus,
            palette,
        ),
        None,
        palette,
    );

    // Owner field
    let owner_field = view_field(
        "Owner",
        false,
        view_text_input(
            "Enter owner...",
            &form.owner,
            Message::EditTaskFormOwner,
            palette,
        ),
        None,
        palette,
    );

    // Due date field
    let due_date_field = view_field(
        "Due Date",
        false,
        view_text_input_fixed(
            "YYYY-MM-DD",
            &form.due_date,
            Message::EditTaskFormDueDate,
            150.0,
            palette,
        ),
        None,
        palette,
    );

    // Tags field
    let tags_field = view_field(
        "Tags",
        false,
        view_text_input(
            "tag1, tag2, tag3",
            &form.tags,
            Message::EditTaskFormTags,
            palette,
        ),
        None,
        palette,
    );

    // Dependencies section
    let deps_header = text("Dependencies").size(14).color(palette.text_secondary);

    let dep_input = view_text_input(
        "Search tasks by name...",
        &form.dependency_input,
        Message::EditTaskFormDependency,
        palette,
    );

    // Filter available tasks based on input (search by title)
    let search_results: Element<'a, Message> = if form.dependency_input.trim().is_empty() {
        Space::new(0, 0).into()
    } else {
        let query = form.dependency_input.to_lowercase();
        let matching_tasks: Vec<_> = state
            .available_tasks
            .iter()
            .filter(|t| {
                t.title.to_lowercase().contains(&query)
                    && !form.dependencies.contains(&t.id)
                    && t.id != form.task_id // Can't depend on self
            })
            .take(5)
            .collect();

        if matching_tasks.is_empty() {
            text("No matching tasks found")
                .size(12)
                .color(palette.text_muted)
                .into()
        } else {
            let items: Vec<Element<'a, Message>> = matching_tasks
                .into_iter()
                .map(|task| {
                    let task_id = task.id.clone();
                    let text_color = palette.text;
                    let text_muted = palette.text_muted;
                    let hover_bg = palette.card_hover;

                    button(
                        container(
                            column![
                                text(&task.title).size(13).color(text_color),
                                text(&task.id)
                                    .size(10)
                                    .color(text_muted)
                                    .font(iced::Font::MONOSPACE),
                            ]
                            .spacing(2),
                        )
                        .padding(Padding::from([8, 12]))
                        .width(Length::Fill),
                    )
                    .on_press(Message::EditTaskFormSelectDependency(task_id))
                    .width(Length::Fill)
                    .style(move |_, status| {
                        let bg = match status {
                            button::Status::Hovered => hover_bg,
                            _ => iced::Color::TRANSPARENT,
                        };
                        button::Style {
                            background: Some(Background::Color(bg)),
                            border: Border {
                                radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    })
                    .into()
                })
                .collect();

            let bg = palette.surface;
            let border_color = palette.border;

            container(Column::from_vec(items).spacing(2).width(Length::Fill))
                .padding(4)
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        color: border_color,
                        width: 1.0,
                        radius: Radius::from(appearance::CORNER_RADIUS),
                    },
                    ..Default::default()
                })
                .into()
        }
    };

    let dep_list: Element<'a, Message> = if form.dependencies.is_empty() {
        text("No dependencies")
            .size(12)
            .color(palette.text_muted)
            .into()
    } else {
        Column::from_vec(
            form.dependencies
                .iter()
                .map(|dep_id| {
                    let task_title = state
                        .available_tasks
                        .iter()
                        .find(|t| &t.id == dep_id)
                        .map(|t| t.title.as_str())
                        .unwrap_or(dep_id.as_str());

                    row![
                        column![
                            text(task_title).size(12).color(palette.text),
                            text(dep_id)
                                .size(10)
                                .color(palette.text_muted)
                                .font(iced::Font::MONOSPACE),
                        ]
                        .spacing(1),
                        horizontal_space(),
                        widget::icon_button(
                            Icon::X,
                            Message::EditTaskFormRemoveDependency(dep_id.clone()),
                            palette
                        ),
                    ]
                    .align_y(iced::Alignment::Center)
                    .width(Length::Fill)
                    .into()
                })
                .collect(),
        )
        .spacing(4)
        .width(Length::Fill)
        .into()
    };

    let dependencies_section = column![deps_header, dep_input, search_results, dep_list].spacing(8);

    // Metadata section
    let metadata = row![
        text(format!("Created: {}", format_timestamp(&form.created_at)))
            .size(11)
            .color(palette.text_muted),
        Space::with_width(24),
        text(format!("Updated: {}", format_timestamp(&form.updated_at)))
            .size(11)
            .color(palette.text_muted),
    ];

    // Form actions
    let cancel_btn = widget::action_button("Cancel", Message::EditTaskFormCancel, palette);

    let block_btn = view_block_button(&form.task_id, palette);

    let save_btn = if form.submitting {
        view_submit_button_disabled("Saving...", palette)
    } else {
        view_submit_button("Save Changes", Message::EditTaskFormSubmit, palette)
    };

    let actions = row![
        horizontal_space(),
        cancel_btn,
        Space::with_width(8),
        block_btn,
        Space::with_width(8),
        save_btn,
    ]
    .align_y(iced::Alignment::Center);

    let bg = palette.surface;
    let border_color = palette.border;

    let form_content = column![
        title_field,
        description_field,
        row![priority_field, Space::with_width(24), status_field].align_y(iced::Alignment::End),
        owner_field,
        due_date_field,
        tags_field,
        Space::with_height(16),
        dependencies_section,
        Space::with_height(16),
        metadata,
        Space::with_height(24),
        actions,
    ]
    .spacing(16)
    .max_width(600);

    scrollable(
        container(form_content)
            .padding(24)
            .style(move |_| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS_LARGE),
                },
                ..Default::default()
            }),
    )
    .height(Length::Fill)
    .into()
}

fn view_block_dialog<'a>(form: &'a EditTaskForm, palette: &'a Palette) -> Element<'a, Message> {
    let bg = palette.surface;
    let border_color = palette.border;
    let status_blocked = palette.status_blocked;

    let dialog = container(
        column![
            text("Block Task").size(18).color(palette.text),
            Space::with_height(16),
            text("Enter a reason for blocking this task:")
                .size(14)
                .color(palette.text_secondary),
            Space::with_height(8),
            view_text_input(
                "Blocking reason...",
                &form.block_reason,
                Message::BlockTaskReason,
                palette,
            ),
            Space::with_height(16),
            row![
                horizontal_space(),
                widget::action_button("Cancel", Message::BlockTaskCancelled, palette),
                Space::with_width(8),
                view_danger_button(
                    "Block Task",
                    Message::BlockTaskSubmit,
                    status_blocked,
                    palette
                ),
            ]
            .align_y(iced::Alignment::Center),
        ]
        .spacing(0)
        .padding(24)
        .max_width(400),
    )
    .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: Radius::from(appearance::CORNER_RADIUS_LARGE),
        },
        ..Default::default()
    });

    // Backdrop
    container(
        container(dialog)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(iced::Color::from_rgba(
            0.0, 0.0, 0.0, 0.5,
        ))),
        ..Default::default()
    })
    .into()
}

fn view_field<'a>(
    label: &'a str,
    required: bool,
    input: Element<'a, Message>,
    error: Option<&'a String>,
    _palette: &'a Palette,
) -> Element<'a, Message> {
    let mut col = column![
        form::field_label(label, required),
        Space::with_height(4),
        input,
    ]
    .spacing(0);

    if let Some(err) = error {
        col = col.push(Space::with_height(4));
        col = col.push(form::error_message(err));
    }

    col.into()
}

fn view_text_input<'a, F>(
    placeholder: &'a str,
    value: &'a str,
    on_input: F,
    palette: &'a Palette,
) -> Element<'a, Message>
where
    F: Fn(String) -> Message + 'a,
{
    let accent = palette.accent;
    let border_hover = palette.border_hover;
    let border = palette.border;
    let bg_input = palette.input;
    let text_muted = palette.text_muted;
    let text_primary = palette.text;

    text_input(placeholder, value)
        .on_input(on_input)
        .padding(12)
        .size(14)
        .width(Length::Fill)
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
        })
        .into()
}

fn view_text_input_fixed<'a, F>(
    placeholder: &'a str,
    value: &'a str,
    on_input: F,
    width: f32,
    palette: &'a Palette,
) -> Element<'a, Message>
where
    F: Fn(String) -> Message + 'a,
{
    let accent = palette.accent;
    let border_hover = palette.border_hover;
    let border = palette.border;
    let bg_input = palette.input;
    let text_muted = palette.text_muted;
    let text_primary = palette.text;

    text_input(placeholder, value)
        .on_input(on_input)
        .padding(12)
        .size(14)
        .width(Length::Fixed(width))
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
        })
        .into()
}

fn view_description_editor<'a>(
    content: &'a text_editor::Content,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let accent = palette.accent;
    let border_hover = palette.border_hover;
    let border = palette.border;
    let bg_input = palette.input;
    let text_muted = palette.text_muted;
    let text_primary = palette.text;

    text_editor(content)
        .placeholder("Enter description...")
        .on_action(Message::EditTaskFormDescriptionAction)
        .padding(12)
        .height(Length::Fixed(120.0))
        .style(move |_: &Theme, status| {
            let border_color = match status {
                text_editor::Status::Focused => accent,
                text_editor::Status::Hovered => border_hover,
                _ => border,
            };
            text_editor::Style {
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
        })
        .into()
}

fn view_pick_list<'a, T, F>(
    options: Vec<T>,
    selected: Option<T>,
    on_selected: F,
    palette: &'a Palette,
) -> Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    F: Fn(T) -> Message + 'a,
{
    let bg = palette.input;
    let border_color = palette.border;
    let text_color = palette.text;
    let text_muted = palette.text_muted;

    pick_list(options, selected, on_selected)
        .width(Length::Fixed(120.0))
        .padding(12)
        .style(move |_: &Theme, _| pick_list::Style {
            background: Background::Color(bg),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS),
            },
            text_color,
            placeholder_color: text_muted,
            handle_color: text_muted,
        })
        .into()
}

fn view_pick_list_wide<'a, T, F>(
    options: Vec<T>,
    selected: Option<T>,
    on_selected: F,
    palette: &'a Palette,
) -> Element<'a, Message>
where
    T: ToString + PartialEq + Clone + 'a,
    F: Fn(T) -> Message + 'a,
{
    let bg = palette.input;
    let border_color = palette.border;
    let text_color = palette.text;
    let text_muted = palette.text_muted;

    pick_list(options, selected, on_selected)
        .width(Length::Fixed(140.0))
        .padding(12)
        .style(move |_: &Theme, _| pick_list::Style {
            background: Background::Color(bg),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS),
            },
            text_color,
            placeholder_color: text_muted,
            handle_color: text_muted,
        })
        .into()
}

fn view_submit_button<'a>(
    label: &'a str,
    on_press: Message,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let accent = palette.accent;
    let text_on_accent = palette.background;

    button(container(text(label).size(14).color(text_on_accent)).padding(Padding::from([10, 20])))
        .on_press(on_press)
        .style(move |_, status| {
            let bg = match status {
                button::Status::Hovered => appearance::lighten(accent, 0.1),
                button::Status::Pressed => appearance::darken(accent, 0.1),
                _ => accent,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: Radius::from(appearance::CORNER_RADIUS),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

fn view_submit_button_disabled<'a>(label: &'a str, palette: &'a Palette) -> Element<'a, Message> {
    let bg = palette.card;
    let text_color = palette.text_muted;

    button(container(text(label).size(14).color(text_color)).padding(Padding::from([10, 20])))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: Radius::from(appearance::CORNER_RADIUS),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn view_block_button<'a>(task_id: &'a str, palette: &'a Palette) -> Element<'a, Message> {
    let status_blocked = palette.status_blocked;
    let card_bg = palette.card;

    button(container(text("Block").size(14).color(status_blocked)).padding(Padding::from([10, 16])))
        .on_press(Message::BlockTask(task_id.to_string()))
        .style(move |_, status| {
            let border_color = match status {
                button::Status::Hovered => appearance::lighten(status_blocked, 0.1),
                _ => status_blocked,
            };
            button::Style {
                background: Some(Background::Color(card_bg)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS),
                },
                ..Default::default()
            }
        })
        .into()
}

fn view_danger_button<'a>(
    label: &'a str,
    on_press: Message,
    danger_color: iced::Color,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let text_on_danger = palette.background;

    button(container(text(label).size(14).color(text_on_danger)).padding(Padding::from([10, 16])))
        .on_press(on_press)
        .style(move |_, status| {
            let bg = match status {
                button::Status::Hovered => appearance::lighten(danger_color, 0.1),
                button::Status::Pressed => appearance::darken(danger_color, 0.1),
                _ => danger_color,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    radius: Radius::from(appearance::CORNER_RADIUS),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

/// Format a timestamp for display (truncate to date/time without milliseconds)
fn format_timestamp(timestamp: &str) -> String {
    // Try to parse and format nicely, fallback to original
    if timestamp.len() > 19 {
        timestamp[..19].replace('T', " ")
    } else {
        timestamp.replace('T', " ")
    }
}
