//! Create task screen - form for new task creation
//!
//! Full-featured task creation form with validation.

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

/// Form state for task creation
#[derive(Debug, Clone, Default)]
pub struct CreateTaskForm {
    pub title: String,
    pub description: String,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub owner: String,
    pub due_date: String,
    pub tags: String,
    pub dependency_input: String,
    pub dependencies: Vec<String>,
    pub validation_errors: std::collections::HashMap<String, String>,
    pub submitting: bool,
}

impl CreateTaskForm {
    pub fn new() -> Self {
        Self {
            status: TaskStatus::Todo,
            priority: TaskPriority::P2,
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

/// State passed to create task view
pub struct CreateTaskScreenState<'a> {
    pub project_name: &'a str,
    pub form: &'a CreateTaskForm,
    pub available_tasks: &'a [GranaryTask], // For dependency selection
    pub description_content: &'a text_editor::Content,
}

/// Main view function
pub fn view<'a>(state: CreateTaskScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(state.project_name, palette);
    let form = view_form(state, palette);

    column![header, form]
        .spacing(24)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn view_header<'a>(project_name: &'a str, palette: &'a Palette) -> Element<'a, Message> {
    let back_btn = widget::icon_button(Icon::ArrowLeft, Message::CreateTaskFormCancel, palette);

    let title = text(format!("Create Task - {}", project_name))
        .size(20)
        .color(palette.text);

    row![back_btn, Space::with_width(12), title,]
        .align_y(iced::Alignment::Center)
        .into()
}

fn view_form<'a>(state: CreateTaskScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let form = state.form;

    // Title field (required)
    let title_field = view_field(
        "Title",
        true,
        view_text_input(
            "Enter task title...",
            &form.title,
            Message::CreateTaskFormTitle,
            palette,
        ),
        form.validation_errors.get("title"),
        palette,
    );

    // Description field (multiline text editor)
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
            Message::CreateTaskFormPriority,
            palette,
        ),
        None,
        palette,
    );

    // Status picker
    let statuses = vec![TaskStatus::Draft, TaskStatus::Todo];
    let status_field = view_field(
        "Status",
        false,
        view_pick_list(
            statuses,
            Some(form.status.clone()),
            Message::CreateTaskFormStatus,
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
            Message::CreateTaskFormOwner,
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
            Message::CreateTaskFormDueDate,
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
            Message::CreateTaskFormTags,
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
        Message::CreateTaskFormDependency,
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
                t.title.to_lowercase().contains(&query) && !form.dependencies.contains(&t.id)
            })
            .take(5) // Limit to 5 results
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
                    .on_press(Message::CreateTaskFormSelectDependency(task_id))
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
        text("No dependencies added")
            .size(12)
            .color(palette.text_muted)
            .into()
    } else {
        // Show added dependencies with task titles if available
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
                            Message::CreateTaskFormRemoveDependency(dep_id.clone()),
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

    // Form actions
    let cancel_btn = widget::action_button("Cancel", Message::CreateTaskFormCancel, palette);

    let submit_btn = if form.submitting {
        view_submit_button_disabled("Creating...", palette)
    } else {
        view_submit_button("Create Task", Message::CreateTaskFormSubmit, palette)
    };

    let actions = row![
        horizontal_space(),
        cancel_btn,
        Space::with_width(12),
        submit_btn
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
        .on_action(Message::CreateTaskFormDescriptionAction)
        .placeholder("Enter description...")
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
