use granary_types::{
    Comment, Initiative, InitiativeSummary, Project, Run, RunnerConfig, Task as GranaryTask,
    TaskPriority, TaskStatus, Worker,
};
use std::path::PathBuf;
use std::time::Instant;

use crate::screen::Screen;

/// Steering file configuration.
///
/// Represents a steering file that provides context/guidance to runners.
/// Defined locally since the main crate's SteeringFile is not in granary_types.
#[derive(Debug, Clone)]
pub struct SteeringFile {
    /// Unique identifier
    pub id: i64,
    /// Path to the steering file
    pub path: String,
    /// Mode: "reference" or "inline"
    pub mode: String,
    /// Scope type: None (global), "project", "task", or "session"
    pub scope_type: Option<String>,
    /// Scope ID (project/task/session ID when scoped)
    pub scope_id: Option<String>,
}

/// Task filter configuration for filtering the task list view.
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    /// Filter by status (None = show all)
    pub status: Option<TaskStatus>,
    /// Filter by priority (None = show all)
    pub priority: Option<TaskPriority>,
    /// Filter by owner (None = show all)
    pub owner: Option<String>,
    /// Filter by tag (None = show all)
    pub tag: Option<String>,
    /// Search text filter
    pub search: Option<String>,
}

/// Top-level application message enum.
///
/// All user interactions and async operation results flow through this enum.
/// Variants are organized by category for clarity.
#[derive(Debug, Clone)]
pub enum Message {
    // ========== Workspace messages ==========
    /// Open workspace selection dialog (alias for compatibility)
    SelectWorkspace,
    /// Workspace folder was selected (or None if cancelled)
    WorkspaceSelected(Option<PathBuf>),

    // ========== Navigation messages ==========
    /// Navigate to a specific screen
    Navigate(Screen),
    /// Go back to previous screen in history
    GoBack,
    /// Navigate to initiatives view
    NavigateToInitiatives,
    /// Navigate to workers view
    NavigateToWorkers,

    // ========== Recent workspaces ==========
    /// Select a workspace from recent list
    SelectRecentWorkspace(PathBuf),
    /// Toggle the workspace selector dropdown
    ToggleWorkspaceDropdown,

    // ========== Auto-refresh ==========
    /// Periodic auto-refresh tick
    AutoRefresh,
    /// Toggle auto-refresh on/off
    ToggleAutoRefresh,
    /// Refresh completed (with timestamp)
    RefreshComplete(Instant),

    // ========== Animation ==========
    /// Animation tick for spinners (100ms interval)
    SpinnerTick,

    // Data loading
    /// Projects finished loading from granary CLI
    ProjectsLoaded(Result<Vec<Project>, String>),
    /// Tasks finished loading from granary CLI
    TasksLoaded(Result<Vec<GranaryTask>, String>),

    // Project actions
    /// User selected a project from the list
    SelectProject(String),
    /// Refresh the projects list
    RefreshProjects,
    /// Archive a project
    ArchiveProject(String),
    /// Project archive completed
    ProjectArchived(Result<(), String>),
    /// Ready a project (convert all draft tasks to todo)
    ReadyProject(String),
    /// Project ready completed
    ProjectReadied(Result<(), String>),

    // Project navigation
    /// Navigate to project detail view
    ViewProjectDetail(String),
    /// Navigate back to projects list
    BackToProjects,
    /// Navigate to create project form
    ShowCreateProject,

    // Project unarchive
    /// Unarchive a project
    UnarchiveProject(String),
    /// Project unarchive completed
    ProjectUnarchived(Result<(), String>),

    // Project creation
    /// Create project form: name input changed
    CreateProjectNameChanged(String),
    /// Create project form: description input changed
    CreateProjectDescriptionChanged(String),
    /// Create project form: owner input changed
    CreateProjectOwnerChanged(String),
    /// Create project form: tags input changed
    CreateProjectTagsChanged(String),
    /// Submit create project form
    SubmitCreateProject,
    /// Project creation completed
    ProjectCreated(Result<(), String>),
    /// Cancel project creation and return to list
    CancelCreateProject,

    // Project editing
    /// Show the edit project form for a specific project
    ShowEditProject(String),
    /// Edit project form: name input changed
    EditProjectNameChanged(String),
    /// Edit project form: description input changed
    EditProjectDescriptionChanged(String),
    /// Edit project form: owner input changed
    EditProjectOwnerChanged(String),
    /// Edit project form: tags input changed
    EditProjectTagsChanged(String),
    /// Submit edit project form
    SubmitEditProject,
    /// Project edit completed
    ProjectEdited(Result<(), String>),
    /// Cancel project edit and return to detail
    CancelEditProject,

    // Task actions
    /// Refresh the tasks list for current project
    RefreshTasks,
    /// New task title input changed
    NewTaskTitle(String),
    /// Create a new task with current title
    CreateTask,
    /// Task creation completed
    TaskCreated(Result<(), String>),
    /// Start working on a task
    StartTask(String),
    /// Mark a task as complete
    CompleteTask(String),
    /// Task update (start/complete) finished
    TaskUpdated(Result<(), String>),
    /// Toggle task expansion to show/hide details
    ToggleTaskExpand(String),
    /// Re-open a completed/blocked task (move back to todo)
    ReopenTask(String),

    // ========== Initiative actions ==========
    /// Initiatives list finished loading
    InitiativesLoaded(Result<Vec<Initiative>, String>),
    /// User selected an initiative to view details
    SelectInitiative(String),
    /// Initiative detail (with summary) finished loading
    InitiativeDetailLoaded(Result<InitiativeSummary, String>),
    /// Archive an initiative
    ArchiveInitiative(String),
    /// Initiative update (archive) completed
    InitiativeUpdated(Result<(), String>),
    /// Refresh initiatives list
    RefreshInitiatives,

    // Task form state (for create screen)
    /// Create task form: title input changed
    CreateTaskFormTitle(String),
    /// Create task form: description input changed
    CreateTaskFormDescription(String),
    /// Create task form: priority changed
    CreateTaskFormPriority(TaskPriority),
    /// Create task form: status changed
    CreateTaskFormStatus(TaskStatus),
    /// Create task form: owner input changed
    CreateTaskFormOwner(String),
    /// Create task form: due date input changed
    CreateTaskFormDueDate(String),
    /// Create task form: tags input changed
    CreateTaskFormTags(String),
    /// Create task form: dependency input changed
    CreateTaskFormDependency(String),
    /// Create task form: add dependency from input
    CreateTaskFormAddDependency,
    /// Create task form: select dependency from search results
    CreateTaskFormSelectDependency(String),
    /// Create task form: remove a dependency
    CreateTaskFormRemoveDependency(String),
    /// Create task form: submit the form
    CreateTaskFormSubmit,
    /// Create task form: task created successfully (returns task_id for dependency handling)
    CreateTaskFormCreated(Result<String, String>),
    /// Create task form: cancel and return
    CreateTaskFormCancel,

    // Edit task variants
    /// Task data loaded for editing
    EditTaskLoaded(Result<GranaryTask, String>),
    /// Edit task form: title input changed
    EditTaskFormTitle(String),
    /// Edit task form: description input changed
    EditTaskFormDescription(String),
    /// Edit task form: priority changed
    EditTaskFormPriority(TaskPriority),
    /// Edit task form: status changed
    EditTaskFormStatus(TaskStatus),
    /// Edit task form: owner input changed
    EditTaskFormOwner(String),
    /// Edit task form: due date input changed
    EditTaskFormDueDate(String),
    /// Edit task form: tags input changed
    EditTaskFormTags(String),
    /// Edit task form: dependency input changed
    EditTaskFormDependency(String),
    /// Edit task form: add dependency from input
    EditTaskFormAddDependency,
    /// Edit task form: select dependency from search results
    EditTaskFormSelectDependency(String),
    /// Edit task form: remove a dependency
    EditTaskFormRemoveDependency(String),
    /// Edit task form: submit the form
    EditTaskFormSubmit,
    /// Edit task form: cancel and return
    EditTaskFormCancel,
    /// Edit task form: save completed
    EditTaskFormSaved(Result<(), String>),

    // Task blocking
    /// Block a task (opens blocking dialog)
    BlockTask(String),
    /// Block task dialog: reason input changed
    BlockTaskReason(String),
    /// Block task dialog: submit block request
    BlockTaskSubmit,
    /// Block task dialog: cancelled
    BlockTaskCancelled,
    /// Task block operation completed
    TaskBlocked(Result<(), String>),

    // View toggles
    /// Toggle between task list and graph view
    ToggleTaskGraphView,
    /// Task filter changed
    TaskFilterChanged(TaskFilter),

    // Task navigation
    /// Open create task screen, optionally pre-selecting a project
    OpenCreateTaskScreen { project_id: Option<String> },
    /// Open edit task screen for a specific task
    OpenEditTaskScreen(String),
    /// Navigate back to task list
    BackToTaskList,

    // Log viewing
    /// Open logs for a worker
    OpenWorkerLogs(String),
    /// Open logs for a run
    OpenRunLogs(String),
    /// Initial log content loaded
    LogsLoaded(Result<Vec<String>, String>),
    /// New log lines appended (for streaming)
    LogsAppended(Result<Vec<String>, String>),
    /// Toggle follow mode on/off
    ToggleLogFollow(bool),
    /// Close logs view and return to main screen
    CloseLogs,
    /// Clear log display (visual only)
    ClearLogs,
    /// Refresh logs (triggered by subscription polling)
    RefreshLogs,

    // Run actions
    /// Runs finished loading from granary CLI
    RunsLoaded(Result<Vec<Run>, String>),
    /// User selected a run from the list
    SelectRun(String),
    /// Refresh the runs list
    RefreshRuns,
    /// Stop a running process
    StopRun(String),
    /// Pause a running process
    PauseRun(String),
    /// Resume a paused process
    ResumeRun(String),
    /// Run action (stop/pause/resume) completed
    RunActionCompleted(Result<(), String>),
    /// Filter runs by worker
    FilterRunsByWorker(Option<String>),
    /// Filter runs by status
    FilterRunsByStatus(Option<String>),

    // Settings - Navigation
    /// Open settings screen
    OpenSettings,
    /// Close settings and return to previous screen
    CloseSettings,

    // Settings - Runners
    /// Load runners list from global config
    LoadRunners,
    /// Runners loaded from CLI
    RunnersLoaded(Result<Vec<(String, RunnerConfig)>, String>),
    /// Add new runner form field changed
    RunnerFormChanged { field: String, value: String },
    /// Edit an existing runner (populate form with runner data)
    EditRunner(String),
    /// Cancel editing and reset form
    CancelEditRunner,
    /// Save new/edited runner
    SaveRunner,
    /// Runner saved result
    RunnerSaved(Result<(), String>),
    /// Delete a runner by name
    DeleteRunner(String),
    /// Runner deleted result
    RunnerDeleted(Result<(), String>),

    // Settings - Steering
    /// Load steering files list
    LoadSteering,
    /// Steering files loaded
    SteeringLoaded(Result<Vec<SteeringFile>, String>),
    /// Add steering file
    AddSteering {
        path: String,
        mode: String,
        project: Option<String>,
    },
    /// Steering added result
    SteeringAdded(Result<(), String>),
    /// Remove steering file by path
    RemoveSteering(String),
    /// Steering removed result
    SteeringRemoved(Result<(), String>),

    // Settings - Config
    /// Load config key-value pairs
    LoadConfig,
    /// Config loaded
    ConfigLoaded(Result<Vec<(String, String)>, String>),
    /// Set config value
    SetConfig { key: String, value: String },
    /// Config set result
    ConfigSet(Result<(), String>),
    /// Delete config key
    DeleteConfig(String),
    /// Config deleted result
    ConfigDeleted(Result<(), String>),

    // ========== Worker management ==========
    /// Workers list loaded from granary CLI
    WorkersLoaded(Result<Vec<Worker>, String>),
    /// Refresh the workers list
    RefreshWorkers,

    // Runner quick-start actions
    /// Quick-start a runner with default settings
    QuickStartRunner(String),
    /// Open customize form pre-filled with runner settings
    OpenCustomizeRunner(String),

    // Start worker form
    /// Open blank start worker form
    OpenStartWorker,
    /// Form field: command changed
    StartWorkerCommandChanged(String),
    /// Form field: args changed (multiline, will split on newlines)
    StartWorkerArgsChanged(String),
    /// Form field: event type changed
    StartWorkerEventChanged(String),
    /// Form field: concurrency changed (parse to u32)
    StartWorkerConcurrencyChanged(String),
    /// Form field: detached toggle changed
    StartWorkerDetachedChanged(bool),
    /// Submit start worker form
    SubmitStartWorker,
    /// Worker start completed
    WorkerStarted(Result<Worker, String>),

    // Active worker actions
    /// Stop a running worker
    StopWorker(String),
    /// Worker stop completed
    WorkerStopped(Result<(), String>),
    /// Delete a stopped/errored worker (via prune)
    DeleteWorker(String),
    /// Worker deletion completed
    WorkerDeleted(Result<(), String>),

    // ========== Comment actions ==========
    /// Load comments for a task
    LoadComments(String),
    /// Comments loaded for a task
    CommentsLoaded(Result<Vec<Comment>, String>),
    /// New comment text input changed
    CommentInputChanged(String),
    /// Submit a new comment for the current expanded task
    SubmitComment,
    /// Comment was added successfully
    CommentAdded(Result<(), String>),
}
