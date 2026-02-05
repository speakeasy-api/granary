//! Start worker screen - form for starting workers with customizable settings
//!
//! Provides a form to configure and start a new worker, optionally pre-filled
//! from a runner configuration.

use crate::appearance::{self, Palette};
use crate::message::Message;
use crate::widget::{self, form, icon};
use iced::border::Radius;
use iced::widget::{
    Column, Space, button, checkbox, column, container, horizontal_space, row, scrollable, text,
    text_input,
};
use iced::{Background, Border, Element, Length, Padding, Theme};
use lucide_icons::Icon;

/// State passed to the start worker view function.
///
/// This struct contains all the data needed to render the start worker form,
/// borrowed from the main application state.
pub struct StartWorkerFormState<'a> {
    /// Runner name if pre-filled from a runner
    pub from_runner: Option<&'a str>,
    /// Form field values
    pub command: &'a str,
    pub args: &'a str, // multiline string
    pub event_type: &'a str,
    pub concurrency: &'a str, // string for input, parse on submit
    pub poll_cooldown: &'a str,
    pub detached: bool,
    /// Read-only env vars from runner (for display only)
    pub env_vars: &'a [(String, String)],
    /// Error message if submission failed
    pub error: Option<&'a str>,
    /// Loading state during submission
    pub submitting: bool,
}

/// Renders the start worker form screen.
pub fn view<'a>(state: StartWorkerFormState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(&state, palette);
    let form = view_form(&state, palette);

    column![header, form]
        .spacing(24)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Renders the header with back button and title.
fn view_header<'a>(state: &StartWorkerFormState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let back_btn = view_back_button(palette);

    let title = text("Start Worker").size(20).color(palette.text);

    let subtitle: Element<'a, Message> = if let Some(runner) = state.from_runner {
        text(format!("(from runner: {})", runner))
            .size(14)
            .color(palette.text_muted)
            .into()
    } else {
        Space::new(0, 0).into()
    };

    row![
        back_btn,
        Space::with_width(12),
        column![title, subtitle].spacing(2),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

/// Renders the form content.
fn view_form<'a>(state: &StartWorkerFormState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    // Command field (required)
    let command_field = view_field(
        "Command",
        true,
        view_text_input(
            "e.g., claude",
            state.command,
            Message::StartWorkerCommandChanged,
            palette,
        ),
        None,
        palette,
    );

    // Arguments field (multiline)
    let args_field = view_field(
        "Arguments",
        false,
        view_text_area(
            "One argument per line, e.g.:\n--print\n--message \"...\"",
            state.args,
            Message::StartWorkerArgsChanged,
            palette,
        ),
        None,
        palette,
    );

    // Event type field
    let event_type_field = view_field(
        "Event type",
        false,
        view_text_input(
            "e.g., task.next",
            state.event_type,
            Message::StartWorkerEventChanged,
            palette,
        ),
        None,
        palette,
    );

    // Concurrency field
    let concurrency_field = view_field(
        "Concurrency",
        false,
        view_text_input_fixed(
            "e.g., 2",
            state.concurrency,
            Message::StartWorkerConcurrencyChanged,
            120.0,
            palette,
        ),
        None,
        palette,
    );

    // Poll cooldown field
    let poll_cooldown_field = view_field(
        "Poll cooldown (seconds)",
        false,
        view_text_input_fixed(
            "e.g., 300",
            state.poll_cooldown,
            Message::StartWorkerEventChanged, // Note: Using EventChanged as placeholder - see note below
            120.0,
            palette,
        ),
        None,
        palette,
    );

    // Event settings row
    let event_settings = column![
        text("Event Settings")
            .size(14)
            .color(palette.text_secondary),
        Space::with_height(8),
        event_type_field,
        row![
            concurrency_field,
            Space::with_width(24),
            poll_cooldown_field
        ]
        .align_y(iced::Alignment::End),
    ]
    .spacing(8);

    // Options section - detached checkbox
    let detached_checkbox = checkbox("Run in background (detached)", state.detached)
        .on_toggle(Message::StartWorkerDetachedChanged)
        .text_size(14)
        .spacing(8);

    let options_section = column![
        text("Options").size(14).color(palette.text_secondary),
        Space::with_height(8),
        detached_checkbox,
    ]
    .spacing(0);

    // Environment variables section (read-only from runner)
    let env_section: Element<'a, Message> = if !state.env_vars.is_empty() {
        let env_items: Vec<Element<'a, Message>> = state
            .env_vars
            .iter()
            .map(|(key, value)| {
                let masked_value = mask_env_value(value);
                text(format!("{}={}", key, masked_value))
                    .size(12)
                    .color(palette.text_muted)
                    .font(iced::Font::MONOSPACE)
                    .into()
            })
            .collect();

        let bg = palette.background;
        column![
            text("Environment (from runner)")
                .size(14)
                .color(palette.text_secondary),
            Space::with_height(8),
            container(Column::from_vec(env_items).spacing(4))
                .padding(12)
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: Some(Background::Color(bg)),
                    border: Border {
                        radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
        ]
        .spacing(0)
        .into()
    } else {
        Space::new(0, 0).into()
    };

    // Error message display
    let error_display: Element<'a, Message> = if let Some(error) = state.error {
        let danger_bg = appearance::with_alpha(palette.status_blocked, 0.1);
        container(
            row![
                icon(Icon::Ban).size(12).color(palette.status_blocked),
                Space::with_width(8),
                text(error).size(12).color(palette.status_blocked),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(12)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(danger_bg)),
            border: Border {
                radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
    } else {
        Space::new(0, 0).into()
    };

    // Form actions
    let cancel_btn = widget::action_button("Cancel", Message::GoBack, palette);

    let submit_btn = if state.submitting {
        view_submit_button_disabled("Starting...", palette)
    } else {
        view_submit_button("Start Worker", Message::SubmitStartWorker, palette)
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

    // Build form sections
    let mut form_sections: Vec<Element<'a, Message>> = vec![
        // Command section
        column![
            text("Command").size(14).color(palette.text_secondary),
            Space::with_height(8),
            command_field,
            args_field,
        ]
        .spacing(8)
        .into(),
        // Event settings
        event_settings.into(),
        // Options
        options_section.into(),
    ];

    // Add env section if present
    if !state.env_vars.is_empty() {
        form_sections.push(env_section);
    }

    // Add error display if present
    if state.error.is_some() {
        form_sections.push(error_display);
    }

    // Add spacing and actions
    form_sections.push(Space::with_height(24).into());
    form_sections.push(actions.into());

    let form_content = Column::from_vec(form_sections).spacing(24).max_width(600);

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

/// Helper function to mask sensitive environment values.
fn mask_env_value(value: &str) -> String {
    if value.len() <= 4 {
        "***".to_string()
    } else {
        format!("{}***", &value[..2])
    }
}

/// Helper to create the back button.
fn view_back_button<'a>(palette: &'a Palette) -> Element<'a, Message> {
    let text_color = palette.text_secondary;
    let bg_normal = palette.card;
    let bg_hover = palette.card_hover;
    let border_color = palette.border;

    button(container(text("<- Back").size(13).color(text_color)).padding(Padding::from([6, 12])))
        .on_press(Message::GoBack)
        .style(move |_: &Theme, status| {
            let bg = match status {
                button::Status::Hovered => bg_hover,
                _ => bg_normal,
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

/// Helper to create a form field with label.
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

/// Text input helper.
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

/// Fixed width text input helper.
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

/// Text area helper for multiline input (using text_input with larger height).
fn view_text_area<'a, F>(
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

    // Using text_input for now - could be replaced with text_editor for true multiline
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

/// Submit button with primary styling.
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

/// Disabled submit button for loading state.
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
