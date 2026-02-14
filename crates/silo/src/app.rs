use iced::widget::{column, container, horizontal_space, row, stack, text_editor};
use iced::{Background, Element, Length, Subscription, Task};

use crate::appearance::{self, Palette};
use crate::config::SiloConfig;
use crate::granary_cli::{
    add_action, add_comment, add_dependency, add_runner, add_steering, archive_initiative,
    archive_project, block_task, complete_task, create_project, create_task, create_task_full,
    delete_config, list_actions, list_config, list_runners, list_steering, load_comments,
    load_initiative_summary, load_initiatives, load_projects, load_run_logs, load_runs, load_task,
    load_tasks, load_worker_logs, load_workers, pause_run, prune_workers, ready_project,
    remove_action, remove_runner, remove_steering, reopen_task, resume_run, set_config, start_task,
    start_worker_from_action, start_worker_from_runner, start_worker_inline, stop_run, stop_worker,
    unarchive_project, update_action, update_project, update_runner, update_task,
};
use crate::message::{Message, SteeringFile, TaskFilter};
use crate::screen;
use crate::screen::create_task::CreateTaskForm;
use crate::screen::edit_task::EditTaskForm;
use crate::screen::settings::{
    ActionFormState, ConfigFormState, RunnerFormState, SteeringFormState,
};
use crate::screen::tasks::TaskViewMode;
use crate::widget::{self, icon};
use granary_types::{
    ActionConfig, Comment, Initiative, InitiativeSummary, Project, Run, RunnerConfig,
    Task as GranaryTask, TaskDependency, Worker,
};
use lucide_icons::Icon;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Wrapper for text_editor::Content that implements Debug.
pub struct EditorContent(pub text_editor::Content);

impl std::fmt::Debug for EditorContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditorContent").finish()
    }
}

impl Default for EditorContent {
    fn default() -> Self {
        Self(text_editor::Content::new())
    }
}

impl EditorContent {
    fn with_text(text: &str) -> Self {
        Self(text_editor::Content::with_text(text))
    }
}

/// Form state for starting a new worker.
#[derive(Debug, Default)]
pub struct StartWorkerForm {
    pub from_runner: Option<String>,
    pub command: String,
    pub args: String,
    pub event_type: String,
    pub concurrency: String,
    pub poll_cooldown: String,
    pub detached: bool,
    pub submitting: bool,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct Silo {
    screen: screen::Screen,
    screen_history: Vec<screen::Screen>,
    workspace: Option<PathBuf>,
    workspace_dropdown_open: bool,
    recent_workspaces: Vec<PathBuf>,
    projects: Vec<Project>,
    tasks: Vec<GranaryTask>,
    dependencies: Vec<TaskDependency>,
    selected_project: Option<String>,
    expanded_tasks: HashSet<String>,
    new_task_title: String,
    status_message: Option<String>,
    loading: bool,

    // Auto-refresh state
    auto_refresh_enabled: bool,
    last_refresh: Option<std::time::Instant>,

    /// Current spinner animation frame (0-9)
    spinner_frame: usize,

    // Log viewing state
    log_source: Option<screen::LogSource>,
    log_lines: Vec<String>,
    log_follow: bool,
    log_loading: bool,

    // Settings state
    runners: Vec<(String, RunnerConfig)>,
    actions: Vec<(String, ActionConfig)>,
    steering_files: Vec<SteeringFile>,
    config_entries: Vec<(String, String)>,
    runner_form: RunnerFormState,
    action_form: ActionFormState,
    steering_form: SteeringFormState,
    config_form: ConfigFormState,

    // Workers state
    workers: Vec<Worker>,
    workers_loading: bool,
    start_worker_form: StartWorkerForm,

    // Runs state
    runs: Vec<Run>,
    selected_run: Option<Run>,
    run_worker_filter: Option<String>,
    run_status_filter: Option<String>,

    // Initiatives state
    initiatives: Vec<Initiative>,
    selected_initiative: Option<Initiative>,
    initiative_summary: Option<InitiativeSummary>,

    // Task filter and view mode
    task_filter: TaskFilter,
    task_view_mode: TaskViewMode,

    // Create project form state
    create_project_name: String,
    create_project_description: String,
    create_project_owner: String,
    create_project_tags: String,

    // Create task form state
    create_task_form: Option<CreateTaskForm>,

    // Edit task form state
    edit_task_form: Option<EditTaskForm>,

    // Edit project form state
    edit_project_form: Option<screen::edit_project::EditProjectForm>,

    // Description editor content (stored separately as Content doesn't impl Clone)
    create_task_desc_content: EditorContent,
    edit_task_desc_content: EditorContent,
    create_project_desc_content: EditorContent,
    edit_project_desc_content: EditorContent,

    // Block task dialog
    blocking_task_id: Option<String>,
    block_reason: String,

    // Comments state
    /// Comments for the currently expanded task
    pub task_comments: HashMap<String, Vec<Comment>>,
    /// Text input for new comment
    pub comment_input: String,
    /// Loading state for comments
    pub comments_loading: bool,
}

impl Silo {
    pub fn new() -> (Self, Task<Message>) {
        // Load recent workspaces from config
        let config = SiloConfig::load();
        let recent_workspaces = config.recent_workspaces().to_vec();

        // Default workspace is $HOME (panic if no home dir - can't function without it)
        let workspace = dirs::home_dir().expect("Could not determine home directory");

        let silo = Self {
            screen: screen::Screen::Projects,
            screen_history: Vec::new(),
            workspace: Some(workspace.clone()),
            workspace_dropdown_open: false,
            recent_workspaces,
            projects: Vec::new(),
            tasks: Vec::new(),
            dependencies: Vec::new(),
            selected_project: None,
            expanded_tasks: HashSet::new(),
            new_task_title: String::new(),
            status_message: None,
            loading: true,

            // Auto-refresh enabled by default
            auto_refresh_enabled: true,
            last_refresh: None,

            // Spinner animation state
            spinner_frame: 0,

            // Log viewing state
            log_source: None,
            log_lines: Vec::new(),
            log_follow: true,
            log_loading: false,

            // Settings state
            runners: Vec::new(),
            actions: Vec::new(),
            steering_files: Vec::new(),
            config_entries: Vec::new(),
            runner_form: RunnerFormState::default(),
            action_form: ActionFormState::default(),
            steering_form: SteeringFormState::default(),
            config_form: ConfigFormState::default(),

            // Workers state
            workers: Vec::new(),
            workers_loading: false,
            start_worker_form: StartWorkerForm::default(),

            // Runs state
            runs: Vec::new(),
            selected_run: None,
            run_worker_filter: None,
            run_status_filter: None,

            // Initiatives state
            initiatives: Vec::new(),
            selected_initiative: None,
            initiative_summary: None,

            // Task filter and view mode
            task_filter: TaskFilter::default(),
            task_view_mode: TaskViewMode::default(),

            // Create project form state
            create_project_name: String::new(),
            create_project_description: String::new(),
            create_project_owner: String::new(),
            create_project_tags: String::new(),

            // Create task form state
            create_task_form: None,

            // Edit task form state
            edit_task_form: None,

            // Edit project form state
            edit_project_form: None,

            // Description editor content
            create_task_desc_content: EditorContent::default(),
            edit_task_desc_content: EditorContent::default(),
            create_project_desc_content: EditorContent::default(),
            edit_project_desc_content: EditorContent::default(),

            // Block task dialog
            blocking_task_id: None,
            block_reason: String::new(),

            // Comments state
            task_comments: HashMap::new(),
            comment_input: String::new(),
            comments_loading: false,
        };

        // Load projects on startup
        let startup_task = Task::perform(load_projects(workspace), Message::ProjectsLoaded);

        (silo, startup_task)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectWorkspace => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Select Granary Workspace")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::WorkspaceSelected,
            ),
            Message::WorkspaceSelected(path) => {
                if let Some(p) = path {
                    self.workspace = Some(p.clone());
                    self.workspace_dropdown_open = false;
                    self.screen = screen::Screen::Projects;
                    self.loading = true;

                    // Persist to recent workspaces
                    let mut config = SiloConfig::load();
                    config.add_recent_workspace(p.clone());
                    if let Err(e) = config.save() {
                        eprintln!("Failed to save recent workspaces: {}", e);
                    }
                    self.recent_workspaces = config.recent_workspaces().to_vec();

                    return Task::perform(load_projects(p), Message::ProjectsLoaded);
                }
                Task::none()
            }
            Message::Navigate(screen) => {
                self.screen_history.push(self.screen.clone());
                self.screen = screen.clone();

                // Load data for the new screen
                if let Some(ws) = &self.workspace {
                    match screen {
                        screen::Screen::Initiatives => {
                            self.loading = true;
                            return Task::perform(
                                load_initiatives(ws.clone()),
                                Message::InitiativesLoaded,
                            );
                        }
                        screen::Screen::Workers => {
                            self.workers_loading = true;
                            let ws1 = ws.clone();
                            let ws2 = ws.clone();
                            let ws3 = ws.clone();
                            return Task::batch([
                                Task::perform(
                                    async move {
                                        list_runners(ws1)
                                            .await
                                            .map(|map| map.into_iter().collect::<Vec<_>>())
                                    },
                                    Message::RunnersLoaded,
                                ),
                                Task::perform(
                                    async move {
                                        list_actions(ws2)
                                            .await
                                            .map(|map| map.into_iter().collect::<Vec<_>>())
                                    },
                                    Message::ActionsLoaded,
                                ),
                                Task::perform(
                                    async move { load_workers(ws3, true).await },
                                    Message::WorkersLoaded,
                                ),
                            ]);
                        }
                        screen::Screen::Runs => {
                            self.loading = true;
                            return Task::perform(
                                load_runs(ws.clone(), None, None),
                                Message::RunsLoaded,
                            );
                        }
                        screen::Screen::Settings => {
                            self.loading = true;
                            let ws1 = ws.clone();
                            let ws2 = ws.clone();
                            let ws3 = ws.clone();
                            let ws4 = ws.clone();
                            return Task::batch([
                                Task::perform(
                                    async move {
                                        list_runners(ws1)
                                            .await
                                            .map(|map| map.into_iter().collect::<Vec<_>>())
                                    },
                                    Message::RunnersLoaded,
                                ),
                                Task::perform(
                                    async move {
                                        list_actions(ws2)
                                            .await
                                            .map(|map| map.into_iter().collect::<Vec<_>>())
                                    },
                                    Message::ActionsLoaded,
                                ),
                                Task::perform(
                                    async move {
                                        list_steering(ws3).await.map(|files| {
                                            files
                                                .into_iter()
                                                .map(|f| SteeringFile {
                                                    id: f.id,
                                                    path: f.path,
                                                    mode: f.mode,
                                                    scope_type: f.scope_type,
                                                    scope_id: f.scope_id,
                                                })
                                                .collect()
                                        })
                                    },
                                    Message::SteeringLoaded,
                                ),
                                Task::perform(
                                    async move {
                                        list_config(ws4).await.map(|entries| {
                                            entries.into_iter().map(|e| (e.key, e.value)).collect()
                                        })
                                    },
                                    Message::ConfigLoaded,
                                ),
                            ]);
                        }
                        screen::Screen::Tasks => {
                            if let Some(proj) = &self.selected_project {
                                self.loading = true;
                                return Task::perform(
                                    load_tasks(ws.clone(), proj.clone()),
                                    Message::TasksLoaded,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                Task::none()
            }
            Message::GoBack => {
                if let Some(previous) = self.screen_history.pop() {
                    self.screen = previous;
                }
                Task::none()
            }
            Message::ToggleWorkspaceDropdown => {
                self.workspace_dropdown_open = !self.workspace_dropdown_open;
                Task::none()
            }
            Message::SelectRecentWorkspace(path) => {
                self.workspace = Some(path.clone());
                self.workspace_dropdown_open = false;
                self.screen = screen::Screen::Projects;
                self.loading = true;

                // Persist to recent workspaces (moves selected to top)
                let mut config = SiloConfig::load();
                config.add_recent_workspace(path.clone());
                if let Err(e) = config.save() {
                    eprintln!("Failed to save recent workspaces: {}", e);
                }
                self.recent_workspaces = config.recent_workspaces().to_vec();

                Task::perform(load_projects(path), Message::ProjectsLoaded)
            }
            Message::AutoRefresh => {
                // Refresh data for current screen based on context
                match &self.screen {
                    screen::Screen::Projects => {
                        if let Some(ws) = &self.workspace {
                            return Task::perform(
                                load_projects(ws.clone()),
                                Message::ProjectsLoaded,
                            );
                        }
                    }
                    screen::Screen::ProjectDetail { .. } => {
                        if let Some(ws) = &self.workspace {
                            let mut tasks = Vec::new();
                            // Refresh projects list
                            tasks.push(Task::perform(
                                load_projects(ws.clone()),
                                Message::ProjectsLoaded,
                            ));
                            // Refresh tasks for selected project
                            if let Some(proj) = &self.selected_project {
                                tasks.push(Task::perform(
                                    load_tasks(ws.clone(), proj.clone()),
                                    Message::TasksLoaded,
                                ));
                            }
                            return Task::batch(tasks);
                        }
                    }
                    screen::Screen::Tasks => {
                        if let (Some(ws), Some(proj)) = (&self.workspace, &self.selected_project) {
                            return Task::perform(
                                load_tasks(ws.clone(), proj.clone()),
                                Message::TasksLoaded,
                            );
                        }
                    }
                    screen::Screen::Initiatives => {
                        if let Some(ws) = &self.workspace {
                            return Task::perform(
                                load_initiatives(ws.clone()),
                                Message::InitiativesLoaded,
                            );
                        }
                    }
                    screen::Screen::InitiativeDetail { id } => {
                        if let Some(ws) = &self.workspace {
                            return Task::perform(
                                load_initiative_summary(ws.clone(), id.clone()),
                                Message::InitiativeDetailLoaded,
                            );
                        }
                    }
                    screen::Screen::Workers => {
                        if let Some(ws) = &self.workspace {
                            return Task::perform(
                                load_workers(ws.clone(), true),
                                Message::WorkersLoaded,
                            );
                        }
                    }
                    screen::Screen::Runs => {
                        if let Some(ws) = &self.workspace {
                            return Task::perform(
                                load_runs(
                                    ws.clone(),
                                    self.run_worker_filter.clone(),
                                    self.run_status_filter.clone(),
                                ),
                                Message::RunsLoaded,
                            );
                        }
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::ToggleAutoRefresh => {
                self.auto_refresh_enabled = !self.auto_refresh_enabled;
                Task::none()
            }
            Message::RefreshComplete(instant) => {
                self.last_refresh = Some(instant);
                Task::none()
            }
            Message::SpinnerTick => {
                self.spinner_frame = (self.spinner_frame + 1) % 10;
                Task::none()
            }
            Message::ProjectsLoaded(result) => {
                match result {
                    Ok(projects) => {
                        self.projects = projects;
                        self.status_message = None;
                    }
                    Err(e) => {
                        self.status_message = Some(e);
                    }
                }
                self.loading = false;
                Task::none()
            }
            Message::TasksLoaded(result) => {
                match result {
                    Ok(tasks) => {
                        self.tasks = tasks;
                        self.status_message = None;
                    }
                    Err(e) => {
                        self.status_message = Some(e);
                    }
                }
                self.loading = false;
                Task::none()
            }
            Message::SelectProject(project_id) => {
                self.selected_project = Some(project_id.clone());
                self.loading = true;
                if let Some(ws) = &self.workspace {
                    return Task::perform(load_tasks(ws.clone(), project_id), Message::TasksLoaded);
                }
                Task::none()
            }
            Message::RefreshProjects => {
                self.loading = true;
                if let Some(ws) = &self.workspace {
                    return Task::perform(load_projects(ws.clone()), Message::ProjectsLoaded);
                }
                Task::none()
            }
            Message::ArchiveProject(project_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(
                        archive_project(ws.clone(), project_id),
                        Message::ProjectArchived,
                    );
                }
                Task::none()
            }
            Message::ProjectArchived(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    // Clear selection if archived project was selected
                    self.selected_project = None;
                    self.tasks.clear();
                    return self.update(Message::RefreshProjects);
                }
                Task::none()
            }
            Message::UnarchiveProject(project_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(
                        unarchive_project(ws.clone(), project_id),
                        Message::ProjectUnarchived,
                    );
                }
                Task::none()
            }
            Message::ProjectUnarchived(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::RefreshProjects);
                }
                Task::none()
            }
            Message::ReadyProject(project_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(
                        ready_project(ws.clone(), project_id),
                        Message::ProjectReadied,
                    );
                }
                Task::none()
            }
            Message::ProjectReadied(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    // Refresh tasks to show updated statuses
                    return self.update(Message::RefreshTasks);
                }
                Task::none()
            }
            Message::RefreshTasks => {
                self.loading = true;
                if let (Some(ws), Some(proj)) = (&self.workspace, &self.selected_project) {
                    return Task::perform(
                        load_tasks(ws.clone(), proj.clone()),
                        Message::TasksLoaded,
                    );
                }
                Task::none()
            }
            Message::NewTaskTitle(title) => {
                self.new_task_title = title;
                Task::none()
            }
            Message::CreateTask => {
                if self.new_task_title.is_empty() {
                    return Task::none();
                }
                if let (Some(ws), Some(proj)) = (&self.workspace, &self.selected_project) {
                    let title = self.new_task_title.clone();
                    self.new_task_title.clear();
                    self.loading = true;
                    return Task::perform(
                        create_task(ws.clone(), proj.clone(), title),
                        Message::TaskCreated,
                    );
                }
                Task::none()
            }
            Message::TaskCreated(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::RefreshTasks);
                }
                Task::none()
            }
            Message::StartTask(task_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(start_task(ws.clone(), task_id), Message::TaskUpdated);
                }
                Task::none()
            }
            Message::CompleteTask(task_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(complete_task(ws.clone(), task_id), Message::TaskUpdated);
                }
                Task::none()
            }
            Message::TaskUpdated(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::RefreshTasks);
                }
                Task::none()
            }
            Message::ToggleTaskExpand(task_id) => {
                if self.expanded_tasks.contains(&task_id) {
                    self.expanded_tasks.remove(&task_id);
                    Task::none()
                } else {
                    self.expanded_tasks.insert(task_id.clone());
                    // Load comments when task is expanded
                    self.update(Message::LoadComments(task_id))
                }
            }
            Message::OpenWorkerLogs(worker_id) => {
                self.log_source = Some(screen::LogSource::Worker {
                    id: worker_id.clone(),
                });
                self.log_lines.clear();
                self.log_follow = true;
                self.log_loading = true;
                self.screen = screen::Screen::Logs {
                    source: screen::LogSource::Worker {
                        id: worker_id.clone(),
                    },
                };
                if let Some(ws) = &self.workspace {
                    return Task::perform(
                        load_worker_logs(ws.clone(), worker_id, None),
                        Message::LogsLoaded,
                    );
                }
                Task::none()
            }
            Message::OpenRunLogs(run_id) => {
                self.log_source = Some(screen::LogSource::Run { id: run_id.clone() });
                self.log_lines.clear();
                self.log_follow = true;
                self.log_loading = true;
                self.screen = screen::Screen::Logs {
                    source: screen::LogSource::Run { id: run_id.clone() },
                };
                if let Some(ws) = &self.workspace {
                    return Task::perform(
                        load_run_logs(ws.clone(), run_id, None),
                        Message::LogsLoaded,
                    );
                }
                Task::none()
            }
            Message::LogsLoaded(result) => {
                self.log_loading = false;
                match result {
                    Ok(lines) => self.log_lines = lines,
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::LogsAppended(result) => {
                if let Ok(new_lines) = result {
                    self.log_lines.extend(new_lines);
                }
                Task::none()
            }
            Message::ToggleLogFollow(follow) => {
                self.log_follow = follow;
                Task::none()
            }
            Message::ClearLogs => {
                self.log_lines.clear();
                Task::none()
            }
            Message::RefreshLogs => {
                // Re-fetch logs when follow mode is active
                if let (Some(ws), Some(source)) = (&self.workspace, &self.log_source) {
                    match source {
                        screen::LogSource::Worker { id } => {
                            return Task::perform(
                                load_worker_logs(ws.clone(), id.clone(), None),
                                Message::LogsLoaded,
                            );
                        }
                        screen::LogSource::Run { id } => {
                            return Task::perform(
                                load_run_logs(ws.clone(), id.clone(), None),
                                Message::LogsLoaded,
                            );
                        }
                    }
                }
                Task::none()
            }
            Message::OpenSettings => {
                self.screen_history.push(self.screen.clone());
                self.screen = screen::Screen::Settings;
                self.loading = true;
                if let Some(ws) = &self.workspace {
                    let ws1 = ws.clone();
                    let ws2 = ws.clone();
                    let ws3 = ws.clone();
                    return Task::batch([
                        Task::perform(
                            async move {
                                list_runners(ws1)
                                    .await
                                    .map(|map| map.into_iter().collect::<Vec<_>>())
                            },
                            Message::RunnersLoaded,
                        ),
                        Task::perform(
                            async move {
                                list_steering(ws2).await.map(|files| {
                                    files
                                        .into_iter()
                                        .map(|f| SteeringFile {
                                            id: f.id,
                                            path: f.path,
                                            mode: f.mode,
                                            scope_type: f.scope_type,
                                            scope_id: f.scope_id,
                                        })
                                        .collect()
                                })
                            },
                            Message::SteeringLoaded,
                        ),
                        Task::perform(
                            async move {
                                list_config(ws3).await.map(|entries| {
                                    entries.into_iter().map(|e| (e.key, e.value)).collect()
                                })
                            },
                            Message::ConfigLoaded,
                        ),
                    ]);
                }
                Task::none()
            }
            Message::CloseSettings => {
                self.screen = self
                    .screen_history
                    .pop()
                    .unwrap_or(screen::Screen::Projects);
                Task::none()
            }
            Message::LoadRunners => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move {
                            list_runners(ws)
                                .await
                                .map(|map| map.into_iter().collect::<Vec<_>>())
                        },
                        Message::RunnersLoaded,
                    );
                }
                Task::none()
            }
            Message::RunnersLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(runners) => self.runners = runners,
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::RunnerFormChanged { field, value } => {
                match field.as_str() {
                    "name" => self.runner_form.name = value,
                    "command" => self.runner_form.command = value,
                    "args" => self.runner_form.args = value,
                    "on" => self.runner_form.on_event = value,
                    "concurrency" => self.runner_form.concurrency = value,
                    // Steering form fields
                    "steering_path" => self.steering_form.path = value,
                    "steering_mode" => self.steering_form.mode = value,
                    "steering_project" => self.steering_form.project = value,
                    // Config form fields
                    "config_key" => self.config_form.key = value,
                    "config_value" => self.config_form.value = value,
                    _ => {}
                }
                Task::none()
            }
            Message::EditRunner(name) => {
                // Find the runner by name and populate the form
                if let Some((_, config)) = self.runners.iter().find(|(n, _)| n == &name) {
                    self.runner_form = RunnerFormState {
                        name: name.clone(),
                        command: config.command.clone(),
                        args: config.args.join(", "),
                        concurrency: config
                            .concurrency
                            .map(|c| c.to_string())
                            .unwrap_or_default(),
                        on_event: config.on.clone().unwrap_or_default(),
                        editing: Some(name),
                    };
                }
                Task::none()
            }
            Message::CancelEditRunner => {
                // Clear the form and exit edit mode
                self.runner_form = RunnerFormState::default();
                Task::none()
            }
            Message::SaveRunner => {
                if let Some(ws) = &self.workspace {
                    if self.runner_form.name.is_empty() || self.runner_form.command.is_empty() {
                        return Task::none();
                    }
                    self.loading = true;
                    let ws = ws.clone();
                    let name = self.runner_form.name.clone();
                    let command = self.runner_form.command.clone();
                    let args = if self.runner_form.args.is_empty() {
                        None
                    } else {
                        Some(
                            self.runner_form
                                .args
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .collect(),
                        )
                    };
                    let on_event = if self.runner_form.on_event.is_empty() {
                        None
                    } else {
                        Some(self.runner_form.on_event.clone())
                    };
                    let concurrency = self.runner_form.concurrency.parse().ok();
                    let is_edit = self.runner_form.editing.is_some();
                    return Task::perform(
                        async move {
                            if is_edit {
                                update_runner(
                                    ws,
                                    name,
                                    Some(command),
                                    args,
                                    on_event,
                                    concurrency,
                                    None,
                                )
                                .await
                            } else {
                                add_runner(ws, name, command, args, on_event, concurrency, None)
                                    .await
                            }
                        },
                        Message::RunnerSaved,
                    );
                }
                Task::none()
            }
            Message::RunnerSaved(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    // Clear form and refresh
                    self.runner_form = RunnerFormState::default();
                    return self.update(Message::LoadRunners);
                }
                Task::none()
            }
            Message::DeleteRunner(name) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move { remove_runner(ws, name).await },
                        Message::RunnerDeleted,
                    );
                }
                Task::none()
            }
            Message::RunnerDeleted(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::LoadRunners);
                }
                Task::none()
            }
            // ========== Action management ==========
            Message::LoadActions => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move {
                            list_actions(ws)
                                .await
                                .map(|map| map.into_iter().collect::<Vec<_>>())
                        },
                        Message::ActionsLoaded,
                    );
                }
                Task::none()
            }
            Message::ActionsLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(actions) => self.actions = actions,
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::ActionFormChanged { field, value } => {
                match field.as_str() {
                    "name" => self.action_form.name = value,
                    "command" => self.action_form.command = value,
                    "args" => self.action_form.args = value,
                    "on" => self.action_form.on_event = value,
                    "concurrency" => self.action_form.concurrency = value,
                    _ => {}
                }
                Task::none()
            }
            Message::EditAction(name) => {
                if let Some((_, config)) = self.actions.iter().find(|(n, _)| n == &name) {
                    self.action_form = ActionFormState {
                        name: name.clone(),
                        command: config.command.clone(),
                        args: config.args.join(", "),
                        concurrency: config
                            .concurrency
                            .map(|c| c.to_string())
                            .unwrap_or_default(),
                        on_event: config.on.clone().unwrap_or_default(),
                        editing: Some(name),
                    };
                }
                Task::none()
            }
            Message::CancelEditAction => {
                self.action_form = ActionFormState::default();
                Task::none()
            }
            Message::SaveAction => {
                if let Some(ws) = &self.workspace {
                    if self.action_form.name.is_empty() || self.action_form.command.is_empty() {
                        return Task::none();
                    }
                    self.loading = true;
                    let ws = ws.clone();
                    let name = self.action_form.name.clone();
                    let command = self.action_form.command.clone();
                    let args = if self.action_form.args.is_empty() {
                        None
                    } else {
                        Some(
                            self.action_form
                                .args
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .collect(),
                        )
                    };
                    let on_event = if self.action_form.on_event.is_empty() {
                        None
                    } else {
                        Some(self.action_form.on_event.clone())
                    };
                    let concurrency = self.action_form.concurrency.parse().ok();
                    let is_edit = self.action_form.editing.is_some();
                    return Task::perform(
                        async move {
                            if is_edit {
                                update_action(
                                    ws,
                                    name,
                                    Some(command),
                                    args,
                                    on_event,
                                    concurrency,
                                    None,
                                )
                                .await
                            } else {
                                add_action(ws, name, command, args, on_event, concurrency, None)
                                    .await
                            }
                        },
                        Message::ActionSaved,
                    );
                }
                Task::none()
            }
            Message::ActionSaved(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    self.action_form = ActionFormState::default();
                    return self.update(Message::LoadActions);
                }
                Task::none()
            }
            Message::DeleteAction(name) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move { remove_action(ws, name).await },
                        Message::ActionDeleted,
                    );
                }
                Task::none()
            }
            Message::ActionDeleted(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::LoadActions);
                }
                Task::none()
            }
            Message::LoadSteering => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move {
                            list_steering(ws).await.map(|files| {
                                files
                                    .into_iter()
                                    .map(|f| SteeringFile {
                                        id: f.id,
                                        path: f.path,
                                        mode: f.mode,
                                        scope_type: f.scope_type,
                                        scope_id: f.scope_id,
                                    })
                                    .collect()
                            })
                        },
                        Message::SteeringLoaded,
                    );
                }
                Task::none()
            }
            Message::SteeringLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(files) => self.steering_files = files,
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::AddSteering {
                path,
                mode,
                project,
            } => {
                if let Some(ws) = &self.workspace {
                    if path.is_empty() {
                        return Task::none();
                    }
                    self.loading = true;
                    let ws = ws.clone();
                    let mode_opt = if mode.is_empty() { None } else { Some(mode) };
                    return Task::perform(
                        async move { add_steering(ws, path, mode_opt, project, None, false).await },
                        Message::SteeringAdded,
                    );
                }
                Task::none()
            }
            Message::SteeringAdded(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    // Clear form and refresh
                    self.steering_form = SteeringFormState::default();
                    return self.update(Message::LoadSteering);
                }
                Task::none()
            }
            Message::RemoveSteering(path) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move { remove_steering(ws, path).await },
                        Message::SteeringRemoved,
                    );
                }
                Task::none()
            }
            Message::SteeringRemoved(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::LoadSteering);
                }
                Task::none()
            }
            Message::LoadConfig => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move {
                            list_config(ws).await.map(|entries| {
                                entries.into_iter().map(|e| (e.key, e.value)).collect()
                            })
                        },
                        Message::ConfigLoaded,
                    );
                }
                Task::none()
            }
            Message::ConfigLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(entries) => self.config_entries = entries,
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::SetConfig { key, value } => {
                if let Some(ws) = &self.workspace {
                    if key.is_empty() || value.is_empty() {
                        return Task::none();
                    }
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move { set_config(ws, key, value).await },
                        Message::ConfigSet,
                    );
                }
                Task::none()
            }
            Message::ConfigSet(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    // Clear form and refresh
                    self.config_form = ConfigFormState::default();
                    return self.update(Message::LoadConfig);
                }
                Task::none()
            }
            Message::DeleteConfig(key) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let ws = ws.clone();
                    return Task::perform(
                        async move { delete_config(ws, key).await },
                        Message::ConfigDeleted,
                    );
                }
                Task::none()
            }
            Message::ConfigDeleted(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::LoadConfig);
                }
                Task::none()
            }
            Message::NavigateToWorkers => {
                self.screen_history.push(self.screen.clone());
                self.screen = screen::Screen::Workers;
                self.workers_loading = true;
                if let Some(ws) = &self.workspace {
                    let ws1 = ws.clone();
                    let ws2 = ws.clone();
                    let ws3 = ws.clone();
                    Task::batch([
                        Task::perform(
                            async move {
                                list_runners(ws1)
                                    .await
                                    .map(|map| map.into_iter().collect::<Vec<_>>())
                            },
                            Message::RunnersLoaded,
                        ),
                        Task::perform(
                            async move {
                                list_actions(ws2)
                                    .await
                                    .map(|map| map.into_iter().collect::<Vec<_>>())
                            },
                            Message::ActionsLoaded,
                        ),
                        Task::perform(
                            async move { load_workers(ws3, true).await },
                            Message::WorkersLoaded,
                        ),
                    ])
                } else {
                    Task::none()
                }
            }
            Message::NavigateToInitiatives => {
                self.screen_history.push(self.screen.clone());
                self.screen = screen::Screen::Initiatives;
                self.selected_initiative = None;
                self.loading = true;
                if let Some(ws) = &self.workspace {
                    Task::perform(load_initiatives(ws.clone()), Message::InitiativesLoaded)
                } else {
                    Task::none()
                }
            }
            Message::WorkersLoaded(result) => {
                self.workers_loading = false;
                match result {
                    Ok(workers) => self.workers = workers,
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::RefreshWorkers => {
                self.workers_loading = true;
                if let Some(ws) = &self.workspace {
                    let ws1 = ws.clone();
                    let ws2 = ws.clone();
                    let ws3 = ws.clone();
                    Task::batch([
                        Task::perform(
                            async move {
                                list_runners(ws1)
                                    .await
                                    .map(|map| map.into_iter().collect::<Vec<_>>())
                            },
                            Message::RunnersLoaded,
                        ),
                        Task::perform(
                            async move {
                                list_actions(ws2)
                                    .await
                                    .map(|map| map.into_iter().collect::<Vec<_>>())
                            },
                            Message::ActionsLoaded,
                        ),
                        Task::perform(
                            async move { load_workers(ws3, true).await },
                            Message::WorkersLoaded,
                        ),
                    ])
                } else {
                    Task::none()
                }
            }
            Message::QuickStartRunner(name) => {
                if let Some(ws) = &self.workspace {
                    self.workers_loading = true;
                    Task::perform(
                        start_worker_from_runner(ws.clone(), name, true),
                        Message::WorkerStarted,
                    )
                } else {
                    Task::none()
                }
            }
            Message::OpenCustomizeRunner(name) => {
                if let Some((_, config)) = self.runners.iter().find(|(n, _)| n == &name) {
                    self.start_worker_form = StartWorkerForm {
                        from_runner: Some(name),
                        command: config.command.clone(),
                        args: config.args.join("\n"),
                        event_type: config.on.clone().unwrap_or_default(),
                        concurrency: config
                            .concurrency
                            .map(|c| c.to_string())
                            .unwrap_or("1".into()),
                        poll_cooldown: "300".into(),
                        detached: true,
                        ..Default::default()
                    };
                }
                self.screen_history.push(self.screen.clone());
                self.screen = screen::Screen::StartWorker;
                Task::none()
            }
            Message::QuickStartAction(name) => {
                if let Some(ws) = &self.workspace {
                    self.workers_loading = true;
                    Task::perform(
                        start_worker_from_action(ws.clone(), name, true),
                        Message::WorkerStarted,
                    )
                } else {
                    Task::none()
                }
            }
            Message::OpenCustomizeAction(name) => {
                if let Some((_, config)) = self.actions.iter().find(|(n, _)| n == &name) {
                    self.start_worker_form = StartWorkerForm {
                        from_runner: None,
                        command: config.command.clone(),
                        args: config.args.join("\n"),
                        event_type: config.on.clone().unwrap_or_default(),
                        concurrency: config
                            .concurrency
                            .map(|c| c.to_string())
                            .unwrap_or("1".into()),
                        poll_cooldown: "300".into(),
                        detached: true,
                        ..Default::default()
                    };
                }
                self.screen_history.push(self.screen.clone());
                self.screen = screen::Screen::StartWorker;
                Task::none()
            }
            Message::OpenStartWorker => {
                self.start_worker_form = StartWorkerForm::default();
                self.start_worker_form.concurrency = "1".into();
                self.start_worker_form.event_type = "task.next".into();
                self.start_worker_form.poll_cooldown = "300".into();
                self.start_worker_form.detached = true;
                self.screen_history.push(self.screen.clone());
                self.screen = screen::Screen::StartWorker;
                Task::none()
            }
            Message::StartWorkerCommandChanged(value) => {
                self.start_worker_form.command = value;
                Task::none()
            }
            Message::StartWorkerArgsChanged(value) => {
                self.start_worker_form.args = value;
                Task::none()
            }
            Message::StartWorkerEventChanged(value) => {
                self.start_worker_form.event_type = value;
                Task::none()
            }
            Message::StartWorkerConcurrencyChanged(value) => {
                self.start_worker_form.concurrency = value;
                Task::none()
            }
            Message::StartWorkerDetachedChanged(value) => {
                self.start_worker_form.detached = value;
                Task::none()
            }
            Message::SubmitStartWorker => {
                // Validate form
                if self.start_worker_form.command.trim().is_empty() {
                    self.start_worker_form.error = Some("Command is required".into());
                    return Task::none();
                }

                self.start_worker_form.submitting = true;
                self.start_worker_form.error = None;

                if let Some(ws) = &self.workspace {
                    let command = self.start_worker_form.command.clone();
                    let args: Vec<String> = self
                        .start_worker_form
                        .args
                        .lines()
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.to_string())
                        .collect();
                    let event_type = if self.start_worker_form.event_type.is_empty() {
                        "task.next".to_string()
                    } else {
                        self.start_worker_form.event_type.clone()
                    };
                    let concurrency = self.start_worker_form.concurrency.parse().unwrap_or(1);
                    let detached = self.start_worker_form.detached;

                    Task::perform(
                        start_worker_inline(
                            ws.clone(),
                            command,
                            args,
                            event_type,
                            concurrency,
                            detached,
                        ),
                        Message::WorkerStarted,
                    )
                } else {
                    self.start_worker_form.submitting = false;
                    self.start_worker_form.error = Some("No workspace selected".into());
                    Task::none()
                }
            }
            Message::WorkerStarted(result) => {
                self.start_worker_form.submitting = false;
                self.workers_loading = false;
                match result {
                    Ok(worker) => {
                        // Add to workers list and navigate back
                        self.workers.push(worker);
                        self.start_worker_form = StartWorkerForm::default();
                        // Navigate back to workers list
                        if let Some(previous) = self.screen_history.pop() {
                            self.screen = previous;
                        } else {
                            self.screen = screen::Screen::Workers;
                        }
                        // Refresh workers list
                        return self.update(Message::RefreshWorkers);
                    }
                    Err(e) => {
                        self.start_worker_form.error = Some(e);
                    }
                }
                Task::none()
            }
            Message::StopWorker(worker_id) => {
                if let Some(ws) = &self.workspace {
                    self.workers_loading = true;
                    Task::perform(stop_worker(ws.clone(), worker_id), Message::WorkerStopped)
                } else {
                    Task::none()
                }
            }
            Message::WorkerStopped(result) => {
                self.workers_loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    // Refresh workers list
                    return self.update(Message::RefreshWorkers);
                }
                Task::none()
            }
            Message::DeleteWorker(_worker_id) => {
                // Delete stopped/errored workers via prune
                // Note: Currently prunes all stopped workers, not just the specified one
                if let Some(ws) = &self.workspace {
                    self.workers_loading = true;
                    Task::perform(prune_workers(ws.clone()), Message::WorkerDeleted)
                } else {
                    Task::none()
                }
            }
            Message::WorkerDeleted(result) => {
                self.workers_loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    // Refresh workers list
                    return self.update(Message::RefreshWorkers);
                }
                Task::none()
            }
            Message::InitiativesLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(initiatives) => {
                        self.initiatives = initiatives;
                        self.status_message = None;
                    }
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::RefreshInitiatives => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(load_initiatives(ws.clone()), Message::InitiativesLoaded);
                }
                Task::none()
            }
            Message::SelectInitiative(initiative_id) => {
                // Find the initiative and set as selected
                if let Some(initiative) = self
                    .initiatives
                    .iter()
                    .find(|i| i.id == initiative_id)
                    .cloned()
                {
                    self.selected_initiative = Some(initiative);
                    self.initiative_summary = None;
                    self.screen_history.push(self.screen.clone());
                    self.screen = screen::Screen::InitiativeDetail {
                        id: initiative_id.clone(),
                    };
                    // Load the initiative summary
                    if let Some(ws) = &self.workspace {
                        self.loading = true;
                        return Task::perform(
                            load_initiative_summary(ws.clone(), initiative_id),
                            Message::InitiativeDetailLoaded,
                        );
                    }
                }
                Task::none()
            }
            Message::InitiativeDetailLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(summary) => {
                        self.initiative_summary = Some(summary);
                        self.status_message = None;
                    }
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::RunsLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(runs) => {
                        self.runs = runs;
                        self.status_message = None;
                    }
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::RefreshRuns => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(
                        load_runs(
                            ws.clone(),
                            self.run_worker_filter.clone(),
                            self.run_status_filter.clone(),
                        ),
                        Message::RunsLoaded,
                    );
                }
                Task::none()
            }
            Message::SelectRun(run_id) => {
                // Find and select the run from the list
                self.selected_run = self.runs.iter().find(|r| r.id == run_id).cloned();
                Task::none()
            }
            Message::FilterRunsByWorker(worker_id) => {
                self.run_worker_filter = worker_id;
                // Also clear status filter when worker filter changes to "clear all"
                if self.run_worker_filter.is_none() {
                    self.run_status_filter = None;
                }
                Task::none()
            }
            Message::FilterRunsByStatus(status) => {
                self.run_status_filter = status;
                Task::none()
            }
            Message::ReopenTask(task_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(reopen_task(ws.clone(), task_id), Message::TaskUpdated);
                }
                Task::none()
            }
            Message::ShowCreateProject => {
                self.create_project_name.clear();
                self.create_project_description.clear();
                self.create_project_owner.clear();
                self.create_project_tags.clear();
                self.create_project_desc_content = EditorContent::default();
                self.screen_history.push(self.screen.clone());
                self.screen = screen::Screen::CreateProject;
                Task::none()
            }
            Message::CreateProjectNameChanged(value) => {
                self.create_project_name = value;
                Task::none()
            }
            Message::CreateProjectDescriptionAction(action) => {
                self.create_project_desc_content.0.perform(action);
                self.create_project_description = self.create_project_desc_content.0.text();
                Task::none()
            }
            Message::CreateProjectOwnerChanged(value) => {
                self.create_project_owner = value;
                Task::none()
            }
            Message::CreateProjectTagsChanged(value) => {
                self.create_project_tags = value;
                Task::none()
            }
            Message::SubmitCreateProject => {
                if self.create_project_name.is_empty() {
                    self.status_message = Some("Project name is required".to_string());
                    return Task::none();
                }

                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    let name = self.create_project_name.clone();
                    let description = if self.create_project_description.is_empty() {
                        None
                    } else {
                        Some(self.create_project_description.clone())
                    };
                    let owner = if self.create_project_owner.is_empty() {
                        None
                    } else {
                        Some(self.create_project_owner.clone())
                    };
                    let tags = if self.create_project_tags.is_empty() {
                        None
                    } else {
                        Some(self.create_project_tags.clone())
                    };

                    return Task::perform(
                        create_project(ws.clone(), name, description, owner, tags),
                        Message::ProjectCreated,
                    );
                }
                Task::none()
            }
            Message::ProjectCreated(result) => {
                self.loading = false;
                match result {
                    Ok(()) => {
                        // Go back and refresh projects
                        if let Some(previous) = self.screen_history.pop() {
                            self.screen = previous;
                        } else {
                            self.screen = screen::Screen::Projects;
                        }
                        return self.update(Message::RefreshProjects);
                    }
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::CancelCreateProject => {
                if let Some(previous) = self.screen_history.pop() {
                    self.screen = previous;
                } else {
                    self.screen = screen::Screen::Projects;
                }
                Task::none()
            }
            Message::ShowEditProject(project_id) => {
                // Find the project and populate the form
                if let Some(project) = self.projects.iter().find(|p| p.id == project_id) {
                    let desc = project.description.clone().unwrap_or_default();
                    self.edit_project_desc_content = EditorContent::with_text(&desc);
                    self.edit_project_form = Some(screen::edit_project::EditProjectForm {
                        project_id: project.id.clone(),
                        name: project.name.clone(),
                        description: desc,
                        owner: project.owner.clone().unwrap_or_default(),
                        tags: project.tags.clone().unwrap_or_default(),
                        submitting: false,
                    });
                    self.screen_history.push(self.screen.clone());
                    self.screen = screen::Screen::EditProject { id: project_id };
                }
                Task::none()
            }
            Message::EditProjectNameChanged(value) => {
                if let Some(form) = &mut self.edit_project_form {
                    form.name = value;
                }
                Task::none()
            }
            Message::EditProjectDescriptionAction(action) => {
                self.edit_project_desc_content.0.perform(action);
                if let Some(form) = &mut self.edit_project_form {
                    form.description = self.edit_project_desc_content.0.text();
                }
                Task::none()
            }
            Message::EditProjectOwnerChanged(value) => {
                if let Some(form) = &mut self.edit_project_form {
                    form.owner = value;
                }
                Task::none()
            }
            Message::EditProjectTagsChanged(value) => {
                if let Some(form) = &mut self.edit_project_form {
                    form.tags = value;
                }
                Task::none()
            }
            Message::SubmitEditProject => {
                if let (Some(ws), Some(form)) = (&self.workspace, &mut self.edit_project_form) {
                    if form.name.is_empty() {
                        return Task::none();
                    }

                    form.submitting = true;
                    self.loading = true;

                    let project_id = form.project_id.clone();
                    let name = Some(form.name.clone());
                    let description = if form.description.is_empty() {
                        None
                    } else {
                        Some(form.description.clone())
                    };
                    let owner = if form.owner.is_empty() {
                        None
                    } else {
                        Some(form.owner.clone())
                    };
                    let tags = if form.tags.is_empty() {
                        None
                    } else {
                        Some(form.tags.clone())
                    };

                    return Task::perform(
                        update_project(
                            ws.clone(),
                            project_id,
                            name,
                            description,
                            owner,
                            None,
                            tags,
                        ),
                        Message::ProjectEdited,
                    );
                }
                Task::none()
            }
            Message::ProjectEdited(result) => {
                self.loading = false;
                if let Some(form) = &mut self.edit_project_form {
                    form.submitting = false;
                }

                match result {
                    Ok(()) => {
                        let project_id = self
                            .edit_project_form
                            .as_ref()
                            .map(|f| f.project_id.clone());
                        self.edit_project_form = None;
                        self.status_message = None;

                        // Return to project detail or projects list
                        if let Some(previous) = self.screen_history.pop() {
                            self.screen = previous;
                        } else if let Some(id) = project_id {
                            self.screen = screen::Screen::ProjectDetail { id };
                        } else {
                            self.screen = screen::Screen::Projects;
                        }

                        // Refresh projects list
                        if let Some(ws) = &self.workspace {
                            return Task::perform(
                                load_projects(ws.clone()),
                                Message::ProjectsLoaded,
                            );
                        }
                    }
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::CancelEditProject => {
                self.edit_project_form = None;
                if let Some(previous) = self.screen_history.pop() {
                    self.screen = previous;
                } else {
                    self.screen = screen::Screen::Projects;
                }
                Task::none()
            }
            Message::OpenCreateTaskScreen { project_id } => {
                self.create_task_form = Some(CreateTaskForm::new());
                self.create_task_desc_content = EditorContent::default();
                self.screen_history.push(self.screen.clone());
                // Use provided project_id if available, otherwise fall back to selected_project
                let effective_project_id = project_id.or_else(|| self.selected_project.clone());
                self.screen = screen::Screen::CreateTask {
                    project_id: effective_project_id,
                };
                Task::none()
            }
            Message::CreateTaskFormTitle(value) => {
                if let Some(form) = &mut self.create_task_form {
                    form.title = value;
                }
                Task::none()
            }
            Message::CreateTaskFormDescriptionAction(action) => {
                self.create_task_desc_content.0.perform(action);
                if let Some(form) = &mut self.create_task_form {
                    form.description = self.create_task_desc_content.0.text();
                }
                Task::none()
            }
            Message::CreateTaskFormPriority(value) => {
                if let Some(form) = &mut self.create_task_form {
                    form.priority = value;
                }
                Task::none()
            }
            Message::CreateTaskFormStatus(value) => {
                if let Some(form) = &mut self.create_task_form {
                    form.status = value;
                }
                Task::none()
            }
            Message::CreateTaskFormOwner(value) => {
                if let Some(form) = &mut self.create_task_form {
                    form.owner = value;
                }
                Task::none()
            }
            Message::CreateTaskFormDueDate(value) => {
                if let Some(form) = &mut self.create_task_form {
                    form.due_date = value;
                }
                Task::none()
            }
            Message::CreateTaskFormTags(value) => {
                if let Some(form) = &mut self.create_task_form {
                    form.tags = value;
                }
                Task::none()
            }
            Message::CreateTaskFormDependency(value) => {
                if let Some(form) = &mut self.create_task_form {
                    form.dependency_input = value;
                }
                Task::none()
            }
            Message::CreateTaskFormAddDependency => {
                if let Some(form) = &mut self.create_task_form {
                    let dep = form.dependency_input.trim().to_string();
                    if !dep.is_empty() && !form.dependencies.contains(&dep) {
                        form.dependencies.push(dep);
                        form.dependency_input.clear();
                    }
                }
                Task::none()
            }
            Message::CreateTaskFormSelectDependency(task_id) => {
                if let Some(form) = &mut self.create_task_form
                    && !form.dependencies.contains(&task_id)
                {
                    form.dependencies.push(task_id);
                    form.dependency_input.clear();
                }
                Task::none()
            }
            Message::CreateTaskFormRemoveDependency(dep_id) => {
                if let Some(form) = &mut self.create_task_form {
                    form.dependencies.retain(|d| d != &dep_id);
                }
                Task::none()
            }
            Message::CreateTaskFormSubmit => {
                // Validate form
                if let Some(form) = &mut self.create_task_form
                    && !form.validate()
                {
                    return Task::none();
                }

                if let (Some(ws), Some(project_id), Some(form)) = (
                    &self.workspace,
                    &self.selected_project,
                    &mut self.create_task_form,
                ) {
                    form.submitting = true;
                    self.loading = true;

                    let ws = ws.clone();
                    let project_id = project_id.clone();
                    let title = form.title.clone();
                    let description = if form.description.is_empty() {
                        None
                    } else {
                        Some(form.description.clone())
                    };
                    let priority = Some(form.priority.clone());
                    let status = Some(form.status.clone());
                    let owner = if form.owner.is_empty() {
                        None
                    } else {
                        Some(form.owner.clone())
                    };
                    let due_at = if form.due_date.is_empty() {
                        None
                    } else {
                        Some(form.due_date.clone())
                    };
                    let tags = if form.tags.is_empty() {
                        None
                    } else {
                        Some(form.tags.split(',').map(|s| s.trim().to_string()).collect())
                    };

                    return Task::perform(
                        async move {
                            create_task_full(
                                ws,
                                project_id,
                                title,
                                description,
                                priority,
                                status,
                                owner,
                                due_at,
                                tags,
                            )
                            .await
                        },
                        Message::CreateTaskFormCreated,
                    );
                }
                Task::none()
            }
            Message::CreateTaskFormCreated(result) => {
                self.loading = false;
                if let Some(form) = &mut self.create_task_form {
                    form.submitting = false;
                }

                match result {
                    Ok(task_output) => {
                        // Parse task ID from output (format: "Created task: <id>")
                        let task_id = task_output
                            .trim()
                            .strip_prefix("Created task: ")
                            .map(|s| s.to_string())
                            .or_else(|| {
                                // Fallback: try to extract ID from the output
                                task_output
                                    .lines()
                                    .find(|line| line.contains("task-"))
                                    .and_then(|line| {
                                        line.split_whitespace()
                                            .find(|s| s.contains("task-"))
                                            .map(|s| s.to_string())
                                    })
                            });

                        // Add dependencies if any were specified
                        if let (Some(ws), Some(form), Some(new_task_id)) =
                            (&self.workspace, &self.create_task_form, task_id)
                            && !form.dependencies.is_empty()
                        {
                            let ws = ws.clone();
                            let deps = form.dependencies.clone();
                            let task_id = new_task_id;

                            // Clear form and navigate back
                            self.create_task_form = None;
                            if let Some(previous) = self.screen_history.pop() {
                                self.screen = previous;
                            } else {
                                self.screen = screen::Screen::Tasks;
                            }

                            // Add dependencies asynchronously and then refresh
                            return Task::perform(
                                async move {
                                    for dep in deps {
                                        if let Err(e) =
                                            add_dependency(ws.clone(), task_id.clone(), dep).await
                                        {
                                            eprintln!("Failed to add dependency: {}", e);
                                        }
                                    }
                                    Ok::<(), String>(())
                                },
                                |_| Message::RefreshTasks,
                            );
                        }

                        // No dependencies - just clear form and refresh
                        self.create_task_form = None;
                        if let Some(previous) = self.screen_history.pop() {
                            self.screen = previous;
                        } else {
                            self.screen = screen::Screen::Tasks;
                        }
                        return self.update(Message::RefreshTasks);
                    }
                    Err(e) => {
                        if let Some(form) = &mut self.create_task_form {
                            form.validation_errors
                                .insert("_general".to_string(), e.clone());
                        }
                        self.status_message = Some(e);
                    }
                }
                Task::none()
            }
            Message::CreateTaskFormCancel => {
                self.create_task_form = None;
                if let Some(previous) = self.screen_history.pop() {
                    self.screen = previous;
                } else {
                    self.screen = screen::Screen::Tasks;
                }
                Task::none()
            }
            Message::OpenEditTaskScreen(task_id) => {
                // Load task data first
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(load_task(ws.clone(), task_id), Message::EditTaskLoaded);
                }
                Task::none()
            }
            Message::EditTaskLoaded(result) => {
                self.loading = false;
                match result {
                    Ok(task) => {
                        let desc = task.description.clone().unwrap_or_default();
                        self.edit_task_desc_content = EditorContent::with_text(&desc);
                        self.edit_task_form = Some(EditTaskForm::from_task(&task, Vec::new()));
                        self.screen_history.push(self.screen.clone());
                        self.screen = screen::Screen::EditTask { id: task.id };
                    }
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::EditTaskFormTitle(value) => {
                if let Some(form) = &mut self.edit_task_form {
                    form.title = value;
                }
                Task::none()
            }
            Message::EditTaskFormDescriptionAction(action) => {
                self.edit_task_desc_content.0.perform(action);
                if let Some(form) = &mut self.edit_task_form {
                    form.description = self.edit_task_desc_content.0.text();
                }
                Task::none()
            }
            Message::EditTaskFormPriority(value) => {
                if let Some(form) = &mut self.edit_task_form {
                    form.priority = value;
                }
                Task::none()
            }
            Message::EditTaskFormStatus(value) => {
                if let Some(form) = &mut self.edit_task_form {
                    form.status = value;
                }
                Task::none()
            }
            Message::EditTaskFormOwner(value) => {
                if let Some(form) = &mut self.edit_task_form {
                    form.owner = value;
                }
                Task::none()
            }
            Message::EditTaskFormDueDate(value) => {
                if let Some(form) = &mut self.edit_task_form {
                    form.due_date = value;
                }
                Task::none()
            }
            Message::EditTaskFormTags(value) => {
                if let Some(form) = &mut self.edit_task_form {
                    form.tags = value;
                }
                Task::none()
            }
            Message::EditTaskFormSubmit => {
                // Validate form first
                if let Some(form) = &mut self.edit_task_form
                    && !form.validate()
                {
                    return Task::none();
                }

                if let (Some(ws), Some(form)) = (&self.workspace, &mut self.edit_task_form) {
                    self.loading = true;
                    form.submitting = true;

                    let task_id = form.task_id.clone();
                    let title = Some(form.title.clone());
                    let description = if form.description.is_empty() {
                        None
                    } else {
                        Some(form.description.clone())
                    };
                    let priority = Some(form.priority.clone());
                    let status = Some(form.status.clone());
                    let owner = if form.owner.is_empty() {
                        None
                    } else {
                        Some(form.owner.clone())
                    };
                    let due_at = if form.due_date.is_empty() {
                        None
                    } else {
                        Some(form.due_date.clone())
                    };
                    let tags = if form.tags.is_empty() {
                        None
                    } else {
                        Some(form.tags.split(',').map(|s| s.trim().to_string()).collect())
                    };

                    return Task::perform(
                        update_task(
                            ws.clone(),
                            task_id,
                            title,
                            description,
                            priority,
                            status,
                            owner,
                            due_at,
                            tags,
                        ),
                        Message::EditTaskFormSaved,
                    );
                }
                Task::none()
            }
            Message::EditTaskFormSaved(result) => {
                self.loading = false;
                if let Some(form) = &mut self.edit_task_form {
                    form.submitting = false;
                }
                match result {
                    Ok(()) => {
                        self.edit_task_form = None;
                        if let Some(previous) = self.screen_history.pop() {
                            self.screen = previous;
                        } else {
                            self.screen = screen::Screen::Tasks;
                        }
                        return self.update(Message::RefreshTasks);
                    }
                    Err(e) => {
                        if let Some(form) = &mut self.edit_task_form {
                            form.validation_errors.insert("_general".to_string(), e);
                        }
                    }
                }
                Task::none()
            }
            Message::EditTaskFormCancel => {
                self.edit_task_form = None;
                if let Some(previous) = self.screen_history.pop() {
                    self.screen = previous;
                } else {
                    self.screen = screen::Screen::Tasks;
                }
                Task::none()
            }
            Message::CloseLogs => {
                self.log_source = None;
                self.log_lines.clear();
                // Go back to previous screen from history
                if let Some(previous) = self.screen_history.pop() {
                    self.screen = previous;
                } else {
                    self.screen = screen::Screen::Projects;
                }
                Task::none()
            }
            Message::ToggleTaskGraphView => {
                self.task_view_mode = match self.task_view_mode {
                    TaskViewMode::List => TaskViewMode::Graph,
                    TaskViewMode::Graph => TaskViewMode::List,
                };
                Task::none()
            }
            Message::TaskFilterChanged(filter) => {
                self.task_filter = filter;
                Task::none()
            }
            Message::BackToTaskList => {
                if let Some(previous) = self.screen_history.pop() {
                    self.screen = previous;
                } else {
                    self.screen = screen::Screen::Projects;
                }
                Task::none()
            }
            Message::LoadComments(task_id) => {
                if let Some(ws) = &self.workspace {
                    self.comments_loading = true;
                    let ws = ws.clone();
                    let task_id_clone = task_id.clone();
                    return Task::perform(
                        async move { load_comments(ws, task_id_clone).await },
                        Message::CommentsLoaded,
                    );
                }
                Task::none()
            }
            Message::CommentsLoaded(result) => {
                self.comments_loading = false;
                match result {
                    Ok(comments) => {
                        // Store comments keyed by task_id (get from first comment or expanded task)
                        if let Some(comment) = comments.first() {
                            self.task_comments
                                .insert(comment.parent_id.clone(), comments);
                        } else if let Some(task_id) = self.expanded_tasks.iter().next().cloned() {
                            // No comments - store empty vec for this task
                            self.task_comments.insert(task_id, Vec::new());
                        }
                    }
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            Message::CommentInputChanged(value) => {
                self.comment_input = value;
                Task::none()
            }
            Message::SubmitComment => {
                // Get the currently expanded task (should be only one when adding comments)
                if let (Some(ws), Some(task_id)) =
                    (&self.workspace, self.expanded_tasks.iter().next().cloned())
                {
                    let content = self.comment_input.trim().to_string();
                    if content.is_empty() {
                        return Task::none();
                    }
                    self.comments_loading = true;
                    let ws = ws.clone();
                    let task_id_clone = task_id.clone();
                    self.comment_input.clear();
                    return Task::perform(
                        async move { add_comment(ws, task_id_clone, content).await },
                        Message::CommentAdded,
                    );
                }
                Task::none()
            }
            Message::CommentAdded(result) => {
                self.comments_loading = false;
                match result {
                    Ok(()) => {
                        // Refresh comments for the expanded task
                        if let Some(task_id) = self.expanded_tasks.iter().next().cloned() {
                            return self.update(Message::LoadComments(task_id));
                        }
                    }
                    Err(e) => self.status_message = Some(e),
                }
                Task::none()
            }
            // ViewProjectDetail: Navigate to project detail view
            Message::ViewProjectDetail(project_id) => {
                self.selected_project = Some(project_id.clone());
                self.screen_history.push(self.screen.clone());
                self.screen = screen::Screen::ProjectDetail { id: project_id };
                self.update(Message::RefreshTasks)
            }

            // BackToProjects: Navigate back to projects list
            Message::BackToProjects => {
                self.selected_project = None;
                if let Some(previous) = self.screen_history.pop() {
                    self.screen = previous;
                } else {
                    self.screen = screen::Screen::Projects;
                }
                self.update(Message::RefreshProjects)
            }

            // ArchiveInitiative: Archive an initiative
            Message::ArchiveInitiative(initiative_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(
                        archive_initiative(ws.clone(), initiative_id),
                        Message::InitiativeUpdated,
                    );
                }
                Task::none()
            }

            // InitiativeUpdated: Handle archive result
            Message::InitiativeUpdated(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::RefreshInitiatives);
                }
                Task::none()
            }
            Message::EditTaskFormDependency(value) => {
                if let Some(form) = &mut self.edit_task_form {
                    form.dependency_input = value;
                }
                Task::none()
            }
            Message::EditTaskFormAddDependency => {
                if let Some(form) = &mut self.edit_task_form {
                    let dep = form.dependency_input.trim().to_string();
                    if !dep.is_empty() && !form.dependencies.contains(&dep) {
                        form.dependencies.push(dep);
                        form.dependency_input.clear();
                    }
                }
                Task::none()
            }
            Message::EditTaskFormSelectDependency(task_id) => {
                if let Some(form) = &mut self.edit_task_form
                    && !form.dependencies.contains(&task_id)
                {
                    form.dependencies.push(task_id);
                    form.dependency_input.clear();
                }
                Task::none()
            }
            Message::EditTaskFormRemoveDependency(dep_id) => {
                if let Some(form) = &mut self.edit_task_form {
                    form.dependencies.retain(|d| d != &dep_id);
                }
                Task::none()
            }
            // BlockTask: Open block dialog
            Message::BlockTask(task_id) => {
                // Check if we're in edit task screen - use form's dialog
                if let Some(form) = &mut self.edit_task_form
                    && form.task_id == task_id
                {
                    form.show_block_dialog = true;
                    form.block_reason.clear();
                    return Task::none();
                }
                // Otherwise use app-level dialog
                self.blocking_task_id = Some(task_id);
                self.block_reason.clear();
                Task::none()
            }

            // BlockTaskReason: Update block reason text
            Message::BlockTaskReason(reason) => {
                if let Some(form) = &mut self.edit_task_form
                    && form.show_block_dialog
                {
                    form.block_reason = reason;
                    return Task::none();
                }
                self.block_reason = reason;
                Task::none()
            }

            // BlockTaskSubmit: Submit block request
            Message::BlockTaskSubmit => {
                // Get task ID and reason from either edit form or app state
                let (task_id, reason) = if let Some(form) = &self.edit_task_form {
                    if form.show_block_dialog {
                        (Some(form.task_id.clone()), form.block_reason.clone())
                    } else {
                        (self.blocking_task_id.clone(), self.block_reason.clone())
                    }
                } else {
                    (self.blocking_task_id.clone(), self.block_reason.clone())
                };

                if let (Some(ws), Some(tid)) = (&self.workspace, task_id) {
                    self.loading = true;
                    // Clear dialog state
                    if let Some(form) = &mut self.edit_task_form {
                        form.show_block_dialog = false;
                    }
                    self.blocking_task_id = None;
                    self.block_reason.clear();
                    return Task::perform(
                        block_task(ws.clone(), tid, reason),
                        Message::TaskBlocked,
                    );
                }
                Task::none()
            }

            // BlockTaskCancelled: Close block dialog
            Message::BlockTaskCancelled => {
                if let Some(form) = &mut self.edit_task_form {
                    form.show_block_dialog = false;
                    form.block_reason.clear();
                }
                self.blocking_task_id = None;
                self.block_reason.clear();
                Task::none()
            }

            // TaskBlocked: Handle block result
            Message::TaskBlocked(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::RefreshTasks);
                }
                Task::none()
            }

            // StopRun: Stop a running process
            Message::StopRun(run_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(
                        stop_run(ws.clone(), run_id),
                        Message::RunActionCompleted,
                    );
                }
                Task::none()
            }

            // PauseRun: Pause a running process
            Message::PauseRun(run_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(
                        pause_run(ws.clone(), run_id),
                        Message::RunActionCompleted,
                    );
                }
                Task::none()
            }

            // ResumeRun: Resume a paused process
            Message::ResumeRun(run_id) => {
                if let Some(ws) = &self.workspace {
                    self.loading = true;
                    return Task::perform(
                        resume_run(ws.clone(), run_id),
                        Message::RunActionCompleted,
                    );
                }
                Task::none()
            }

            // RunActionCompleted: Handle stop/pause/resume result
            Message::RunActionCompleted(result) => {
                self.loading = false;
                if let Err(e) = result {
                    self.status_message = Some(e);
                } else {
                    return self.update(Message::RefreshRuns);
                }
                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let palette = appearance::palette();

        // SelectWorkspace is full-screen (no sidebar/header)
        if self.screen == screen::Screen::SelectWorkspace {
            return container(screen::select_workspace::view(palette))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(palette.background)),
                    ..Default::default()
                })
                .into();
        }

        // Three-panel layout for all other screens
        let sidebar = widget::sidebar::view(&self.screen, palette);
        let header = self.view_header(palette);
        let content = self.view_content(palette);

        let main_area = column![header, content]
            .width(Length::Fill)
            .height(Length::Fill);

        let layout = row![sidebar, main_area]
            .width(Length::Fill)
            .height(Length::Fill);

        let base_container = container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(palette.background)),
                ..Default::default()
            });

        // Overlay workspace dropdown when open (floats above content without layout shift)
        if self.workspace_dropdown_open {
            let workspace_state = widget::WorkspaceSelectorState {
                current_workspace: self.workspace.as_ref(),
                recent_workspaces: &self.recent_workspaces,
                dropdown_open: self.workspace_dropdown_open,
            };
            let dropdown_overlay = widget::workspace_dropdown(&workspace_state, palette);

            stack![base_container, dropdown_overlay].into()
        } else if self.blocking_task_id.is_some() {
            // Overlay block task dialog
            let block_dialog = self.view_block_dialog(palette);
            stack![base_container, block_dialog].into()
        } else {
            base_container.into()
        }
    }

    /// Renders the block task dialog overlay
    fn view_block_dialog<'a>(&'a self, palette: &'a Palette) -> Element<'a, Message> {
        use iced::Padding;
        use iced::border::{Border, Radius};
        use iced::widget::{Space, button, mouse_area, text, text_input};

        let bg = palette.card;
        let border_color = palette.border;
        let status_blocked = palette.status_blocked;

        let dialog = container(
            column![
                text("Block Task").size(18).color(palette.text),
                Space::with_height(12),
                text("Enter a reason for blocking this task:")
                    .size(14)
                    .color(palette.text_secondary),
                Space::with_height(8),
                text_input("Blocking reason...", &self.block_reason)
                    .on_input(Message::BlockTaskReason)
                    .padding(12)
                    .size(14),
                Space::with_height(16),
                row![
                    horizontal_space(),
                    widget::action_button("Cancel", Message::BlockTaskCancelled, palette),
                    Space::with_width(8),
                    button(
                        container(text("Block Task").size(14).color(iced::Color::WHITE))
                            .padding(Padding::from([10, 16]))
                    )
                    .on_press(Message::BlockTaskSubmit)
                    .style(move |_, status| {
                        let bg_color = match status {
                            button::Status::Hovered => appearance::lighten(status_blocked, 0.1),
                            _ => status_blocked,
                        };
                        button::Style {
                            background: Some(Background::Color(bg_color)),
                            border: Border {
                                color: bg_color,
                                width: 1.0,
                                radius: Radius::from(appearance::CORNER_RADIUS),
                            },
                            text_color: iced::Color::WHITE,
                            ..Default::default()
                        }
                    }),
                ]
                .align_y(iced::Alignment::Center),
            ]
            .spacing(0)
            .padding(24)
            .width(400),
        )
        .width(Length::Shrink)
        .style(move |_| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: Radius::from(appearance::CORNER_RADIUS_LARGE),
            },
            ..Default::default()
        });

        // Backdrop - captures clicks to close dialog
        let backdrop = mouse_area(
            container(Space::new(Length::Fill, Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(iced::Color::from_rgba(
                        0.0, 0.0, 0.0, 0.5,
                    ))),
                    ..Default::default()
                }),
        )
        .on_press(Message::BlockTaskCancelled);

        // Center the dialog
        let centered_dialog = container(dialog)
            .center_x(Length::Fill)
            .center_y(Length::Fill);

        // Stack backdrop behind dialog
        stack![backdrop, centered_dialog].into()
    }

    /// Renders the header bar with workspace selector and status indicators
    fn view_header<'a>(&'a self, palette: &'a Palette) -> Element<'a, Message> {
        let workspace_state = widget::WorkspaceSelectorState {
            current_workspace: self.workspace.as_ref(),
            recent_workspaces: &self.recent_workspaces,
            dropdown_open: self.workspace_dropdown_open,
        };

        let status_indicator = self.view_status_indicator(palette);

        row![
            widget::workspace_selector(workspace_state, palette),
            horizontal_space(),
            status_indicator,
        ]
        .padding(16)
        .spacing(12)
        .align_y(iced::Alignment::Center)
        .into()
    }

    /// Renders status indicators (loading, auto-refresh toggle, etc.)
    fn view_status_indicator<'a>(&'a self, palette: &'a Palette) -> Element<'a, Message> {
        use iced::widget::{button, text};

        // Show loading indicator when loading
        if self.loading || self.workers_loading || self.log_loading {
            return text("Loading...").size(12).color(palette.text_muted).into();
        }

        // Make auto-refresh status clickable
        let indicator_icon = if self.auto_refresh_enabled {
            Icon::CircleDot
        } else {
            Icon::Circle
        };
        let status_text = if self.auto_refresh_enabled {
            "Auto-refresh on"
        } else {
            "Auto-refresh off"
        };

        let btn_content = row![
            icon(indicator_icon).size(12).color(palette.text_muted),
            text(status_text).size(12),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center);

        button(btn_content)
            .style(|_theme, status| {
                let is_hovered = matches!(status, button::Status::Hovered);
                button::Style {
                    background: None,
                    text_color: if is_hovered {
                        palette.text
                    } else {
                        palette.text_muted
                    },
                    border: iced::Border::default(),
                    shadow: iced::Shadow::default(),
                }
            })
            .padding([4, 8])
            .on_press(Message::ToggleAutoRefresh)
            .into()
    }

    /// Dispatches to the appropriate screen view based on current screen
    fn view_content<'a>(&'a self, palette: &'a Palette) -> Element<'a, Message> {
        match &self.screen {
            screen::Screen::SelectWorkspace => screen::select_workspace::view(palette),

            screen::Screen::Projects => {
                let state = screen::main_screen::MainScreenState {
                    workspace: self.workspace.as_ref(),
                    projects: &self.projects,
                    tasks: &self.tasks,
                    dependencies: &self.dependencies,
                    selected_project: self.selected_project.as_ref(),
                    expanded_tasks: &self.expanded_tasks,
                    new_task_title: &self.new_task_title,
                    status_message: self.status_message.as_ref(),
                    loading: self.loading,
                    task_comments: &self.task_comments,
                    comment_input: &self.comment_input,
                    comments_loading: self.comments_loading,
                    show_back_button: false,
                };
                screen::main_screen::view(state, palette)
            }

            screen::Screen::ProjectDetail { id: _ } => {
                // Show back button if navigated from initiatives
                let show_back = self.screen_history.last().is_some_and(|s| {
                    matches!(
                        s,
                        screen::Screen::Initiatives | screen::Screen::InitiativeDetail { .. }
                    )
                });
                let state = screen::main_screen::MainScreenState {
                    workspace: self.workspace.as_ref(),
                    projects: &self.projects,
                    tasks: &self.tasks,
                    dependencies: &self.dependencies,
                    selected_project: self.selected_project.as_ref(),
                    expanded_tasks: &self.expanded_tasks,
                    new_task_title: &self.new_task_title,
                    status_message: self.status_message.as_ref(),
                    loading: self.loading,
                    task_comments: &self.task_comments,
                    comment_input: &self.comment_input,
                    comments_loading: self.comments_loading,
                    show_back_button: show_back,
                };
                screen::main_screen::view(state, palette)
            }

            screen::Screen::Tasks => {
                let project_name = self
                    .selected_project
                    .as_ref()
                    .and_then(|id| self.projects.iter().find(|p| &p.id == id))
                    .map(|p| p.name.as_str())
                    .unwrap_or("All Tasks");

                let state = screen::tasks::TasksScreenState {
                    workspace: self.workspace.as_ref(),
                    project_id: self.selected_project.as_ref(),
                    project_name,
                    tasks: &self.tasks,
                    dependencies: &self.dependencies,
                    expanded_tasks: &self.expanded_tasks,
                    filter: &self.task_filter,
                    view_mode: self.task_view_mode,
                    new_task_title: &self.new_task_title,
                    loading: self.loading,
                };
                screen::tasks::view(state, palette)
            }

            screen::Screen::TaskDetail { id: _ } => {
                // Use tasks screen for task detail
                let project_name = self
                    .selected_project
                    .as_ref()
                    .and_then(|id| self.projects.iter().find(|p| &p.id == id))
                    .map(|p| p.name.as_str())
                    .unwrap_or("All Tasks");

                let state = screen::tasks::TasksScreenState {
                    workspace: self.workspace.as_ref(),
                    project_id: self.selected_project.as_ref(),
                    project_name,
                    tasks: &self.tasks,
                    dependencies: &self.dependencies,
                    expanded_tasks: &self.expanded_tasks,
                    filter: &self.task_filter,
                    view_mode: self.task_view_mode,
                    new_task_title: &self.new_task_title,
                    loading: self.loading,
                };
                screen::tasks::view(state, palette)
            }

            screen::Screen::Initiatives => {
                let state = screen::initiatives::InitiativesScreenState {
                    initiatives: &self.initiatives,
                    loading: self.loading,
                    status_message: self.status_message.as_ref(),
                };
                screen::initiatives::view(state, palette)
            }

            screen::Screen::InitiativeDetail { id } => {
                if let Some(initiative) = self.initiatives.iter().find(|i| &i.id == id) {
                    let state = screen::initiative_detail::InitiativeDetailState {
                        initiative,
                        summary: self.initiative_summary.as_ref(),
                        tasks: &self.tasks,
                        expanded_tasks: &self.expanded_tasks,
                        loading: self.loading,
                        status_message: self.status_message.as_ref(),
                    };
                    screen::initiative_detail::view(state, palette)
                } else {
                    self.placeholder_screen("Initiative not found", palette)
                }
            }

            screen::Screen::CreateProject => {
                let state = screen::create_project::CreateProjectState {
                    name: &self.create_project_name,
                    description_content: &self.create_project_desc_content.0,
                    owner: &self.create_project_owner,
                    tags: &self.create_project_tags,
                    loading: self.loading,
                    error_message: self.status_message.as_ref(),
                };
                screen::create_project::view(state, palette)
            }

            screen::Screen::EditProject { id: _ } => {
                if let Some(form) = &self.edit_project_form {
                    let state = screen::edit_project::EditProjectState {
                        form,
                        description_content: &self.edit_project_desc_content.0,
                        loading: self.loading,
                        error_message: self.status_message.as_ref(),
                    };
                    screen::edit_project::view(state, palette)
                } else {
                    self.placeholder_screen("Loading project...", palette)
                }
            }

            screen::Screen::CreateTask { project_id } => {
                if let Some(form) = &self.create_task_form {
                    let project_name = project_id
                        .as_ref()
                        .and_then(|id| self.projects.iter().find(|p| &p.id == id))
                        .map(|p| p.name.as_str())
                        .unwrap_or("New Task");

                    let state = screen::create_task::CreateTaskScreenState {
                        project_name,
                        form,
                        available_tasks: &self.tasks,
                        description_content: &self.create_task_desc_content.0,
                    };
                    screen::create_task::view(state, palette)
                } else {
                    self.placeholder_screen("Loading task form...", palette)
                }
            }

            screen::Screen::EditTask { id } => {
                if let Some(form) = &self.edit_task_form {
                    let project_name = self
                        .tasks
                        .iter()
                        .find(|t| &t.id == id)
                        .and_then(|t| {
                            self.projects
                                .iter()
                                .find(|p| p.id == t.project_id)
                                .map(|p| p.name.as_str())
                        })
                        .unwrap_or("Edit Task");

                    let state = screen::edit_task::EditTaskScreenState {
                        project_name,
                        form,
                        available_tasks: &self.tasks,
                        loading: self.loading,
                        description_content: &self.edit_task_desc_content.0,
                    };
                    screen::edit_task::view(state, palette)
                } else {
                    self.placeholder_screen("Loading task...", palette)
                }
            }

            screen::Screen::Workers => {
                let state = screen::workers::WorkersScreenState {
                    runners: &self.runners,
                    actions: &self.actions,
                    workers: &self.workers,
                    workspace: self.workspace.as_ref(),
                    loading: self.workers_loading,
                };
                screen::workers::view(state, palette)
            }

            screen::Screen::WorkerDetail { id: _ } => {
                // Use workers screen for worker detail
                let state = screen::workers::WorkersScreenState {
                    runners: &self.runners,
                    actions: &self.actions,
                    workers: &self.workers,
                    workspace: self.workspace.as_ref(),
                    loading: self.workers_loading,
                };
                screen::workers::view(state, palette)
            }

            screen::Screen::StartWorker => {
                let state = screen::start_worker::StartWorkerFormState {
                    from_runner: self.start_worker_form.from_runner.as_deref(),
                    command: &self.start_worker_form.command,
                    args: &self.start_worker_form.args,
                    event_type: &self.start_worker_form.event_type,
                    concurrency: &self.start_worker_form.concurrency,
                    poll_cooldown: &self.start_worker_form.poll_cooldown,
                    detached: self.start_worker_form.detached,
                    env_vars: &[],
                    error: self.start_worker_form.error.as_deref(),
                    submitting: self.start_worker_form.submitting,
                };
                screen::start_worker::view(state, palette)
            }

            screen::Screen::Runs => {
                let state = screen::runs::RunsScreenState {
                    runs: &self.runs,
                    selected_run: self.selected_run.as_ref(),
                    worker_filter: self.run_worker_filter.as_ref(),
                    status_filter: self.run_status_filter.as_ref(),
                    loading: self.loading,
                };
                screen::runs::view(state, palette)
            }

            screen::Screen::RunDetail { id: _ } => {
                // Use runs screen for run detail
                let state = screen::runs::RunsScreenState {
                    runs: &self.runs,
                    selected_run: self.selected_run.as_ref(),
                    worker_filter: self.run_worker_filter.as_ref(),
                    status_filter: self.run_status_filter.as_ref(),
                    loading: self.loading,
                };
                screen::runs::view(state, palette)
            }

            screen::Screen::Logs { source } => {
                let state = screen::logs::LogsScreenState {
                    log_source: source,
                    lines: &self.log_lines,
                    follow: self.log_follow,
                    loading: self.log_loading,
                };
                screen::logs::view(state, palette)
            }

            screen::Screen::Settings => {
                let state = screen::settings::SettingsScreenState {
                    runners: &self.runners,
                    actions: &self.actions,
                    steering_files: &self.steering_files,
                    config_entries: &self.config_entries,
                    loading: self.loading,
                    runner_form: &self.runner_form,
                    action_form: &self.action_form,
                    steering_form: &self.steering_form,
                    config_form: &self.config_form,
                };
                screen::settings::view(state, palette)
            }
        }
    }

    /// Simple placeholder for screens that aren't fully implemented
    fn placeholder_screen<'a>(
        &self,
        message: &'a str,
        palette: &'a Palette,
    ) -> Element<'a, Message> {
        use iced::widget::text;
        container(text(message).size(16).color(palette.text_muted))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = Vec::new();

        // Log follow subscription: 1-second interval for log updates
        if self.log_follow && self.log_source.is_some() {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::RefreshLogs),
            );
        }

        // Auto-refresh subscription: 3-second interval for data updates
        if self.auto_refresh_enabled && !self.has_open_modal() && !self.is_editing() {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_secs(3)).map(|_| Message::AutoRefresh),
            );
        }

        // Spinner animation subscription: 100ms interval for smooth animation
        if self.needs_spinner_animation() {
            subscriptions.push(
                iced::time::every(std::time::Duration::from_millis(100))
                    .map(|_| Message::SpinnerTick),
            );
        }

        if subscriptions.is_empty() {
            Subscription::none()
        } else {
            Subscription::batch(subscriptions)
        }
    }

    /// Returns true if any modal/dropdown is currently open
    fn has_open_modal(&self) -> bool {
        self.workspace_dropdown_open
        // Add other modal states as they're implemented
    }

    /// Returns true if the user is on an editing/creation screen
    fn is_editing(&self) -> bool {
        matches!(
            self.screen,
            screen::Screen::CreateProject
                | screen::Screen::CreateTask { .. }
                | screen::Screen::EditTask { .. }
                | screen::Screen::StartWorker
        )
    }

    /// Returns true if spinner animation is needed
    ///
    /// Active when loading or there are in-progress tasks visible
    fn needs_spinner_animation(&self) -> bool {
        use granary_types::TaskStatus;

        self.loading
            || self.workers_loading
            || self.log_loading
            || self
                .tasks
                .iter()
                .any(|t| t.status_enum() == TaskStatus::InProgress)
    }
}
