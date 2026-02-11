//! Screen modules for the Silo application.
//!
//! Each screen is a separate module with its own view function.
//! The Screen enum provides routing between screens.

pub mod create_project;
pub mod create_task;
pub mod edit_project;
pub mod edit_task;
pub mod initiative_detail;
pub mod initiatives;
pub mod logs;
pub mod main_screen;
pub mod project_detail;
pub mod projects;
pub mod runs;
pub mod select_workspace;
pub mod settings;
pub mod start_worker;
pub mod tasks;
pub mod workers;

use crate::appearance::Palette;
use crate::message::{Message, SteeringFile, TaskFilter};
use granary_types::{
    Initiative, InitiativeSummary, Project, Run, RunnerConfig, Task as GranaryTask, TaskDependency,
    Worker,
};
use iced::widget::{Space, column, container, text, text_editor};
use iced::{Alignment, Element, Length};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use self::settings::{ConfigFormState, RunnerFormState, SteeringFormState};

/// Application screens for navigation.
#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    /// Initial screen for selecting a granary workspace folder
    SelectWorkspace,
    /// Initiative list - multi-project coordination
    Initiatives,
    /// Single initiative with its tasks
    InitiativeDetail { id: String },
    /// Project list - all projects
    Projects,
    /// Single project with its tasks
    ProjectDetail { id: String },
    /// All tasks view (filterable by status, priority, project)
    Tasks,
    /// Single task detail/edit view
    TaskDetail { id: String },
    /// New project form
    CreateProject,
    /// Edit project form
    EditProject { id: String },
    /// New task form
    CreateTask { project_id: Option<String> },
    /// Edit task form
    EditTask { id: String },
    /// Worker list - background automation
    Workers,
    /// Single worker detail with runs
    WorkerDetail { id: String },
    /// New worker form
    StartWorker,
    /// All runs view
    Runs,
    /// Single run detail
    RunDetail { id: String },
    /// Log viewer (for worker or run)
    Logs { source: LogSource },
    /// App settings (config, runners, steering)
    Settings,
}

/// Source of logs to display
#[derive(Debug, Clone, PartialEq)]
pub enum LogSource {
    Worker { id: String },
    Run { id: String },
}

/// State context passed to screen renderers.
///
/// This struct contains all the data needed to render any screen,
/// borrowed from the main application state.
pub struct ScreenContext<'a> {
    pub workspace: Option<&'a PathBuf>,
    pub projects: &'a [Project],
    pub tasks: &'a [GranaryTask],
    pub dependencies: &'a [TaskDependency],
    pub selected_project: Option<&'a String>,
    pub expanded_tasks: &'a HashSet<String>,
    pub new_task_title: &'a str,
    pub status_message: Option<&'a String>,
    pub loading: bool,
    pub spinner_frame: usize,
    // Task stats per project
    pub task_stats: &'a HashMap<String, crate::widget::TaskStats>,
    // Task filter
    pub task_filter: &'a TaskFilter,
    pub task_view_mode: tasks::TaskViewMode,
    // Workers and runners
    pub workers: &'a [Worker],
    pub runners: &'a [(String, RunnerConfig)],
    // Runs
    pub runs: &'a [Run],
    pub selected_run: Option<&'a Run>,
    pub worker_filter: Option<&'a String>,
    pub status_filter: Option<&'a String>,
    // Logs
    pub log_lines: &'a [String],
    pub log_follow: bool,
    // Settings
    pub steering_files: &'a [SteeringFile],
    pub config_entries: &'a [(String, String)],
    pub runner_form: &'a RunnerFormState,
    pub steering_form: &'a SteeringFormState,
    pub config_form: &'a ConfigFormState,
    // Start worker form
    pub start_worker_command: &'a str,
    pub start_worker_args: &'a str,
    pub start_worker_event: &'a str,
    pub start_worker_concurrency: &'a str,
    pub start_worker_poll_cooldown: &'a str,
    pub start_worker_detached: bool,
    pub start_worker_from_runner: Option<&'a str>,
    pub start_worker_env_vars: &'a [(String, String)],
    pub start_worker_error: Option<&'a str>,
    pub start_worker_submitting: bool,
    // Create/edit task forms
    pub create_task_form: Option<&'a create_task::CreateTaskForm>,
    pub edit_task_form: Option<&'a edit_task::EditTaskForm>,
    // Create project form
    pub create_project_name: &'a str,
    pub create_project_desc_content: &'a text_editor::Content,
    pub create_project_owner: &'a str,
    pub create_project_tags: &'a str,
    // Edit project form
    pub edit_project_form: Option<&'a edit_project::EditProjectForm>,
    pub edit_project_desc_content: &'a text_editor::Content,
    // Description content for task forms
    pub create_task_desc_content: &'a text_editor::Content,
    pub edit_task_desc_content: &'a text_editor::Content,
    // Initiatives
    pub initiatives: &'a [Initiative],
    pub selected_initiative: Option<&'a Initiative>,
    pub initiative_summary: Option<&'a InitiativeSummary>,
}

/// Dispatches view rendering to the appropriate screen module.
///
/// This is the central routing function for all screen views.
/// Each screen receives the ScreenContext and palette to render itself.
pub fn dispatch_view<'a>(
    screen: &'a Screen,
    ctx: &ScreenContext<'a>,
    palette: &'a Palette,
) -> Element<'a, Message> {
    match screen {
        Screen::SelectWorkspace => select_workspace::view(palette),

        Screen::Projects => {
            let state = projects::ProjectsScreenState {
                projects: ctx.projects,
                selected_project: ctx.selected_project,
                task_stats: ctx.task_stats,
                loading: ctx.loading,
                status_message: ctx.status_message,
            };
            projects::view(state, palette)
        }

        Screen::ProjectDetail { id } => {
            if let Some(project) = ctx.projects.iter().find(|p| &p.id == id) {
                let state = project_detail::ProjectDetailState {
                    project,
                    tasks: ctx.tasks,
                    expanded_tasks: ctx.expanded_tasks,
                    new_task_title: ctx.new_task_title,
                    loading: ctx.loading,
                    status_message: ctx.status_message,
                };
                project_detail::view(state, palette)
            } else {
                placeholder_screen("Project Detail", id, palette)
            }
        }

        Screen::Initiatives => {
            let state = initiatives::InitiativesScreenState {
                initiatives: ctx.initiatives,
                loading: ctx.loading,
                status_message: ctx.status_message,
            };
            initiatives::view(state, palette)
        }

        Screen::InitiativeDetail { id } => {
            if let Some(initiative) = ctx.initiatives.iter().find(|i| &i.id == id) {
                let state = initiative_detail::InitiativeDetailState {
                    initiative,
                    summary: ctx.initiative_summary,
                    tasks: ctx.tasks,
                    expanded_tasks: ctx.expanded_tasks,
                    loading: ctx.loading,
                    status_message: ctx.status_message,
                };
                initiative_detail::view(state, palette)
            } else {
                placeholder_screen("Initiative Detail", id, palette)
            }
        }

        Screen::Tasks => {
            let project_name = ctx
                .selected_project
                .and_then(|id| ctx.projects.iter().find(|p| &p.id == id))
                .map(|p| p.name.as_str())
                .unwrap_or("All Tasks");

            let state = tasks::TasksScreenState {
                workspace: ctx.workspace,
                project_id: ctx.selected_project,
                project_name,
                tasks: ctx.tasks,
                dependencies: ctx.dependencies,
                expanded_tasks: ctx.expanded_tasks,
                filter: ctx.task_filter,
                view_mode: ctx.task_view_mode,
                new_task_title: ctx.new_task_title,
                loading: ctx.loading,
            };
            tasks::view(state, palette)
        }

        Screen::TaskDetail { id } => placeholder_screen("Task Detail", id, palette),

        Screen::CreateProject => {
            let state = create_project::CreateProjectState {
                name: ctx.create_project_name,
                description_content: ctx.create_project_desc_content,
                owner: ctx.create_project_owner,
                tags: ctx.create_project_tags,
                loading: ctx.loading,
                error_message: ctx.status_message,
            };
            create_project::view(state, palette)
        }

        Screen::EditProject { id: _ } => {
            if let Some(form) = ctx.edit_project_form {
                let state = edit_project::EditProjectState {
                    form,
                    description_content: ctx.edit_project_desc_content,
                    loading: ctx.loading,
                    error_message: ctx.status_message,
                };
                edit_project::view(state, palette)
            } else {
                placeholder_screen("Edit Project", "Loading project...", palette)
            }
        }

        Screen::CreateTask { project_id } => {
            if let Some(form) = ctx.create_task_form {
                let project_name = project_id
                    .as_ref()
                    .and_then(|id| ctx.projects.iter().find(|p| &p.id == id))
                    .map(|p| p.name.as_str())
                    .unwrap_or("New Project");

                let state = create_task::CreateTaskScreenState {
                    project_name,
                    form,
                    description_content: ctx.create_task_desc_content,
                    available_tasks: ctx.tasks,
                };
                create_task::view(state, palette)
            } else {
                placeholder_screen("Create Task", "Form not initialized", palette)
            }
        }

        Screen::EditTask { id } => {
            if let Some(form) = ctx.edit_task_form {
                // Find project name from task's project_id if available
                let project_name = ctx
                    .tasks
                    .iter()
                    .find(|t| &t.id == id)
                    .and_then(|t| {
                        ctx.projects
                            .iter()
                            .find(|p| p.id == t.project_id)
                            .map(|p| p.name.as_str())
                    })
                    .unwrap_or("Task");

                let state = edit_task::EditTaskScreenState {
                    project_name,
                    form,
                    description_content: ctx.edit_task_desc_content,
                    available_tasks: ctx.tasks,
                    loading: ctx.loading,
                };
                edit_task::view(state, palette)
            } else {
                placeholder_screen("Edit Task", "Loading task...", palette)
            }
        }

        Screen::Workers => {
            let state = workers::WorkersScreenState {
                runners: ctx.runners,
                workers: ctx.workers,
                workspace: ctx.workspace,
                loading: ctx.loading,
            };
            workers::view(state, palette)
        }

        Screen::WorkerDetail { id } => placeholder_screen("Worker Detail", id, palette),

        Screen::StartWorker => {
            let state = start_worker::StartWorkerFormState {
                from_runner: ctx.start_worker_from_runner,
                command: ctx.start_worker_command,
                args: ctx.start_worker_args,
                event_type: ctx.start_worker_event,
                concurrency: ctx.start_worker_concurrency,
                poll_cooldown: ctx.start_worker_poll_cooldown,
                detached: ctx.start_worker_detached,
                env_vars: ctx.start_worker_env_vars,
                error: ctx.start_worker_error,
                submitting: ctx.start_worker_submitting,
            };
            start_worker::view(state, palette)
        }

        Screen::Runs => {
            let state = runs::RunsScreenState {
                runs: ctx.runs,
                selected_run: ctx.selected_run,
                worker_filter: ctx.worker_filter,
                status_filter: ctx.status_filter,
                loading: ctx.loading,
            };
            runs::view(state, palette)
        }

        Screen::RunDetail { id } => placeholder_screen("Run Detail", id, palette),

        Screen::Logs { source } => {
            let state = logs::LogsScreenState {
                log_source: source,
                lines: ctx.log_lines,
                follow: ctx.log_follow,
                loading: ctx.loading,
            };
            logs::view(state, palette)
        }

        Screen::Settings => {
            let state = settings::SettingsScreenState {
                runners: ctx.runners,
                steering_files: ctx.steering_files,
                config_entries: ctx.config_entries,
                loading: ctx.loading,
                runner_form: ctx.runner_form,
                steering_form: ctx.steering_form,
                config_form: ctx.config_form,
            };
            settings::view(state, palette)
        }
    }
}

/// Placeholder screen for unimplemented views.
fn placeholder_screen<'a>(
    title: &'a str,
    subtitle: &'a str,
    palette: &'a Palette,
) -> Element<'a, Message> {
    container(
        column![
            text(title).size(28).color(palette.text),
            Space::with_height(8),
            text(subtitle).size(14).color(palette.text_muted),
        ]
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}
