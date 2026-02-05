---
name: iced-development
description: ALWAYS use this skill when writing, modifying, or reviewing Rust code that uses the Iced GUI library. This skill applies to Application implementations, Message enums, view/update functions, and UI components.
globs:
  - "crates/silo/**/*.rs"
---

# Iced GUI Development Best Practices

## Core Architecture: Elm Architecture (MVU)

Iced follows the Elm Architecture pattern. Every application must implement:

```rust
struct App {
    // Model: All application state lives here
}

enum Message {
    // All possible events/actions
}

impl Application for App {
    fn update(&mut self, message: Message) -> Task<Message>;
    fn view(&self) -> Element<Message>;
    fn subscription(&self) -> Subscription<Message>;
}
```

**Key Principles:**
- State is immutable during `view()` - only mutate in `update()`
- All user interactions flow through the `Message` enum
- `update()` returns `Task<Message>` for async operations
- `view()` is a pure function that renders UI from state

---

## Project Structure

### Recommended Layout

```
src/
├── main.rs              # Thin entry point (3-10 lines)
├── lib.rs               # Public API, re-exports
├── app.rs               # Application struct + update/view
├── config.rs            # Configuration handling
├── message.rs           # Message enum (if >50 variants)
├── appearance/          # Theming and styling
│   ├── mod.rs           # Theme struct + palettes
│   ├── button.rs        # Button style variants
│   ├── container.rs     # Container style variants
│   ├── text_input.rs    # Input field styles
│   └── fonts.rs         # Typography + icon fonts
├── screen/              # Screen/page modules
│   ├── mod.rs           # Screen enum + routing
│   ├── home.rs          # Individual screens
│   └── settings.rs
├── widget/              # Custom/composite widgets
│   └── mod.rs           # Widget re-exports
└── subscription/        # Background event sources
    └── mod.rs
```

### Entry Point Pattern

```rust
// main.rs - Keep minimal
fn main() -> iced::Result {
    App::run(Settings::default())
}
```

```rust
// lib.rs - Application setup
pub fn run() -> iced::Result {
    iced::application(App::title, App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .settings(settings())
        .run()
}
```

---

## Message Enum Patterns

### Hierarchical Nesting (Recommended for >3 screens)

```rust
#[derive(Debug, Clone)]
pub enum Message {
    // Screen delegation - each screen has its own Message type
    Home(home::Message),
    Settings(settings::Message),
    Details(details::Message),

    // Navigation
    Navigate(Screen),
    GoBack,

    // Global events
    ConfigLoaded(Result<Config, Error>),
    ThemeChanged(Theme),

    // Window events
    WindowResized(Size),
    CloseRequested,
}
```

**In update(), delegate to screens:**
```rust
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        Message::Home(msg) => {
            self.home.update(msg).map(Message::Home)
        }
        Message::Navigate(screen) => {
            self.history.push(self.current.clone());
            self.current = screen;
            Task::none()
        }
        // ...
    }
}
```

### Flat Pattern (For smaller apps, <30 variants)

```rust
#[derive(Debug, Clone)]
pub enum Message {
    // Lifecycle
    Ready,
    Quit,

    // User input
    InputChanged(String),
    ButtonPressed,
    ItemSelected(usize),

    // Async results
    DataLoaded(Result<Vec<Item>, Error>),
    SaveCompleted(Result<(), Error>),
}
```

---

## State Management

### Application State Structure

```rust
pub struct App {
    // === Navigation ===
    current_screen: Screen,
    screen_history: Vec<Screen>,  // For back navigation

    // === Domain Data ===
    items: Vec<Item>,
    selected_item: Option<usize>,

    // === UI State ===
    modal: Option<Modal>,
    loading: bool,
    error_message: Option<String>,

    // === Configuration ===
    config: Config,
    theme: Theme,

    // === Screen State (if not using separate structs) ===
    search_input: String,
    filter: Filter,
}
```

### Screen-Local State Pattern

```rust
// screen/settings.rs
pub struct Settings {
    // Form inputs
    name: String,
    email: String,

    // UI state
    saving: bool,
    validation_errors: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    NameChanged(String),
    EmailChanged(String),
    Save,
    SaveCompleted(Result<(), Error>),
}

impl Settings {
    pub fn new() -> Self { ... }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NameChanged(name) => {
                self.name = name;
                Task::none()
            }
            Message::Save => {
                self.saving = true;
                Task::perform(
                    save_settings(self.name.clone(), self.email.clone()),
                    Message::SaveCompleted
                )
            }
            Message::SaveCompleted(result) => {
                self.saving = false;
                if let Err(e) = result {
                    // Handle error
                }
                Task::none()
            }
            // ...
        }
    }

    pub fn view(&self) -> Element<Message> {
        column![
            text_input("Name", &self.name)
                .on_input(Message::NameChanged),
            text_input("Email", &self.email)
                .on_input(Message::EmailChanged),
            button("Save")
                .on_press_maybe((!self.saving).then_some(Message::Save)),
        ]
        .into()
    }
}
```

---

## Styling Architecture

### Theme Constants

```rust
// appearance/mod.rs
pub const CORNER_RADIUS: f32 = 8.0;
pub const BORDER_WIDTH: f32 = 1.0;
pub const SPACING: u16 = 10;
pub const PADDING: u16 = 12;

pub const FONT_SIZE_SMALL: f32 = 12.0;
pub const FONT_SIZE_BODY: f32 = 14.0;
pub const FONT_SIZE_TITLE: f32 = 20.0;

pub const TOOLTIP_DELAY: u64 = 300;  // milliseconds
```

### Color Palette Pattern

```rust
use std::sync::LazyLock;

pub struct Palette {
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub border: Color,
}

pub static DARK: LazyLock<Palette> = LazyLock::new(|| Palette {
    background: Color::from_rgb8(0x1e, 0x1e, 0x2e),
    surface: Color::from_rgb8(0x31, 0x31, 0x44),
    text: Color::from_rgb8(0xcd, 0xd6, 0xf4),
    text_secondary: Color::from_rgb8(0xa6, 0xad, 0xc8),
    primary: Color::from_rgb8(0x89, 0xb4, 0xfa),
    secondary: Color::from_rgb8(0xf5, 0xc2, 0xe7),
    success: Color::from_rgb8(0xa6, 0xe3, 0xa1),
    warning: Color::from_rgb8(0xf9, 0xe2, 0xaf),
    danger: Color::from_rgb8(0xf3, 0x8b, 0xa8),
    border: Color::from_rgb8(0x45, 0x47, 0x5a),
});

pub static LIGHT: LazyLock<Palette> = LazyLock::new(|| Palette {
    background: Color::from_rgb8(0xef, 0xf1, 0xf5),
    surface: Color::WHITE,
    // ...
});
```

### Style Enum Pattern for Widgets

```rust
// appearance/button.rs
#[derive(Debug, Clone, Copy, Default)]
pub enum ButtonStyle {
    #[default]
    Primary,
    Secondary,
    Danger,
    Ghost,      // Transparent background
    Icon,       // Icon-only button
}

impl button::Catalog for Theme {
    type Class<'a> = ButtonStyle;

    fn default<'a>() -> Self::Class<'a> {
        ButtonStyle::default()
    }

    fn style(&self, class: &Self::Class<'_>, status: button::Status) -> button::Style {
        let palette = self.palette();

        let (bg, text) = match class {
            ButtonStyle::Primary => (palette.primary, palette.background),
            ButtonStyle::Secondary => (palette.surface, palette.text),
            ButtonStyle::Danger => (palette.danger, palette.background),
            ButtonStyle::Ghost => (Color::TRANSPARENT, palette.text),
            ButtonStyle::Icon => (Color::TRANSPARENT, palette.text_secondary),
        };

        let bg = match status {
            button::Status::Active => bg,
            button::Status::Hovered => lighten(bg, 0.1),
            button::Status::Pressed => darken(bg, 0.1),
            button::Status::Disabled => with_alpha(bg, 0.5),
        };

        button::Style {
            background: Some(Background::Color(bg)),
            text_color: text,
            border: Border {
                radius: CORNER_RADIUS.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
```

### Color Utility Functions

```rust
pub fn lighten(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r + amount).min(1.0),
        g: (color.g + amount).min(1.0),
        b: (color.b + amount).min(1.0),
        a: color.a,
    }
}

pub fn darken(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r - amount).max(0.0),
        g: (color.g - amount).max(0.0),
        b: (color.b - amount).max(0.0),
        a: color.a,
    }
}

pub fn with_alpha(color: Color, alpha: f32) -> Color {
    Color { a: alpha, ..color }
}
```

---

## Component Patterns

### Reusable Widget Builders

```rust
// widget/mod.rs

/// Button with tooltip
pub fn tooltip_button<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    tooltip_text: &'a str,
    on_press: Message,
) -> Element<'a, Message> {
    tooltip(
        button(content)
            .on_press(on_press)
            .style(ButtonStyle::Primary),
        text(tooltip_text).size(FONT_SIZE_SMALL),
        tooltip::Position::Bottom,
    )
    .gap(5)
    .into()
}

/// Icon button with optional tooltip
pub fn icon_button<'a, Message: Clone + 'a>(
    icon: char,
    on_press: Message,
    tooltip_text: Option<&'a str>,
) -> Element<'a, Message> {
    let btn = button(
        text(icon)
            .font(ICON_FONT)
            .size(16)
    )
    .on_press(on_press)
    .style(ButtonStyle::Icon)
    .padding(8);

    if let Some(tip) = tooltip_text {
        tooltip(btn, text(tip).size(FONT_SIZE_SMALL), tooltip::Position::Bottom)
            .into()
    } else {
        btn.into()
    }
}

/// Labeled input field
pub fn labeled_input<'a, Message: Clone + 'a>(
    label: &'a str,
    value: &'a str,
    placeholder: &'a str,
    on_change: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        text(label).size(FONT_SIZE_SMALL),
        text_input(placeholder, value)
            .on_input(on_change)
            .padding(10),
    ]
    .spacing(4)
    .into()
}
```

### Modal/Dialog Pattern

```rust
#[derive(Debug, Clone)]
pub enum Modal {
    Confirm {
        title: String,
        message: String,
        on_confirm: Box<Message>,
    },
    Error {
        message: String,
    },
    Custom(Box<dyn ModalContent>),
}

impl Modal {
    pub fn view(&self) -> Element<Message> {
        let content = match self {
            Modal::Confirm { title, message, on_confirm } => {
                column![
                    text(title).size(FONT_SIZE_TITLE),
                    text(message),
                    row![
                        button("Cancel")
                            .on_press(Message::CloseModal)
                            .style(ButtonStyle::Secondary),
                        button("Confirm")
                            .on_press(*on_confirm.clone())
                            .style(ButtonStyle::Primary),
                    ]
                    .spacing(SPACING)
                ]
            }
            Modal::Error { message } => {
                column![
                    text("Error").size(FONT_SIZE_TITLE),
                    text(message),
                    button("OK")
                        .on_press(Message::CloseModal)
                        .style(ButtonStyle::Primary),
                ]
            }
            // ...
        };

        container(content)
            .padding(20)
            .style(ContainerStyle::Modal)
            .into()
    }
}

// In App::view()
fn view(&self) -> Element<Message> {
    let content = self.current_screen.view();

    if let Some(modal) = &self.modal {
        stack![
            content,
            // Semi-transparent backdrop
            mouse_area(
                container(Space::new(Length::Fill, Length::Fill))
                    .style(ContainerStyle::Backdrop)
            )
            .on_press(Message::CloseModal),
            // Centered modal
            center(modal.view()),
        ]
        .into()
    } else {
        content
    }
}
```

---

## Async Operations

### Task Pattern

```rust
fn update(&mut self, message: Message) -> Task<Message> {
    match message {
        // Trigger async operation
        Message::LoadData => {
            self.loading = true;
            Task::perform(
                async { load_data_from_api().await },
                Message::DataLoaded
            )
        }

        // Handle success
        Message::DataLoaded(Ok(data)) => {
            self.loading = false;
            self.data = data;
            Task::none()
        }

        // Handle error
        Message::DataLoaded(Err(e)) => {
            self.loading = false;
            self.error = Some(e.to_string());
            Task::none()
        }

        // Chain multiple tasks
        Message::SaveAndClose => {
            Task::batch([
                Task::perform(save_data(self.data.clone()), Message::SaveCompleted),
                Task::done(Message::RequestClose),
            ])
        }

        // ...
    }
}
```

### Subscription Patterns

```rust
fn subscription(&self) -> Subscription<Message> {
    Subscription::batch([
        // Keyboard shortcuts
        keyboard::on_key_press(|key, modifiers| {
            match (key.as_ref(), modifiers) {
                (keyboard::Key::Character("s"), m) if m.command() => {
                    Some(Message::Save)
                }
                (keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                    Some(Message::Cancel)
                }
                _ => None,
            }
        }),

        // Periodic updates
        time::every(Duration::from_secs(30)).map(|_| Message::RefreshData),

        // Window events
        window::events().map(|(id, event)| Message::WindowEvent(id, event)),

        // File watcher (custom subscription)
        self.watch_config_file(),
    ])
}

// Custom subscription example
fn watch_config_file(&self) -> Subscription<Message> {
    struct ConfigWatcher;

    subscription::channel(
        std::any::TypeId::of::<ConfigWatcher>(),
        100,
        |mut output| async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel(10);

            // Set up file watcher
            let mut watcher = notify::recommended_watcher(move |res| {
                let _ = tx.blocking_send(res);
            }).unwrap();

            watcher.watch(&config_path(), notify::RecursiveMode::NonRecursive).unwrap();

            loop {
                if let Some(Ok(_)) = rx.recv().await {
                    let _ = output.send(Message::ConfigChanged).await;
                }
            }
        }
    )
}
```

---

## Navigation Pattern

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Home,
    Settings,
    Details { id: usize },
    Editor { id: Option<usize> },  // None = new item
}

pub struct App {
    current: Screen,
    history: Vec<Screen>,
    // Screen state
    home: home::State,
    settings: settings::State,
    details: Option<details::State>,
    editor: Option<editor::State>,
}

impl App {
    fn navigate(&mut self, screen: Screen) -> Task<Message> {
        // Don't navigate to same screen
        if self.current == screen {
            return Task::none();
        }

        self.history.push(self.current.clone());
        self.current = screen.clone();

        // Initialize screen state if needed
        match &screen {
            Screen::Details { id } => {
                self.details = Some(details::State::new(*id));
                Task::perform(
                    load_details(*id),
                    Message::DetailsLoaded
                )
            }
            Screen::Editor { id } => {
                self.editor = Some(editor::State::new(*id));
                Task::none()
            }
            _ => Task::none()
        }
    }

    fn go_back(&mut self) -> Task<Message> {
        if let Some(previous) = self.history.pop() {
            self.current = previous;
        }
        Task::none()
    }
}
```

---

## Common Widgets Reference

### Layout

```rust
// Vertical stack
column![widget1, widget2, widget3]
    .spacing(SPACING)
    .padding(PADDING)
    .align_x(Alignment::Center)

// Horizontal stack
row![widget1, widget2, widget3]
    .spacing(SPACING)
    .align_y(Alignment::Center)

// Scrollable content
scrollable(content)
    .height(Length::Fill)
    .width(Length::Fill)

// Centered content
center(content)

// Container with styling
container(content)
    .padding(PADDING)
    .style(ContainerStyle::Card)

// Flexible space
Space::with_width(Length::Fill)
Space::with_height(10)
```

### Input Widgets

```rust
// Text input
text_input("Placeholder", &self.value)
    .on_input(Message::ValueChanged)
    .on_submit(Message::Submit)
    .padding(10)
    .width(Length::Fill)

// Button
button(text("Click me"))
    .on_press(Message::Clicked)
    .on_press_maybe(condition.then_some(Message::Clicked))  // Conditional
    .style(ButtonStyle::Primary)
    .padding([8, 16])

// Checkbox
checkbox("Enable feature", self.enabled)
    .on_toggle(Message::ToggleEnabled)

// Radio buttons
column(options.iter().map(|opt| {
    radio(opt.label, opt.value, self.selected, Message::Selected)
}))

// Pick list / dropdown
pick_list(&self.options, self.selected, Message::Selected)
    .placeholder("Select...")

// Slider
slider(0.0..=100.0, self.value, Message::ValueChanged)
    .step(1.0)

// Toggler
toggler(self.enabled)
    .label("Dark mode")
    .on_toggle(Message::ToggleDarkMode)
```

### Display Widgets

```rust
// Text with styling
text("Hello")
    .size(FONT_SIZE_TITLE)
    .color(palette.text)
    .font(Font::MONOSPACE)

// Image
image("path/to/image.png")
    .width(100)
    .height(100)

// SVG
svg(svg::Handle::from_path("icon.svg"))
    .width(24)
    .height(24)

// Progress bar
progress_bar(0.0..=100.0, self.progress)
    .height(4)

// Tooltip
tooltip(
    button("?").on_press(Message::Help),
    text("Help text here"),
    tooltip::Position::Right,
)
```

---

## Key Dependencies

```toml
[dependencies]
iced = { version = "0.14", features = ["advanced", "image", "svg"] }
tokio = { version = "1", features = ["full"] }

# Configuration
confy = "0.6"
serde = { version = "1", features = ["derive"] }

# File dialogs
rfd = "0.15"

# Desktop notifications (optional)
notify-rust = "4"

# System tray (optional)
tray-icon = "0.19"

# File watching (optional)
notify = "7"
```

---

## Anti-Patterns to Avoid

1. **Mutating state in `view()`** - View must be a pure function
2. **Blocking in `update()`** - Use `Task::perform()` for async work
3. **Giant monolithic Message enum** - Use nested enums for screens
4. **Hardcoded colors** - Use theme palettes for consistency
5. **Missing `Clone` on Message** - All Message types must be `Clone`
6. **Forgetting `Task::none()`** - Every match arm must return a Task
7. **Direct state access across screens** - Pass data via messages
8. **Ignoring subscription cleanup** - Subscriptions run until app closes

---

## Checklist Before Implementation

- [ ] Define clear `Message` enum with all user actions
- [ ] Structure state hierarchically (app → screen → component)
- [ ] Create theme palette with dark/light variants
- [ ] Plan navigation flow and screen transitions
- [ ] Identify async operations that need `Task::perform()`
- [ ] List subscriptions needed (keyboard, time, external events)
- [ ] Design modal/dialog system if needed
