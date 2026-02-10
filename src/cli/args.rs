use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Granary - A CLI context hub for agentic work
#[derive(Parser)]
#[command(name = "granary")]
#[command(author, version = crate::cli::update::version_with_update_notice(), about, long_about = None)]
#[command(after_help = "\
AGENTS (AI/LLM):
  Plan a feature:
    granary plan \"Feature name\"

  Plan multi-project work:
    granary initiate \"Initiative name\"

  Work on a task:
    granary work start <task-id>

  Search projects and tasks:
    granary search \"keyword\"

  For workflow guidance, run: granary")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Output format (table, json, yaml, md, prompt)
    #[arg(long, short = 'f', global = true, value_enum)]
    pub format: Option<CliOutputFormat>,

    /// Shorthand for --format=json
    #[arg(long, global = true, conflicts_with_all = ["prompt", "text"])]
    pub json: bool,

    /// Shorthand for --format=prompt (LLM-optimized output)
    #[arg(long, global = true, conflicts_with_all = ["json", "text"])]
    pub prompt: bool,

    /// Shorthand for --format=table (text output)
    #[arg(long, global = true, conflicts_with_all = ["json", "prompt"])]
    pub text: bool,

    /// Workspace path override
    #[arg(long, global = true, env = "GRANARY_HOME")]
    pub workspace: Option<PathBuf>,

    /// Session ID override
    #[arg(long, global = true, env = "GRANARY_SESSION")]
    pub session: Option<String>,

    /// Watch mode - continuously poll and update output (works with: tasks, projects, workers, runs, sessions, initiatives, search, summary)
    #[arg(long, global = true)]
    pub watch: bool,

    /// Polling interval in seconds for watch mode
    #[arg(long, global = true, default_value = "2", value_name = "SECONDS")]
    pub interval: u64,
}

impl Cli {
    /// Returns Some(format) if user explicitly specified via --format flag or shorthand flags,
    /// None to use command default. All commands use this to respect explicit user overrides
    /// while allowing command-specific defaults via the Output trait.
    pub fn output_format_override(&self) -> Option<CliOutputFormat> {
        if self.json {
            Some(CliOutputFormat::Json)
        } else if self.prompt {
            Some(CliOutputFormat::Prompt)
        } else if self.text {
            Some(CliOutputFormat::Table)
        } else {
            self.format
        }
    }
}

#[derive(Clone, Copy, Default, ValueEnum)]
pub enum CliOutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
    Md,
    Prompt,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new workspace (alias for `workspace init`)
    Init {
        /// Create a local .granary/ directory instead of a named workspace
        #[arg(long)]
        local: bool,

        /// Force initialization even if workspace already exists
        #[arg(long)]
        force: bool,

        /// Skip git root directory check
        #[arg(long)]
        skip_git_check: bool,
    },

    /// Manage workspaces
    Workspace {
        #[command(subcommand)]
        action: Option<WorkspaceAction>,
    },

    /// List all workspaces (alias for `workspace list`)
    Workspaces,

    /// Check workspace health
    Doctor {
        /// Automatically fix issues (e.g. add missing granary instructions to agent files)
        #[arg(long)]
        fix: bool,
    },

    /// Plan a new feature - creates project and guides task creation
    #[command(
        arg_required_else_help = true,
        after_help = "EXAMPLES:\n    granary plan \"Add Instagram OAuth2 provider\"\n    granary plan --project existing-project-abc1"
    )]
    Plan {
        /// Feature/project name (creates a new project)
        #[arg(conflicts_with = "project")]
        name: Option<String>,

        /// Plan an existing project (for initiative sub-projects)
        #[arg(long, conflicts_with = "name")]
        project: Option<String>,
    },

    /// Work on a task - claims and provides full context
    #[command(
        after_help = "EXAMPLES:\n    granary work start my-project-abc1-task-1\n    granary work done my-project-abc1-task-1 \"Implemented feature\"\n    granary work block my-project-abc1-task-1 \"Waiting for API credentials\"\n    granary work release my-project-abc1-task-1"
    )]
    Work {
        #[command(subcommand)]
        command: WorkCommand,
    },

    /// Show any entity by ID (auto-detects type from ID pattern)
    #[command(
        after_help = "EXAMPLES:\n    granary show my-project-abc1           # Show a project\n    granary show my-project-abc1-task-1    # Show a task\n    granary show sess-20260112-xyz1        # Show a session\n    granary show chkpt-abc123              # Show a checkpoint\n\nID PATTERNS:\n    project:    <name>-<4chars>              e.g., my-project-abc1\n    task:       <project-id>-task-<n>        e.g., my-project-abc1-task-1\n    session:    sess-<date>-<4chars>         e.g., sess-20260112-xyz1\n    checkpoint: chkpt-<6chars>               e.g., chkpt-abc123\n    comment:    <task-id>-comment-<n>        e.g., my-proj-abc1-task-1-comment-1\n    artifact:   <task-id>-artifact-<n>       e.g., my-proj-abc1-task-1-artifact-1"
    )]
    Show {
        /// Entity ID (auto-detected: project, task, session, checkpoint, comment, artifact)
        id: String,
    },

    /// List all projects or create a new one
    #[command(
        after_help = "AGENTS: To plan a new project with guided task creation, use:\n    granary plan \"Project name\""
    )]
    Projects {
        #[command(subcommand)]
        action: Option<ProjectsAction>,

        /// Include archived projects (for list)
        #[arg(long)]
        all: bool,
    },

    /// Work with a specific project or create a new one
    #[command(
        after_help = "AGENTS: To plan a new project with guided task creation, use:\n    granary plan \"Project name\""
    )]
    Project {
        /// Project ID (or "create" to create a new project)
        id: String,

        #[command(subcommand)]
        action: Option<ProjectAction>,
    },

    /// List tasks
    #[command(
        after_help = "AGENTS: To work on a task with full context, use:\n    granary work start <task-id>"
    )]
    Tasks {
        /// Show all tasks (across all projects)
        #[arg(long)]
        all: bool,

        /// Filter by status
        #[arg(long)]
        status: Option<String>,

        /// Filter by priority
        #[arg(long)]
        priority: Option<String>,

        /// Filter by owner
        #[arg(long)]
        owner: Option<String>,
    },

    /// Work with a specific task
    #[command(
        after_help = "AGENTS: To work on this task with full context and steering, use:\n    granary work start <task-id>"
    )]
    Task {
        /// Task ID
        id: String,

        #[command(subcommand)]
        action: Option<TaskAction>,
    },

    /// Get the next actionable task
    Next {
        /// Include reason for selection
        #[arg(long)]
        include_reason: bool,

        /// Show all currently available tasks
        #[arg(long)]
        all: bool,
    },

    /// Start a task (alias for task <id> start)
    #[command(
        after_help = "AGENTS: For full task context with steering files, use:\n    granary work start <task-id>"
    )]
    Start {
        /// Task ID
        task_id: String,

        /// Owner
        #[arg(long)]
        owner: Option<String>,

        /// Lease duration in minutes
        #[arg(long)]
        lease: Option<u32>,
    },

    /// Set focus to a task
    Focus {
        /// Task ID
        task_id: String,
    },

    /// Pin a task for attention
    Pin {
        /// Task ID
        task_id: String,
    },

    /// Unpin a task
    Unpin {
        /// Task ID
        task_id: String,
    },

    /// List sessions
    Sessions {
        /// Include closed sessions
        #[arg(long)]
        all: bool,
    },

    /// Session management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },

    /// Generate summary of current work
    Summary {
        /// Approximate token budget
        #[arg(long)]
        token_budget: Option<usize>,
    },

    /// Export context pack for LLM consumption
    Context {
        /// What to include (comma-separated: projects,tasks,comments,decisions,blockers,artifacts)
        #[arg(long)]
        include: Option<String>,

        /// Maximum items per category
        #[arg(long)]
        max_items: Option<usize>,
    },

    /// Checkpoint management
    Checkpoint {
        #[command(subcommand)]
        action: CheckpointAction,
    },

    /// Generate handoff document for agent delegation
    Handoff {
        /// Target agent or role
        #[arg(long)]
        to: String,

        /// Task IDs (comma-separated)
        #[arg(long)]
        tasks: String,

        /// Constraints for the agent
        #[arg(long)]
        constraints: Option<String>,

        /// Acceptance criteria
        #[arg(long)]
        acceptance_criteria: Option<String>,
    },

    /// Apply a batch of operations from JSON
    Apply {
        /// Read from stdin
        #[arg(long)]
        stdin: bool,
    },

    /// Process a batch of operations from JSONL
    Batch {
        /// Read from stdin
        #[arg(long)]
        stdin: bool,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Steering file management
    Steering {
        #[command(subcommand)]
        action: SteeringAction,
    },

    /// Search projects and tasks by title
    #[command(after_help = "EXAMPLE:\n    granary search \"oauth\"")]
    Search {
        /// Search query
        query: String,
    },

    /// List all initiatives or create a new one
    #[command(
        after_help = "AGENTS: To plan a multi-project initiative, use:\n    granary initiate \"Initiative name\""
    )]
    Initiatives {
        #[command(subcommand)]
        action: Option<InitiativesAction>,

        /// Include archived initiatives (for list)
        #[arg(long)]
        all: bool,
    },

    /// Work with a specific initiative
    #[command(after_help = "EXAMPLE:\n    granary initiative user-auth-abc1 projects")]
    Initiative {
        /// Initiative ID
        id: String,

        #[command(subcommand)]
        action: Option<InitiativeAction>,
    },

    /// Start planning a multi-project initiative (agent-friendly)
    #[command(after_help = "EXAMPLE:\n    granary initiate \"User authentication system\"")]
    Initiate {
        /// Initiative name
        name: String,

        /// Optional description
        #[arg(long)]
        description: Option<String>,
    },

    /// Update granary to the latest version
    Update {
        /// Check for updates without installing
        #[arg(long)]
        check: bool,

        /// Install a specific version (e.g., 0.6.2-pre.1)
        #[arg(long)]
        to: Option<String>,
    },

    /// List all workers
    Workers {
        /// Include stopped/errored workers
        #[arg(long)]
        all: bool,
    },

    /// Manage a specific worker
    Worker {
        #[command(subcommand)]
        command: WorkerCommand,
    },

    /// List all runs
    Runs {
        /// Filter by worker ID
        #[arg(long)]
        worker: Option<String>,

        /// Filter by status (pending, running, completed, failed, paused, cancelled)
        #[arg(long)]
        status: Option<String>,

        /// Include completed/failed/cancelled runs
        #[arg(long)]
        all: bool,

        /// Maximum number of runs to show
        #[arg(long, default_value = "50")]
        limit: u32,
    },

    /// Manage a specific run
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },

    /// List and manage events
    #[command(
        after_help = "EXAMPLES:\n    granary events                            # List recent events\n    granary events --type task.created --since 1h  # Filter by type and time\n    granary events --watch                    # Tail events\n    granary events drain --before 7d          # Drain old events"
    )]
    Events {
        #[command(subcommand)]
        action: Option<EventsAction>,

        /// Filter by event type (e.g., task.created, project.updated)
        #[arg(long = "type")]
        event_type: Option<String>,

        /// Filter by entity type (e.g., task, project)
        #[arg(long)]
        entity: Option<String>,

        /// Show events since duration (1h, 7d, 30m) or ISO timestamp
        #[arg(long)]
        since: Option<String>,

        /// Maximum number of events to show
        #[arg(long, default_value = "50")]
        limit: u32,
    },

    /// Manage the granary daemon
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}

#[derive(Subcommand)]
pub enum WorkCommand {
    /// Start working on a task (claims it and outputs context)
    #[command(
        after_help = "EXAMPLE:\n    granary work start my-project-abc1-task-1 --owner \"Opus 4.5\""
    )]
    Start {
        /// Task ID
        task_id: String,

        /// Owner name (e.g., "Opus 4.5 Worker 83")
        #[arg(long)]
        owner: Option<String>,
    },

    /// Mark task as done
    #[command(
        after_help = "EXAMPLE:\n    granary work done my-project-abc1-task-1 \"Implemented OAuth2 token exchange\""
    )]
    Done {
        /// Task ID
        task_id: String,

        /// Summary of changes
        summary: String,
    },

    /// Block task with reason
    #[command(
        after_help = "EXAMPLE:\n    granary work block my-project-abc1-task-1 \"Waiting for API credentials\""
    )]
    Block {
        /// Task ID
        task_id: String,

        /// Reason for blocking
        reason: String,
    },

    /// Release task (give up claim)
    #[command(after_help = "EXAMPLE:\n    granary work release my-project-abc1-task-1")]
    Release {
        /// Task ID
        task_id: String,
    },
}

#[derive(Subcommand)]
pub enum ProjectsAction {
    /// Create a new project
    #[command(
        after_help = "AGENTS: For guided project planning with task creation, use:\n    granary plan \"Project name\""
    )]
    Create {
        /// Project name
        name: String,

        /// Project description
        #[arg(long)]
        description: Option<String>,

        /// Project owner
        #[arg(long)]
        owner: Option<String>,

        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// Update project
    Update {
        /// New name
        #[arg(long)]
        name: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New owner
        #[arg(long)]
        owner: Option<String>,

        /// Tags to add (+tag) or remove (-tag)
        #[arg(long)]
        tags: Option<String>,
    },

    /// Archive project
    Archive,

    /// List or create tasks in project
    Tasks {
        #[command(subcommand)]
        action: Option<ProjectTasksAction>,
    },

    /// Manage project dependencies
    Deps {
        #[command(subcommand)]
        action: ProjectDepsAction,
    },

    /// Show project summary
    Summary,

    /// Mark project as ready for work (planning complete)
    Ready,

    /// Manage project-attached steering files
    Steer {
        #[command(subcommand)]
        action: ProjectSteerAction,
    },
}

#[derive(Subcommand)]
pub enum ProjectSteerAction {
    /// Add a steering file to this project
    Add {
        /// File path
        path: String,

        /// Mode (always, on-demand)
        #[arg(long, default_value = "always")]
        mode: String,
    },

    /// Remove a steering file from this project
    Rm {
        /// File path
        path: String,
    },

    /// List steering files for this project
    List,
}

#[derive(Subcommand)]
pub enum ProjectDepsAction {
    /// Add a dependency (this project depends on another)
    Add {
        /// Project ID to depend on
        depends_on_id: String,
    },

    /// Remove a dependency
    Rm {
        /// Project ID to remove from dependencies
        depends_on_id: String,
    },

    /// List all dependencies
    List,

    /// Show dependency graph
    Graph,
}

#[derive(Subcommand)]
pub enum ProjectTasksAction {
    /// Create a new task
    #[command(
        after_help = "EXAMPLE:\n    granary project my-proj-abc1 tasks create \"Implement OAuth\" --description \"Add OAuth2 flow\""
    )]
    Create {
        /// Task title
        title: String,

        /// Task description
        #[arg(long)]
        description: Option<String>,

        /// Priority (P0-P4)
        #[arg(long, default_value = "P2")]
        priority: String,

        /// Status (draft, todo)
        #[arg(long, default_value = "draft")]
        status: String,

        /// Owner
        #[arg(long)]
        owner: Option<String>,

        /// Dependencies (comma-separated task IDs)
        #[arg(long)]
        dependencies: Option<String>,

        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,

        /// Due date (ISO 8601)
        #[arg(long)]
        due: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum TaskAction {
    /// Update task
    Update {
        /// New title
        #[arg(long)]
        title: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New status (draft, todo, in_progress, done, blocked)
        #[arg(long)]
        status: Option<String>,

        /// New priority (P0-P4)
        #[arg(long)]
        priority: Option<String>,

        /// New owner
        #[arg(long)]
        owner: Option<String>,

        /// Tags
        #[arg(long)]
        tags: Option<String>,

        /// Due date
        #[arg(long)]
        due: Option<String>,
    },

    /// Mark a draft task as ready (transition Draft -> Todo)
    Ready,

    /// Start working on task
    #[command(
        after_help = "AGENTS: For full task context with steering files, use:\n    granary work start <task-id>"
    )]
    Start {
        /// Owner
        #[arg(long)]
        owner: Option<String>,

        /// Lease duration in minutes
        #[arg(long)]
        lease: Option<u32>,
    },

    /// Mark task as done
    Done {
        /// Completion comment
        #[arg(long)]
        comment: Option<String>,
    },

    /// Block task
    Block {
        /// Reason for blocking
        #[arg(long)]
        reason: String,
    },

    /// Unblock task
    Unblock,

    /// Claim task with a lease
    Claim {
        /// Owner
        #[arg(long)]
        owner: String,

        /// Lease duration in minutes
        #[arg(long)]
        lease: Option<u32>,
    },

    /// Extend lease (heartbeat)
    Heartbeat {
        /// New lease duration in minutes
        #[arg(long, default_value = "30")]
        lease: u32,
    },

    /// Release claim on task
    Release,

    /// Manage dependencies
    Deps {
        #[command(subcommand)]
        action: DepsAction,
    },

    /// List or create subtasks
    Tasks {
        #[command(subcommand)]
        action: Option<SubtaskAction>,
    },

    /// List or create comments
    Comments {
        #[command(subcommand)]
        action: Option<CommentAction>,
    },

    /// List or manage artifacts
    Artifacts {
        #[command(subcommand)]
        action: Option<ArtifactAction>,
    },
}

#[derive(Subcommand)]
pub enum DepsAction {
    /// Add dependencies
    Add {
        /// Task IDs to depend on (space-separated)
        task_ids: Vec<String>,
    },

    /// Remove a dependency
    Rm {
        /// Task ID to remove from dependencies
        task_id: String,
    },

    /// Show dependency graph
    Graph,
}

#[derive(Subcommand)]
pub enum SubtaskAction {
    /// Create a subtask
    Create {
        /// Subtask title
        title: String,

        /// Description
        #[arg(long)]
        description: Option<String>,

        /// Priority
        #[arg(long, default_value = "P2")]
        priority: String,

        /// Owner
        #[arg(long)]
        owner: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum CommentAction {
    /// Create a comment
    Create {
        /// Comment content (positional argument)
        content_positional: Option<String>,

        /// Comment content (flag form, alternative to positional)
        #[arg(long = "content")]
        content_flag: Option<String>,

        /// Comment kind (note, progress, decision, blocker, handoff, incident, context)
        #[arg(long, default_value = "note")]
        kind: String,

        /// Author
        #[arg(long)]
        author: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ArtifactAction {
    /// Add a file artifact
    Add {
        /// Artifact type (file, url, git_ref, log)
        artifact_type: String,

        /// Path or URL
        path: String,

        /// Description
        #[arg(long)]
        description: Option<String>,
    },

    /// Remove an artifact
    Rm {
        /// Artifact ID
        artifact_id: String,
    },
}

#[derive(Subcommand)]
pub enum SessionAction {
    /// Start a new session
    Start {
        /// Session name
        name: String,

        /// Session owner
        #[arg(long)]
        owner: Option<String>,

        /// Session mode (plan, execute, review)
        #[arg(long, default_value = "execute")]
        mode: String,
    },

    /// Show current session
    Current,

    /// Switch to a session
    Use {
        /// Session ID
        session_id: String,
    },

    /// Close current or specified session
    Close {
        /// Session ID (uses current if not specified)
        session_id: Option<String>,

        /// Closing summary
        #[arg(long)]
        summary: Option<String>,
    },

    /// Add item to session scope (auto-detects type from ID if not specified)
    #[command(
        after_help = "EXAMPLES:\n    granary session add my-project-abc1              # Auto-detect as project\n    granary session add my-project-abc1-task-1      # Auto-detect as task\n    granary session add project my-project-abc1     # Explicit type (backward compat)"
    )]
    Add {
        /// Arguments: either just <id> (auto-detect type) or <type> <id> (explicit type)
        #[arg(num_args = 1..=2)]
        args: Vec<String>,
    },

    /// Remove item from session scope
    Rm {
        /// Item type
        item_type: String,

        /// Item ID
        item_id: String,
    },

    /// Print environment variables for shell export
    Env,
}

#[derive(Subcommand)]
pub enum CheckpointAction {
    /// Create a checkpoint
    Create {
        /// Checkpoint name
        name: String,
    },

    /// List checkpoints
    List,

    /// Compare two checkpoints
    Diff {
        /// From checkpoint (name or "now")
        from: String,

        /// To checkpoint (name or "now")
        to: String,
    },

    /// Restore from a checkpoint
    Restore {
        /// Checkpoint name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Get a config value
    Get {
        /// Config key
        key: String,
    },

    /// Set a config value
    Set {
        /// Config key
        key: String,

        /// Config value
        value: String,
    },

    /// List all config values
    List,

    /// Delete a config value
    Delete {
        /// Config key
        key: String,
    },

    /// Open global config file (~/.granary/config.toml) in $EDITOR
    Edit,

    /// Manage global runners configuration
    Runners {
        #[command(subcommand)]
        action: Option<RunnersAction>,
    },
}

#[derive(Subcommand)]
pub enum RunnersAction {
    /// Add or update a runner configuration
    Add {
        /// Runner name
        name: String,

        /// Command to execute
        #[arg(long)]
        command: String,

        /// Arguments (can be specified multiple times)
        #[arg(long = "arg", short = 'a')]
        args: Vec<String>,

        /// Maximum concurrent executions
        #[arg(long)]
        concurrency: Option<u32>,

        /// Default event type this runner responds to
        #[arg(long)]
        on: Option<String>,

        /// Environment variables (KEY=VALUE format, can be specified multiple times)
        #[arg(long = "env", short = 'e')]
        env_vars: Vec<String>,
    },

    /// Update an existing runner
    Update {
        /// Runner name
        name: String,

        /// New command to execute
        #[arg(long)]
        command: Option<String>,

        /// Arguments (replaces existing if provided)
        #[arg(long = "arg", short = 'a')]
        args: Option<Vec<String>>,

        /// Maximum concurrent executions
        #[arg(long)]
        concurrency: Option<u32>,

        /// Default event type this runner responds to
        #[arg(long)]
        on: Option<String>,

        /// Environment variables (KEY=VALUE format, replaces existing if provided)
        #[arg(long = "env", short = 'e')]
        env_vars: Option<Vec<String>>,
    },

    /// Remove a runner configuration
    Rm {
        /// Runner name
        name: String,
    },

    /// Show a specific runner configuration
    Show {
        /// Runner name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum SteeringAction {
    /// List steering files
    List,

    /// Add a steering file
    Add {
        /// File path
        path: String,

        /// Mode (always, on-demand)
        #[arg(long, default_value = "always")]
        mode: String,

        /// Attach to a project (only included when project is in context)
        #[arg(long, conflicts_with_all = ["task", "for_session"])]
        project: Option<String>,

        /// Attach to a task (only included in handoffs for that task)
        #[arg(long, conflicts_with_all = ["project", "for_session"])]
        task: Option<String>,

        /// Attach to current session (auto-deleted on session close)
        #[arg(long = "for-session", conflicts_with_all = ["project", "task"])]
        for_session: bool,
    },

    /// Remove a steering file
    Rm {
        /// File path
        path: String,

        /// Remove from a specific project
        #[arg(long, conflicts_with_all = ["task", "for_session"])]
        project: Option<String>,

        /// Remove from a specific task
        #[arg(long, conflicts_with_all = ["project", "for_session"])]
        task: Option<String>,

        /// Remove from current session
        #[arg(long = "for-session", conflicts_with_all = ["project", "task"])]
        for_session: bool,
    },
}

#[derive(Subcommand)]
pub enum InitiativesAction {
    /// Create a new initiative
    #[command(
        after_help = "AGENTS: For guided initiative planning with project creation, use:\n    granary initiate \"Initiative name\""
    )]
    Create {
        /// Initiative name
        name: String,

        /// Initiative description
        #[arg(long)]
        description: Option<String>,

        /// Initiative owner
        #[arg(long)]
        owner: Option<String>,

        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum InitiativeAction {
    /// Update initiative
    Update {
        /// New name
        #[arg(long)]
        name: Option<String>,

        /// New description
        #[arg(long)]
        description: Option<String>,

        /// New owner
        #[arg(long)]
        owner: Option<String>,

        /// Tags (comma-separated)
        #[arg(long)]
        tags: Option<String>,
    },

    /// Archive initiative
    Archive,

    /// List projects in initiative
    Projects,

    /// Add project to initiative
    AddProject {
        /// Project ID
        project_id: String,
    },

    /// Remove project from initiative
    RemoveProject {
        /// Project ID
        project_id: String,
    },

    /// Show dependency graph between projects in this initiative (Mermaid output)
    Graph,

    /// Get the next actionable task(s) across this initiative.
    /// Returns tasks that are unblocked at both the project and task level.
    Next {
        /// Return all actionable tasks instead of just the next one
        #[arg(long)]
        all: bool,
    },

    /// Show a high-level summary of the initiative.
    /// Includes progress, blockers, and next actions.
    Summary,
}

#[derive(Subcommand)]
pub enum WorkerCommand {
    /// Start a new worker
    Start {
        /// Runner name from config
        #[arg(long)]
        runner: Option<String>,

        /// Inline command to execute
        #[arg(long)]
        command: Option<String>,

        /// Command arguments (can be specified multiple times)
        #[arg(long = "arg", short = 'a')]
        args: Vec<String>,

        /// Event type to subscribe to (uses runner's default if not specified)
        #[arg(long)]
        on: Option<String>,

        /// Filter expressions (can be specified multiple times)
        #[arg(long = "filter", short = 'f')]
        filters: Vec<String>,

        /// Run in background as daemon
        #[arg(long, short = 'd')]
        detached: bool,

        /// Maximum concurrent runner instances
        #[arg(long, default_value = "1")]
        concurrency: u32,

        /// Cooldown in seconds for polled events like task.next (default: 300 = 5 minutes)
        #[arg(long, default_value = "300")]
        poll_cooldown: i64,
    },

    /// Show worker status
    Status {
        /// Worker ID
        worker_id: String,
    },

    /// View worker logs
    Logs {
        /// Worker ID
        worker_id: String,

        /// Follow log output (like tail -f)
        #[arg(long, short = 'f')]
        follow: bool,

        /// Number of lines to show from the end
        #[arg(long, short = 'n', default_value = "50")]
        lines: usize,
    },

    /// Stop a worker
    Stop {
        /// Worker ID
        worker_id: String,

        /// Also stop/cancel all active runs
        #[arg(long)]
        runs: bool,
    },

    /// Remove stopped/errored workers
    Prune,
}

#[derive(Subcommand)]
pub enum RunCommand {
    /// Show run status and details
    Status {
        /// Run ID
        run_id: String,
    },

    /// View run logs
    Logs {
        /// Run ID
        run_id: String,

        /// Follow log output (like tail -f)
        #[arg(long, short = 'f')]
        follow: bool,

        /// Number of lines to show from the end
        #[arg(long, short = 'n', default_value = "100")]
        lines: usize,
    },

    /// Stop a running run
    Stop {
        /// Run ID
        run_id: String,
    },

    /// Pause a running run (sends SIGSTOP)
    Pause {
        /// Run ID
        run_id: String,
    },

    /// Resume a paused run (sends SIGCONT)
    Resume {
        /// Run ID
        run_id: String,
    },
}

#[derive(Subcommand)]
pub enum EventsAction {
    /// Drain (delete) old events
    Drain {
        /// Delete events before this duration (1h, 7d, 30m) or ISO timestamp
        before: String,
    },
}

#[derive(Subcommand)]
pub enum DaemonCommand {
    /// Show daemon status
    Status,

    /// Start the daemon (if not running)
    Start,

    /// Stop the daemon
    Stop,

    /// Restart the daemon
    Restart,

    /// Show daemon logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "50")]
        lines: usize,
    },
}

#[derive(Subcommand)]
pub enum WorkspaceAction {
    /// Initialize a new workspace for the current directory
    Init {
        /// Create a local .granary/ directory instead of a named workspace
        #[arg(long)]
        local: bool,

        /// Force initialization even if workspace already exists
        #[arg(long)]
        force: bool,

        /// Skip git root directory check
        #[arg(long)]
        skip_git_check: bool,

        /// Workspace name (derived from directory name if not specified)
        #[arg(long)]
        name: Option<String>,
    },

    /// List all workspaces
    List,

    /// Catch-all for `granary workspace <name> [add|remove|move <target>]`
    #[command(external_subcommand)]
    Named(Vec<String>),
}

#[derive(Debug)]
pub enum NamedWorkspaceAction {
    /// Show info about the named workspace
    Info,
    /// Add current directory to the named workspace
    Add,
    /// Remove current directory from the named workspace
    Remove,
    /// Move workspace root from current directory to a new path
    Move { target: PathBuf },
    /// Migrate between local and global workspace modes
    Migrate {
        /// Migrate to global mode
        global: bool,
        /// Migrate to local mode
        local: bool,
        /// Workspace name override (for --global)
        name: Option<String>,
    },
}

impl NamedWorkspaceAction {
    /// Parse a named workspace action from external subcommand args.
    /// args[0] is the workspace name, args[1..] is the action.
    pub fn parse(args: &[String]) -> Result<(String, Self), String> {
        if args.is_empty() {
            return Err("Workspace name is required".to_string());
        }

        let name = args[0].clone();

        if args.len() == 1 {
            return Ok((name, Self::Info));
        }

        match args[1].as_str() {
            "add" => {
                if args.len() > 2 {
                    return Err("'add' takes no additional arguments".to_string());
                }
                Ok((name, Self::Add))
            }
            "remove" => {
                if args.len() > 2 {
                    return Err("'remove' takes no additional arguments".to_string());
                }
                Ok((name, Self::Remove))
            }
            "move" => {
                if args.len() != 3 {
                    return Err("Usage: granary workspace <name> move <target>".to_string());
                }
                Ok((
                    name,
                    Self::Move {
                        target: PathBuf::from(&args[2]),
                    },
                ))
            }
            "migrate" => {
                let mut global = false;
                let mut local = false;
                let mut migrate_name: Option<String> = None;
                let mut i = 2;
                while i < args.len() {
                    match args[i].as_str() {
                        "--global" => global = true,
                        "--local" => local = true,
                        "--name" => {
                            i += 1;
                            if i >= args.len() {
                                return Err("--name requires a value".to_string());
                            }
                            migrate_name = Some(args[i].clone());
                        }
                        other => {
                            return Err(format!(
                                "Unknown migrate flag '{}'. Expected: --global, --local, --name",
                                other
                            ));
                        }
                    }
                    i += 1;
                }
                if !global && !local {
                    return Err(
                        "Usage: granary workspace <name> migrate --global|--local".to_string()
                    );
                }
                if global && local {
                    return Err("Cannot specify both --global and --local".to_string());
                }
                Ok((
                    name,
                    Self::Migrate {
                        global,
                        local,
                        name: migrate_name,
                    },
                ))
            }
            other => Err(format!(
                "Unknown workspace action '{}'. Expected: add, remove, move, migrate",
                other
            )),
        }
    }
}
