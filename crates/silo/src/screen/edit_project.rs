//! Edit project screen - form for editing existing projects.
//!
//! This screen provides a form with all fields needed to edit a project
//! including name (required), description, owner, and tags.

use crate::appearance::{self, Palette};
use crate::message::Message;
use iced::border::Radius;
use iced::widget::{
    Space, button, column, container, horizontal_space, row, scrollable, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Padding, Theme};

type Renderer = iced::Renderer;

/// Form state for editing a project.
#[derive(Debug, Clone, Default)]
pub struct EditProjectForm {
    pub project_id: String,
    pub name: String,
    pub description: String,
    pub owner: String,
    pub tags: String,
    pub submitting: bool,
}

/// State passed to the edit project view function.
pub struct EditProjectState<'a> {
    pub form: &'a EditProjectForm,
    pub loading: bool,
    pub error_message: Option<&'a String>,
}

/// Renders the edit project form screen.
pub fn view<'a>(state: EditProjectState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let header = view_header(palette);
    let form = view_form(&state, palette);
    let error_section = view_error(&state, palette);
    let footer = view_footer(&state, palette);

    let content = column![header, form, error_section, footer]
        .spacing(24)
        .padding(32)
        .width(Length::Fill)
        .height(Length::Fill);

    content.into()
}

/// Renders the header with title and cancel button.
fn view_header<'a>(palette: &'a Palette) -> Element<'a, Message> {
    let title = text("Edit Project")
        .size(28)
        .color(palette.text)
        .font(iced::Font::MONOSPACE);

    let text_color = palette.text_secondary;
    let hover_bg = palette.card_hover;

    let cancel_btn = button(
        container(text("Cancel").size(14).color(text_color)).padding(Padding::from([8, 16])),
    )
    .on_press(Message::CancelEditProject)
    .style(move |_, status| {
        let bg = match status {
            button::Status::Hovered => hover_bg,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: Radius::from(appearance::CORNER_RADIUS_SMALL),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    row![title, horizontal_space(), cancel_btn]
        .align_y(iced::Alignment::Center)
        .into()
}

/// Renders the form fields.
fn view_form<'a>(state: &EditProjectState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let enabled = !state.loading && !state.form.submitting;

    let name_field = view_text_field(
        "Name *",
        "Project name",
        &state.form.name,
        Message::EditProjectNameChanged,
        enabled,
        palette,
    );

    let description_field = view_text_field(
        "Description",
        "Optional description",
        &state.form.description,
        Message::EditProjectDescriptionChanged,
        enabled,
        palette,
    );

    let owner_field = view_text_field(
        "Owner",
        "Optional owner",
        &state.form.owner,
        Message::EditProjectOwnerChanged,
        enabled,
        palette,
    );

    let tags_field = view_text_field(
        "Tags",
        "tag1, tag2, tag3",
        &state.form.tags,
        Message::EditProjectTagsChanged,
        enabled,
        palette,
    );

    let form_content = column![
        name_field,
        Space::with_height(16),
        description_field,
        Space::with_height(16),
        owner_field,
        Space::with_height(16),
        tags_field,
    ]
    .width(Length::Fill);

    let bg = palette.surface;
    let border_color = palette.border;

    scrollable(
        container(form_content)
            .padding(24)
            .width(Length::Fill)
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

/// Renders a labeled text input field.
fn view_text_field<'a, F>(
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
    let label_text = text(label).size(12).color(palette.text_secondary);

    let accent = palette.accent;
    let border_hover = palette.border_hover;
    let border = palette.border;
    let bg_input = palette.input;
    let text_muted = palette.text_muted;
    let text_primary = palette.text;

    let mut input: text_input::TextInput<'a, Message, Theme, Renderer> =
        text_input(placeholder, value)
            .padding(12)
            .size(14)
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

    if enabled {
        input = input.on_input(on_input);
    }

    column![label_text, Space::with_height(6), input]
        .width(Length::Fill)
        .into()
}

/// Renders the error message area.
fn view_error<'a>(state: &EditProjectState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    if let Some(err) = state.error_message {
        let bg = palette.danger_light;
        let text_color = palette.danger;

        container(text(err.as_str()).size(13).color(text_color))
            .padding(Padding::from([12, 16]))
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgba(bg.r, bg.g, bg.b, 0.15))),
                border: Border {
                    color: Color::from_rgba(text_color.r, text_color.g, text_color.b, 0.3),
                    width: 1.0,
                    radius: Radius::from(appearance::CORNER_RADIUS),
                },
                ..Default::default()
            })
            .into()
    } else {
        Space::new(0, 0).into()
    }
}

/// Renders the footer with cancel and save buttons.
fn view_footer<'a>(state: &EditProjectState<'a>, palette: &'a Palette) -> Element<'a, Message> {
    let can_submit = !state.form.name.is_empty() && !state.loading && !state.form.submitting;

    // Cancel button (secondary style)
    let text_secondary = palette.text_secondary;
    let hover_bg = palette.card_hover;
    let border_color = palette.border;

    let cancel_btn = button(
        container(text("Cancel").size(14).color(text_secondary)).padding(Padding::from([12, 24])),
    )
    .on_press(Message::CancelEditProject)
    .style(move |_, status| {
        let bg = match status {
            button::Status::Hovered => hover_bg,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS),
            },
            ..Default::default()
        }
    });

    // Save button (primary style)
    let accent = palette.accent;
    let accent_hover = appearance::lighten(accent, 0.1);
    let accent_disabled = appearance::with_alpha(accent, 0.5);
    let text_on_accent = palette.background;
    let text_disabled = palette.text_muted;

    let btn_label = if state.form.submitting {
        "Saving..."
    } else {
        "Save Changes"
    };

    let save_btn = button(
        container(text(btn_label).size(14).color(if can_submit {
            text_on_accent
        } else {
            text_disabled
        }))
        .padding(Padding::from([12, 24])),
    )
    .on_press_maybe(can_submit.then_some(Message::SubmitEditProject))
    .style(move |_, status| {
        let bg = if !can_submit {
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
                radius: Radius::from(appearance::CORNER_RADIUS),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    row![
        horizontal_space(),
        cancel_btn,
        Space::with_width(12),
        save_btn
    ]
    .align_y(iced::Alignment::Center)
    .into()
}
