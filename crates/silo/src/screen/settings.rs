//! Settings screen - manage runners, steering files, and workspace config
//!
//! This screen provides three collapsible sections for managing:
//! - Runners: Command configurations for task execution
//! - Steering Files: Context/guidance files for runners
//! - Workspace Config: Key-value configuration pairs

use crate::appearance::{self, FONT_SIZE_SMALL, PADDING, PADDING_LARGE, Palette, SPACING};
use crate::message::{Message, SteeringFile};
use iced::border::Radius;
use iced::widget::{
    Space, button, column, container, horizontal_space, row, scrollable, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Padding as IcedPadding, Theme};

use granary_types::{ActionConfig, RunnerConfig};

/// Form state for adding/editing a runner
#[derive(Debug, Clone, Default)]
pub struct RunnerFormState {
    /// Runner name (unique identifier)
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Arguments (supports ${VAR} expansion, shown as hint)
    pub args: String,
    /// Concurrency limit (stored as string for input, parsed on save)
    pub concurrency: String,
    /// Event type the runner responds to (e.g., task.next, task.start)
    pub on_event: String,
    /// Some(name) if editing existing runner, None if creating new
    pub editing: Option<String>,
}

/// Form state for adding/editing an action
#[derive(Debug, Clone, Default)]
pub struct ActionFormState {
    /// Action name (unique identifier)
    pub name: String,
    /// Command to execute
    pub command: String,
    /// Arguments (supports ${VAR} expansion, shown as hint)
    pub args: String,
    /// Concurrency limit (stored as string for input, parsed on save)
    pub concurrency: String,
    /// Event type the action responds to (e.g., task.next, task.start)
    pub on_event: String,
    /// Some(name) if editing existing action, None if creating new
    pub editing: Option<String>,
}

/// Form state for adding a steering file
#[derive(Debug, Clone, Default)]
pub struct SteeringFormState {
    /// Path to the steering file
    pub path: String,
    /// Mode: "reference" or "inline"
    pub mode: String,
    /// Optional project scope
    pub project: String,
}

/// Form state for adding a config entry
#[derive(Debug, Clone, Default)]
pub struct ConfigFormState {
    /// Config key
    pub key: String,
    /// Config value
    pub value: String,
}

/// State needed to render the settings screen
pub struct SettingsScreenState<'a> {
    /// List of runner configurations (name, config)
    pub runners: &'a [(String, RunnerConfig)],
    /// List of action configurations (name, config)
    pub actions: &'a [(String, ActionConfig)],
    /// List of steering files
    pub steering_files: &'a [SteeringFile],
    /// List of config key-value pairs
    pub config_entries: &'a [(String, String)],
    /// Whether data is loading
    pub loading: bool,
    /// Form state for new runner
    pub runner_form: &'a RunnerFormState,
    /// Form state for new action
    pub action_form: &'a ActionFormState,
    /// Form state for new steering file
    pub steering_form: &'a SteeringFormState,
    /// Form state for new config entry
    pub config_form: &'a ConfigFormState,
}

/// Renders the settings screen with three sections
pub fn view<'a>(state: SettingsScreenState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    // Header with back button and title
    let header = view_header(palette);

    // Runners section
    let runners_section = section_card(
        "Runners",
        runners_content(state.runners, state.runner_form, state.loading, palette),
        palette,
    );

    // Actions section
    let actions_section = section_card(
        "Actions",
        actions_content(state.actions, state.action_form, state.loading, palette),
        palette,
    );

    // Steering section
    let steering_section = section_card(
        "Steering Files",
        steering_content(
            state.steering_files,
            state.steering_form,
            state.loading,
            palette,
        ),
        palette,
    );

    // Config section
    let config_section = section_card(
        "Workspace Config",
        config_content(
            state.config_entries,
            state.config_form,
            state.loading,
            palette,
        ),
        palette,
    );

    let content = scrollable(
        column![
            header,
            Space::with_height(24),
            runners_section,
            Space::with_height(24),
            actions_section,
            Space::with_height(24),
            steering_section,
            Space::with_height(24),
            config_section,
        ]
        .padding(32)
        .width(Length::Fill),
    )
    .height(Length::Fill);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Renders the header with title
fn view_header<'a>(palette: &'a Palette) -> Element<'a, Message> {
    crate::widget::page_header_simple("Settings", palette)
}

/// Creates a section card with title header and content
fn section_card<'a>(
    title: &'a str,
    content: Element<'a, Message>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let bg = palette.surface;
    let border_color = palette.border;

    let header = container(text(title).size(16).color(palette.text))
        .padding(IcedPadding::from([12, PADDING_LARGE]))
        .width(Length::Fill)
        .style({
            let border_bottom = palette.border;
            move |_| container::Style {
                border: Border {
                    color: border_bottom,
                    width: 1.0,
                    radius: Radius::from(0.0),
                },
                ..Default::default()
            }
        });

    let body = container(content)
        .padding(PADDING_LARGE)
        .width(Length::Fill);

    let card_content = column![header, body].width(Length::Fill);

    container(card_content)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS_LARGE),
            },
            ..Default::default()
        })
        .into()
}

/// Renders the runners section content
fn runners_content<'a>(
    runners: &'a [(String, RunnerConfig)],
    form: &'a RunnerFormState,
    _loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let mut content_items: Vec<Element<'a, Message>> = Vec::new();

    // List existing runners
    if runners.is_empty() {
        content_items.push(
            text("No runners configured")
                .size(FONT_SIZE_SMALL)
                .color(palette.text_muted)
                .into(),
        );
    } else {
        for (name, config) in runners {
            content_items.push(runner_row(name, config, palette));
        }
    }

    // Runner editor form (add or edit)
    content_items.push(Space::with_height(16).into());
    let form_title = if form.editing.is_some() {
        "Edit Runner"
    } else {
        "Add Runner"
    };
    content_items.push(
        text(form_title)
            .size(FONT_SIZE_SMALL)
            .color(palette.text_secondary)
            .into(),
    );
    content_items.push(Space::with_height(8).into());
    content_items.push(runner_editor(form, palette));

    column(content_items).spacing(SPACING).into()
}

/// Renders a single runner row with edit/delete buttons
fn runner_row<'a>(
    name: &'a str,
    config: &'a RunnerConfig,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let name_text = text(name).size(14).color(palette.text);

    let command_text = text(format!("Command: {}", config.command))
        .size(FONT_SIZE_SMALL)
        .color(palette.text_secondary);

    let event_str = config.on.as_deref().unwrap_or("(none)");
    let concurrency_str = config
        .concurrency
        .map(|c| c.to_string())
        .unwrap_or_else(|| "1".to_string());
    let meta_text = text(format!(
        "Event: {} • Concurrency: {}",
        event_str, concurrency_str
    ))
    .size(FONT_SIZE_SMALL)
    .color(palette.text_muted);

    let edit_btn = small_action_button("Edit", Message::EditRunner(name.to_string()), palette);
    let delete_btn =
        small_action_button("Delete", Message::DeleteRunner(name.to_string()), palette);

    let info_col = column![name_text, command_text, meta_text].spacing(4);

    let row_bg = palette.card;
    let row_border = palette.border;

    container(
        row![info_col, horizontal_space(), edit_btn, delete_btn]
            .spacing(8)
            .align_y(iced::Alignment::Center),
    )
    .padding(IcedPadding::from([PADDING, PADDING_LARGE]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(row_bg)),
        border: Border {
            color: row_border,
            width: 1.0,
            radius: Radius::from(appearance::CORNER_RADIUS),
        },
        ..Default::default()
    })
    .into()
}

/// Renders the runner editor form (supports both create and edit modes)
///
/// When `form.editing` is `Some(name)`, the form is in edit mode for an existing runner.
/// When `form.editing` is `None`, the form is in create mode for a new runner.
fn runner_editor<'a>(form: &'a RunnerFormState, palette: &'a Palette) -> Element<'a, Message> {
    let name_input = labeled_field(
        "Name",
        text_input("Runner name", &form.name)
            .on_input(|v| Message::RunnerFormChanged {
                field: "name".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let command_input = labeled_field(
        "Command",
        text_input("Command (e.g., claude)", &form.command)
            .on_input(|v| Message::RunnerFormChanged {
                field: "command".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let args_input = labeled_field(
        "Args",
        text_input("Args (e.g., --print --message \"${PROMPT}\")", &form.args)
            .on_input(|v| Message::RunnerFormChanged {
                field: "args".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let concurrency_input = labeled_field(
        "Concurrency",
        text_input("Concurrency (default: 1)", &form.concurrency)
            .on_input(|v| Message::RunnerFormChanged {
                field: "concurrency".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let on_event_input = labeled_field(
        "On Event",
        text_input("Event (e.g., task.next)", &form.on_event)
            .on_input(|v| Message::RunnerFormChanged {
                field: "on".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let can_save = !form.name.is_empty() && !form.command.is_empty();
    let button_label = if form.editing.is_some() {
        "Update"
    } else {
        "Add"
    };
    let save_btn = primary_button(button_label, Message::SaveRunner, can_save, false, palette);

    // Add cancel button when in edit mode
    let button_row: Element<'a, Message> = if form.editing.is_some() {
        let cancel_btn = small_action_button("Cancel", Message::CancelEditRunner, palette);
        row![horizontal_space(), cancel_btn, save_btn]
            .spacing(8)
            .into()
    } else {
        row![horizontal_space(), save_btn].into()
    };

    column![
        row![name_input, command_input].spacing(SPACING),
        row![args_input, concurrency_input, on_event_input].spacing(SPACING),
        button_row,
    ]
    .spacing(SPACING)
    .into()
}

/// Renders the actions section content
fn actions_content<'a>(
    actions: &'a [(String, ActionConfig)],
    form: &'a ActionFormState,
    _loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let mut content_items: Vec<Element<'a, Message>> = Vec::new();

    if actions.is_empty() {
        content_items.push(
            text("No actions configured")
                .size(FONT_SIZE_SMALL)
                .color(palette.text_muted)
                .into(),
        );
    } else {
        for (name, config) in actions {
            content_items.push(action_row(name, config, palette));
        }
    }

    content_items.push(Space::with_height(16).into());
    let form_title = if form.editing.is_some() {
        "Edit Action"
    } else {
        "Add Action"
    };
    content_items.push(
        text(form_title)
            .size(FONT_SIZE_SMALL)
            .color(palette.text_secondary)
            .into(),
    );
    content_items.push(Space::with_height(8).into());
    content_items.push(action_editor(form, palette));

    column(content_items).spacing(SPACING).into()
}

/// Renders a single action row with edit/delete buttons
fn action_row<'a>(
    name: &'a str,
    config: &'a ActionConfig,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let name_text = text(name).size(14).color(palette.text);

    let command_text = text(format!("Command: {}", config.command))
        .size(FONT_SIZE_SMALL)
        .color(palette.text_secondary);

    let event_str = config.on.as_deref().unwrap_or("(none)");
    let concurrency_str = config
        .concurrency
        .map(|c| c.to_string())
        .unwrap_or_else(|| "1".to_string());
    let meta_text = text(format!(
        "Event: {} • Concurrency: {}",
        event_str, concurrency_str
    ))
    .size(FONT_SIZE_SMALL)
    .color(palette.text_muted);

    let edit_btn = small_action_button("Edit", Message::EditAction(name.to_string()), palette);
    let delete_btn =
        small_action_button("Delete", Message::DeleteAction(name.to_string()), palette);

    let info_col = column![name_text, command_text, meta_text].spacing(4);

    let row_bg = palette.card;
    let row_border = palette.border;

    container(
        row![info_col, horizontal_space(), edit_btn, delete_btn]
            .spacing(8)
            .align_y(iced::Alignment::Center),
    )
    .padding(IcedPadding::from([PADDING, PADDING_LARGE]))
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(row_bg)),
        border: Border {
            color: row_border,
            width: 1.0,
            radius: Radius::from(appearance::CORNER_RADIUS),
        },
        ..Default::default()
    })
    .into()
}

/// Renders the action editor form (supports both create and edit modes)
fn action_editor<'a>(form: &'a ActionFormState, palette: &'a Palette) -> Element<'a, Message> {
    let name_input = labeled_field(
        "Name",
        text_input("Action name", &form.name)
            .on_input(|v| Message::ActionFormChanged {
                field: "name".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let command_input = labeled_field(
        "Command",
        text_input("Command (e.g., claude)", &form.command)
            .on_input(|v| Message::ActionFormChanged {
                field: "command".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let args_input = labeled_field(
        "Args",
        text_input("Args (e.g., --print --message \"${PROMPT}\")", &form.args)
            .on_input(|v| Message::ActionFormChanged {
                field: "args".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let concurrency_input = labeled_field(
        "Concurrency",
        text_input("Concurrency (default: 1)", &form.concurrency)
            .on_input(|v| Message::ActionFormChanged {
                field: "concurrency".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let on_event_input = labeled_field(
        "On Event",
        text_input("Event (e.g., task.next)", &form.on_event)
            .on_input(|v| Message::ActionFormChanged {
                field: "on".to_string(),
                value: v,
            })
            .padding(10)
            .size(13)
            .style({
                let accent = palette.accent;
                let border_hover = palette.border_hover;
                let border = palette.border;
                let bg_input = palette.input;
                let text_muted = palette.text_muted;
                let text_primary = palette.text;
                move |_: &Theme, status| {
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
                            radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        },
                        icon: text_muted,
                        placeholder: text_muted,
                        value: text_primary,
                        selection: accent,
                    }
                }
            }),
        palette,
    );

    let can_save = !form.name.is_empty() && !form.command.is_empty();
    let button_label = if form.editing.is_some() {
        "Update"
    } else {
        "Add"
    };
    let save_btn = primary_button(button_label, Message::SaveAction, can_save, false, palette);

    let button_row: Element<'a, Message> = if form.editing.is_some() {
        let cancel_btn = small_action_button("Cancel", Message::CancelEditAction, palette);
        row![horizontal_space(), cancel_btn, save_btn]
            .spacing(8)
            .into()
    } else {
        row![horizontal_space(), save_btn].into()
    };

    column![
        row![name_input, command_input].spacing(SPACING),
        row![args_input, concurrency_input, on_event_input].spacing(SPACING),
        button_row,
    ]
    .spacing(SPACING)
    .into()
}

/// Helper function to create a labeled form field
fn labeled_field<'a>(
    label: &'a str,
    input: impl Into<Element<'a, Message>>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    column![
        text(label)
            .size(FONT_SIZE_SMALL)
            .color(palette.text_secondary),
        Space::with_height(4),
        input.into(),
    ]
    .width(Length::Fill)
    .into()
}

/// Renders the steering files section content
fn steering_content<'a>(
    steering_files: &'a [SteeringFile],
    form: &'a SteeringFormState,
    loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let mut content_items: Vec<Element<'a, Message>> = Vec::new();

    // Group by scope
    let global_files: Vec<_> = steering_files
        .iter()
        .filter(|f| f.scope_type.is_none())
        .collect();
    let project_files: Vec<_> = steering_files
        .iter()
        .filter(|f| f.scope_type.as_deref() == Some("project"))
        .collect();

    // Global section
    if !global_files.is_empty() {
        content_items.push(
            text("Global")
                .size(FONT_SIZE_SMALL)
                .color(palette.text_secondary)
                .into(),
        );
        for file in &global_files {
            content_items.push(steering_row(file, palette));
        }
    }

    // Project-scoped section
    if !project_files.is_empty() {
        if !global_files.is_empty() {
            content_items.push(Space::with_height(12).into());
        }
        content_items.push(
            text("Project-scoped")
                .size(FONT_SIZE_SMALL)
                .color(palette.text_secondary)
                .into(),
        );
        for file in &project_files {
            content_items.push(steering_row(file, palette));
        }
    }

    if steering_files.is_empty() {
        content_items.push(
            text("No steering files configured")
                .size(FONT_SIZE_SMALL)
                .color(palette.text_muted)
                .into(),
        );
    }

    // Add steering form
    content_items.push(Space::with_height(16).into());
    content_items.push(
        text("Add Steering File")
            .size(FONT_SIZE_SMALL)
            .color(palette.text_secondary)
            .into(),
    );
    content_items.push(Space::with_height(8).into());
    content_items.push(steering_form(form, loading, palette));

    column(content_items).spacing(SPACING).into()
}

/// Renders a single steering file row
fn steering_row<'a>(file: &'a SteeringFile, palette: &'a Palette) -> Element<'a, Message> {
    let path_text = text(&file.path).size(14).color(palette.text);

    let scope_str = match (&file.scope_type, &file.scope_id) {
        (Some(scope), Some(id)) => format!("{}: {}", scope, id),
        _ => "global".to_string(),
    };
    let meta_text = text(format!("Mode: {} • Scope: {}", file.mode, scope_str))
        .size(FONT_SIZE_SMALL)
        .color(palette.text_muted);

    let delete_btn = small_action_button(
        "Remove",
        Message::RemoveSteering(file.path.clone()),
        palette,
    );

    let info_col = column![path_text, meta_text].spacing(4);

    let row_bg = palette.card;
    let row_border = palette.border;

    container(row![info_col, horizontal_space(), delete_btn].align_y(iced::Alignment::Center))
        .padding(IcedPadding::from([PADDING, PADDING_LARGE]))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(row_bg)),
            border: Border {
                color: row_border,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS),
            },
            ..Default::default()
        })
        .into()
}

/// Renders the steering add form
fn steering_form<'a>(
    form: &'a SteeringFormState,
    loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let path_input = text_field(
        "Path",
        "/path/to/steering.md",
        &form.path,
        |v| Message::RunnerFormChanged {
            field: "steering_path".to_string(),
            value: v,
        },
        !loading,
        palette,
    );

    let mode_input = text_field(
        "Mode",
        "reference",
        &form.mode,
        |v| Message::RunnerFormChanged {
            field: "steering_mode".to_string(),
            value: v,
        },
        !loading,
        palette,
    );

    let project_input = text_field(
        "Project (optional)",
        "project-id",
        &form.project,
        |v| Message::RunnerFormChanged {
            field: "steering_project".to_string(),
            value: v,
        },
        !loading,
        palette,
    );

    let can_save = !form.path.is_empty() && !loading;
    let mode = if form.mode.is_empty() {
        "reference".to_string()
    } else {
        form.mode.clone()
    };
    let project = if form.project.is_empty() {
        None
    } else {
        Some(form.project.clone())
    };
    let save_btn = primary_button(
        "Add Steering",
        Message::AddSteering {
            path: form.path.clone(),
            mode,
            project,
        },
        can_save,
        loading,
        palette,
    );

    column![
        row![path_input, mode_input, project_input].spacing(SPACING),
        row![horizontal_space(), save_btn],
    ]
    .spacing(SPACING)
    .into()
}

/// Renders the config section content
fn config_content<'a>(
    config_entries: &'a [(String, String)],
    form: &'a ConfigFormState,
    loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let mut content_items: Vec<Element<'a, Message>> = Vec::new();

    // List existing config entries
    if config_entries.is_empty() {
        content_items.push(
            text("No config entries")
                .size(FONT_SIZE_SMALL)
                .color(palette.text_muted)
                .into(),
        );
    } else {
        for (key, value) in config_entries {
            content_items.push(config_row(key, value, palette));
        }
    }

    // Add config form
    content_items.push(Space::with_height(16).into());
    content_items.push(
        text("Add Config Entry")
            .size(FONT_SIZE_SMALL)
            .color(palette.text_secondary)
            .into(),
    );
    content_items.push(Space::with_height(8).into());
    content_items.push(config_form(form, loading, palette));

    column(content_items).spacing(SPACING).into()
}

/// Renders a single config row
fn config_row<'a>(key: &'a str, value: &'a str, palette: &'a Palette) -> Element<'a, Message> {
    let key_text = text(key).size(14).color(palette.text);
    let value_text = text(value)
        .size(FONT_SIZE_SMALL)
        .color(palette.text_secondary);

    let delete_btn = small_action_button("Delete", Message::DeleteConfig(key.to_string()), palette);

    let info_col = column![key_text, value_text].spacing(4);

    let row_bg = palette.card;
    let row_border = palette.border;

    container(row![info_col, horizontal_space(), delete_btn].align_y(iced::Alignment::Center))
        .padding(IcedPadding::from([PADDING, PADDING_LARGE]))
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(row_bg)),
            border: Border {
                color: row_border,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS),
            },
            ..Default::default()
        })
        .into()
}

/// Renders the config add form
fn config_form<'a>(
    form: &'a ConfigFormState,
    loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let key_input = text_field(
        "Key",
        "config.key",
        &form.key,
        |v| Message::RunnerFormChanged {
            field: "config_key".to_string(),
            value: v,
        },
        !loading,
        palette,
    );

    let value_input = text_field(
        "Value",
        "config value",
        &form.value,
        |v| Message::RunnerFormChanged {
            field: "config_value".to_string(),
            value: v,
        },
        !loading,
        palette,
    );

    let can_save = !form.key.is_empty() && !form.value.is_empty() && !loading;
    let save_btn = primary_button(
        "Set Config",
        Message::SetConfig {
            key: form.key.clone(),
            value: form.value.clone(),
        },
        can_save,
        loading,
        palette,
    );

    column![
        row![key_input, value_input].spacing(SPACING),
        row![horizontal_space(), save_btn],
    ]
    .spacing(SPACING)
    .into()
}

// === Helper widgets ===

/// Creates a labeled text input field
fn text_field<'a, F>(
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    on_input: F,
    enabled: bool,
    palette: &'a Palette,
) -> Element<'a, Message>
where
    F: Fn(String) -> Message + 'a,
{
    let label_text = text(label)
        .size(FONT_SIZE_SMALL)
        .color(palette.text_secondary);

    let accent = palette.accent;
    let border_hover = palette.border_hover;
    let border = palette.border;
    let bg_input = palette.input;
    let text_muted = palette.text_muted;
    let text_primary = palette.text;

    let mut input =
        text_input(placeholder, value)
            .padding(10)
            .size(13)
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
                        radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                    },
                    icon: text_muted,
                    placeholder: text_muted,
                    value: text_primary,
                    selection: accent,
                }
            });

    if enabled {
        input = input.on_input(on_input);
    }

    column![label_text, Space::with_height(4), input]
        .width(Length::Fill)
        .into()
}

/// Creates a small action button (Edit, Delete, Remove)
fn small_action_button<'a>(
    label: &'a str,
    msg: Message,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let text_color = palette.text_secondary;
    let bg_hover = palette.card_hover;
    let border_color = palette.border;

    button(container(text(label).size(11).color(text_color)).padding(IcedPadding::from([4, 10])))
        .on_press(msg)
        .style(move |_: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => bg_hover,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    color: border_color,
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                },
                ..Default::default()
            }
        })
        .into()
}

/// Creates a primary action button (Add, Save)
fn primary_button<'a>(
    label: &'a str,
    msg: Message,
    enabled: bool,
    loading: bool,
    palette: &'a Palette,
) -> Element<'a, Message> {
    let accent = palette.accent;
    let accent_hover = appearance::lighten(accent, 0.1);
    let accent_disabled = appearance::with_alpha(accent, 0.5);
    let text_on_accent = palette.background;
    let text_disabled = palette.text_muted;

    let btn_label = if loading { "..." } else { label };
    let can_press = enabled && !loading;

    button(
        container(text(btn_label).size(13).color(if can_press {
            text_on_accent
        } else {
            text_disabled
        }))
        .padding(IcedPadding::from([8, 16])),
    )
    .on_press_maybe(can_press.then_some(msg))
    .style(move |_, status| {
        let bg = if !can_press {
            accent_disabled
        } else {
            match status {
                button::Status::Hovered => accent_hover,
                button::Status::Pressed => appearance::darken(accent, 0.1),
                _ => accent,
            }
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
}
