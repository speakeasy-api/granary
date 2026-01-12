use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::output::OutputFormat;

/// Granary - A CLI context hub for agentic work
#[derive(Parser)]
#[command(name = "granary")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Output format
    #[arg(long, global = true, value_enum, default_value = "table")]
    pub format: CliOutputFormat,

    /// JSON output (shorthand for --format json)
    #[arg(long, global = true)]
    pub json: bool,

    /// Workspace path override
    #[arg(long, global = true, env = "GRANARY_HOME")]
    pub workspace: Option<PathBuf>,

    /// Session ID override
    #[arg(long, global = true, env = "GRANARY_SESSION")]
    pub session: Option<String>,
}

impl Cli {
    pub fn output_format(&self) -> OutputFormat {
        if self.json {
            OutputFormat::Json
        } else {
            self.format.into()
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

impl From<CliOutputFormat> for OutputFormat {
    fn from(f: CliOutputFormat) -> Self {
        match f {
            CliOutputFormat::Table => OutputFormat::Table,
            CliOutputFormat::Json => OutputFormat::Json,
            CliOutputFormat::Yaml => OutputFormat::Yaml,
            CliOutputFormat::Md => OutputFormat::Md,
            CliOutputFormat::Prompt => OutputFormat::Prompt,
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Initialize a new workspace
    Init,

    /// Check workspace health
    Doctor,

    /// Show any entity by ID (auto-detects type from ID pattern)
    #[command(
        after_help = "EXAMPLES:\n    granary show my-project-abc1           # Show a project\n    granary show my-project-abc1-task-1    # Show a task\n    granary show sess-20260112-xyz1        # Show a session\n    granary show chkpt-abc123              # Show a checkpoint\n\nID PATTERNS:\n    project:    <name>-<4chars>              e.g., my-project-abc1\n    task:       <project-id>-task-<n>        e.g., my-project-abc1-task-1\n    session:    sess-<date>-<4chars>         e.g., sess-20260112-xyz1\n    checkpoint: chkpt-<6chars>               e.g., chkpt-abc123\n    comment:    <task-id>-comment-<n>        e.g., my-proj-abc1-task-1-comment-1\n    artifact:   <task-id>-artifact-<n>       e.g., my-proj-abc1-task-1-artifact-1"
    )]
    Show {
        /// Entity ID (auto-detected: project, task, session, checkpoint, comment, artifact)
        id: String,
    },

    /// List all projects or create a new one
    Projects {
        #[command(subcommand)]
        action: Option<ProjectsAction>,

        /// Include archived projects (for list)
        #[arg(long)]
        all: bool,
    },

    /// Work with a specific project or create a new one
    Project {
        /// Project ID (or "create" to create a new project)
        id: String,

        #[command(subcommand)]
        action: Option<ProjectAction>,
    },

    /// List tasks
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
    },

    /// Start a task (alias for task <id> start)
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
}

#[derive(Subcommand)]
pub enum ProjectsAction {
    /// Create a new project
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
}

#[derive(Subcommand)]
pub enum ProjectTasksAction {
    /// Create a new task
    Create {
        /// Task title
        title: String,

        /// Task description
        #[arg(long)]
        description: Option<String>,

        /// Priority (P0-P4)
        #[arg(long, default_value = "P2")]
        priority: String,

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

        /// New status (todo, in_progress, done, blocked)
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

    /// Start working on task
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
