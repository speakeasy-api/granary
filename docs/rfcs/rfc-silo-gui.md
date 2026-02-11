# RFC: Silo - Granary GUI

**Status:** Draft
**Author:** Daniel Kovacs
**Created:** 2026-02-02

## Summary

This RFC defines Silo, a comprehensive GUI application for Granary. Silo provides workspace management, navigation sidebar, auto-refresh capabilities, and views for initiatives, projects, tasks, workers, and runs.

## Motivation

A GUI for Granary will improve discoverability, reduce context-switching to CLI, and provide visual representations of complex relationships (task dependencies, initiative hierarchies, worker/run status).

## Design

### 1. Workspace Management

#### 1.1 Global Startup Mode

Start in "global" mode using user's HOME directory as the default workspace. This allows immediate access to granary state without requiring workspace selection.

```rust
impl Silo {
    fn new() -> (Self, Task<Message>) {
        let home = dirs::home_dir().unwrap_or_default();
        let silo = Self {
            workspace: Some(home.clone()),
            screen: Screen::Main,
            // ... other fields
        };
        (silo, Task::perform(load_projects(home), Message::ProjectsLoaded))
    }
}
```

#### 1.2 Workspace Selector Widget

Add a persistent workspace selector button in the header area that:
- Displays current workspace path (truncated if long)
- Opens workspace picker on click
- Shows recent workspaces dropdown on hover/click

**UI Component:**
```
┌─────────────────────────────────────────────────────────────────┐
│ silo    [~/projects/myapp ▾]              🔄 Loading...         │
│                                                                 │
│         ┌─────────────────────────┐                             │
│         │ Recent Workspaces       │                             │
│         ├─────────────────────────┤                             │
│         │ ~/projects/myapp      ✓ │                             │
│         │ ~/work/backend          │                             │
│         │ ~/personal/notes        │                             │
│         ├─────────────────────────┤                             │
│         │ Browse...               │                             │
│         └─────────────────────────┘                             │
```

#### 1.3 Recent Workspaces Persistence

Store recent workspaces in a local config file:

**Location:** `~/.granary/silo/recent_workspaces.json`

**Structure:**
```json
{
  "recent": [
    {
      "path": "/Users/daniel/projects/myapp",
      "last_accessed": "2026-02-02T10:30:00Z"
    }
  ],
  "max_recent": 10
}
```

**New Module:** `src/config.rs`
- Load/save recent workspaces
- Add workspace to recent list on selection
- Prune list to max_recent entries

### 2. Navigation & General UI

#### 2.1 Sidebar Navigation

Replace the current two-panel layout with a three-panel layout:

```
┌──────┬──────────────────────────────────────────────────────────┐
│      │ Header with workspace selector                          │
├──────┼──────────────────────────────────────────────────────────┤
│      │                                                          │
│  ◉   │                                                          │
│ Init │                                                          │
│      │                                                          │
│  □   │                                                          │
│ Proj │            Main Content Area                             │
│      │                                                          │
│  ☑   │                                                          │
│ Task │                                                          │
│      │                                                          │
│  ⚙   │                                                          │
│ Work │                                                          │
│      │                                                          │
└──────┴──────────────────────────────────────────────────────────┘
```

**Sidebar Items:**
| Icon | Label | View | Description |
|------|-------|------|-------------|
| ◉ | Initiatives | `Screen::Initiatives` | Multi-project coordination |
| □ | Projects | `Screen::Projects` | All projects list |
| ☑ | Tasks | `Screen::Tasks` | All tasks (filterable) |
| ⚡ | Workers | `Screen::Workers` | Background automation |
| ▶ | Runs | `Screen::Runs` | Command executions |
| ⚙ | Settings | `Screen::Settings` | App preferences |

**New Screen Enum:**
```rust
pub enum Screen {
    SelectWorkspace,    // Initial onboarding (if no recent workspaces)
    Initiatives,        // Initiative list
    InitiativeDetail,   // Single initiative with tasks
    Projects,           // Project list
    ProjectDetail,      // Single project with tasks
    Tasks,              // All tasks view
    TaskDetail,         // Single task detail/edit
    CreateProject,      // New project form
    CreateTask,         // New task form
    EditTask,           // Edit task form
    Workers,            // Worker list
    WorkerDetail,       // Single worker with runs
    StartWorker,        // New worker form
    Runs,               // All runs view
    RunDetail,          // Single run detail
    Logs,               // Log viewer (worker or run)
    Settings,           // App settings (config, runners, steering)
}
```

#### 2.2 Auto-Refresh via Subscriptions

Implement periodic data refresh using Iced subscriptions:

```rust
impl Silo {
    fn subscription(&self) -> Subscription<Message> {
        if self.auto_refresh_enabled {
            iced::time::every(Duration::from_secs(3))
                .map(|_| Message::AutoRefresh)
        } else {
            Subscription::none()
        }
    }
}
```

**Auto-refresh Logic:**
- Only refresh data relevant to current screen
- Skip refresh if a modal/form is open
- Show subtle refresh indicator (not full loading state)
- Debounce to prevent excessive CLI calls

**New Messages:**
```rust
pub enum Message {
    // ... existing messages
    AutoRefresh,
    ToggleAutoRefresh,
    RefreshComplete,
}
```

### 3. Initiatives View

#### 3.1 Initiatives List Screen

Display active initiatives with summary statistics:

```
┌──────────────────────────────────────────────────────────────────┐
│ Initiatives                                        [+ Create]    │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│ ┌──────────────────────────────────────────────────────────────┐│
│ │ 🎯 Q1 Platform Redesign                              Active  ││
│ │    3 projects • 45 tasks • 67% complete                      ││
│ │    ████████████████████░░░░░░░░░░                           ││
│ │    2 blockers • Next: Implement auth service                 ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
│ ┌──────────────────────────────────────────────────────────────┐│
│ │ 📦 Infrastructure Migration                          Active  ││
│ │    2 projects • 18 tasks • 33% complete                      ││
│ │    ██████████░░░░░░░░░░░░░░░░░░░░                           ││
│ │    0 blockers • Next: Configure Kubernetes                   ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

**Data Source:** `granary initiatives --json`

**Display Fields:**
- Initiative name and status badge
- Project count, task count
- Completion percentage with progress bar
- Blocker count
- Next action preview

#### 3.2 Initiative Detail View

Clicking an initiative navigates to a filtered task view:

```
┌──────────────────────────────────────────────────────────────────┐
│ ← Back    Q1 Platform Redesign                        [Archive]  │
├──────────────────────────────────────────────────────────────────┤
│ Description: Complete redesign of the core platform...          │
│ Owner: alice@example.com                                         │
│ Progress: ████████████████████░░░░░░░░░░  67% (30/45 tasks)     │
├──────────────────────────────────────────────────────────────────┤
│ Projects in Initiative                                           │
│ ┌───────────────────┐ ┌───────────────────┐ ┌───────────────────┐│
│ │ auth-service      │ │ api-gateway       │ │ frontend-app     ││
│ │ 12/15 tasks done  │ │ 8/20 tasks done   │ │ 10/10 tasks done ││
│ └───────────────────┘ └───────────────────┘ └───────────────────┘│
├──────────────────────────────────────────────────────────────────┤
│ Tasks                                               [+ Add Task] │
│ ┌──────────────────────────────────────────────────────────────┐│
│ │ [Dependency Tree View - see Section 5.1]                     ││
│ └──────────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────┘
```

**Data Source:** `granary initiative <id> summary --json`

**Initiative Summary JSON Structure:**
```json
{
  "initiative": { "id": "...", "name": "...", "status": "..." },
  "status": {
    "total_projects": 3,
    "completed_projects": 1,
    "blocked_projects": 0,
    "total_tasks": 45,
    "tasks_done": 30,
    "tasks_in_progress": 5,
    "tasks_blocked": 2,
    "tasks_todo": 8,
    "percent_complete": 67.0
  },
  "projects": [...],
  "blockers": [...],
  "next_actions": [...]
}
```

### 4. Projects View Improvements

#### 4.1 Enhanced Project Cards

Current project cards show minimal info. Enhance with:

```
┌──────────────────────────────────────────────────────────────────┐
│ □ my-project-abc1                                      [Active] │
│   A comprehensive project description that spans multiple      │
│   lines if needed...                                            │
│                                                                  │
│   Owner: daniel@example.com                                      │
│   Tags: backend, critical, q1-2026                              │
│   Created: Jan 15, 2026 • Updated: 2 hours ago                  │
│                                                                  │
│   Tasks: 12 todo, 5 in progress, 30 done, 2 blocked             │
│   ████████████████████████████████░░░░░░░░  76%                 │
│                                                                  │
│   [View Tasks]    [Archive]    [Edit]                           │
└──────────────────────────────────────────────────────────────────┘
```

**Display Fields (from `granary_types::Project`):**
- `id`, `name` (with slug visible)
- `description` (truncated with expand)
- `owner`
- `status` (badge: Active/Archived)
- `tags` (parsed from JSON array string)
- `created_at`, `updated_at` (formatted)
- Task statistics (aggregated from tasks query)

#### 4.2 Project Status Controls

Add action buttons based on current status:

| Current Status | Available Actions |
|----------------|-------------------|
| Active | Archive, Edit |
| Archived | Unarchive, Delete |

**CLI Commands:**
- Archive: `granary project <id> archive`
- Unarchive: `granary project <id> update --status active`

**New Messages:**
```rust
pub enum Message {
    // ... existing
    ArchiveProject(String),
    UnarchiveProject(String),
    ProjectStatusUpdated(Result<(), String>),
}
```

#### 4.3 Create Project Form

New screen for project creation with all CLI options:

```
┌──────────────────────────────────────────────────────────────────┐
│ ← Cancel                Create New Project            [Create]   │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Name *                                                          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ My New Project                                              │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Description                                                     │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │                                                              │ │
│  │                                                              │ │
│  │                                                              │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Owner                                                           │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ daniel@example.com                                          │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Tags (comma-separated)                                          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ backend, api, q1-2026                                       │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

**CLI Command:** `granary projects create <name> --description "..." --owner "..." --tags "..."`

**Form State:**
```rust
pub struct CreateProjectForm {
    name: String,
    description: String,
    owner: String,
    tags: String,
    error: Option<String>,
    submitting: bool,
}
```

### 5. Tasks View Improvements

#### 5.1 Dependency Tree Visualization

Display tasks with their dependencies as a visual tree/graph:

```
┌──────────────────────────────────────────────────────────────────┐
│ Task Dependencies                                    [List View] │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────┐                                             │
│  │ Setup Database  │ ✓ Done                                      │
│  └────────┬────────┘                                             │
│           │                                                      │
│           ▼                                                      │
│  ┌─────────────────┐     ┌─────────────────┐                     │
│  │ Create Models   │────▶│ Implement API   │ ◐ In Progress      │
│  │ ✓ Done          │     └────────┬────────┘                     │
│  └─────────────────┘              │                              │
│                                   ▼                              │
│                          ┌─────────────────┐                     │
│                          │ Write Tests     │ ○ Todo              │
│                          └────────┬────────┘                     │
│                                   │                              │
│                                   ▼                              │
│                          ┌─────────────────┐                     │
│                          │ Deploy to Prod  │ ○ Todo              │
│                          └─────────────────┘                     │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

**Implementation Approach:**

1. **Data Fetching:** Use `granary project <id> tasks --json` which includes `blocked_by` field
2. **Graph Building:** Build adjacency list from task dependencies
3. **Layout Algorithm:** Use topological sort + level assignment for vertical positioning
4. **Rendering:** Custom Iced canvas widget or SVG-like element composition

**Data Structure:**
```rust
pub struct TaskGraph {
    tasks: HashMap<String, GranaryTask>,
    edges: Vec<(String, String)>,  // (from_id, to_id)
    levels: HashMap<String, usize>, // task_id -> depth level
}

impl TaskGraph {
    fn from_tasks(tasks: Vec<GranaryTask>) -> Self;
    fn topological_sort(&self) -> Vec<String>;
    fn assign_levels(&mut self);
}
```

**New Widget:** `src/widget/task_graph.rs`

#### 5.2 Create Task Form

Full task creation form with all available fields:

```
┌──────────────────────────────────────────────────────────────────┐
│ ← Cancel                  Create New Task             [Create]   │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Title *                                                         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Implement user authentication                               │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Description                                                     │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ Add JWT-based authentication with refresh tokens.           │ │
│  │                                                              │ │
│  │ ## Goal                                                      │ │
│  │ Secure all API endpoints with proper auth.                  │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Priority                          Status                        │
│  ┌───────────────────┐             ┌───────────────────┐        │
│  │ P1 - High       ▾ │             │ Draft           ▾ │        │
│  └───────────────────┘             └───────────────────┘        │
│                                                                  │
│  Owner                             Due Date                      │
│  ┌───────────────────┐             ┌───────────────────┐        │
│  │                   │             │ 2026-02-15        │        │
│  └───────────────────┘             └───────────────────┘        │
│                                                                  │
│  Tags (comma-separated)                                          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ auth, security, backend                                     │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Dependencies (blocks this task)                                 │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ [Setup Database] [x]  [Create Models] [x]                   │ │
│  │ [+ Add Dependency]                                          │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

**Form State:**
```rust
pub struct CreateTaskForm {
    project_id: String,
    title: String,
    description: String,
    priority: TaskPriority,
    status: TaskStatus,
    owner: String,
    due_date: String,
    tags: String,
    dependencies: Vec<String>,  // task IDs
    error: Option<String>,
    submitting: bool,
}
```

**CLI Command:**
```bash
granary project <project_id> tasks create "<title>" \
  --description "..." \
  --priority P1 \
  --status draft \
  --owner "..." \
  --due "2026-02-15" \
  --tags "auth,security"
```

Then add dependencies:
```bash
granary task <task_id> deps add <dependency_id>
```

#### 5.3 Edit Task Form

Similar to create form but pre-populated with existing values:

**CLI Command:** `granary task <id> update --title "..." --description "..." ...`

#### 5.4 Enhanced Task Card Display

Improved task card with better organization when expanded:

```
┌──────────────────────────────────────────────────────────────────┐
│ ┌──────────────────────────────────────────────────────────────┐│
│ │ ◐ Implement user authentication                     P1  [▼] ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
│ ┌─ Expanded Details ───────────────────────────────────────────┐│
│ │                                                               ││
│ │  Status        ◐ In Progress    Priority      🔴 P1 - High  ││
│ │  Owner         alice@example     Due           Feb 15, 2026  ││
│ │                                                               ││
│ │  Description                                                  ││
│ │  ┌─────────────────────────────────────────────────────────┐ ││
│ │  │ Add JWT-based authentication with refresh tokens.       │ ││
│ │  │                                                         │ ││
│ │  │ ## Goal                                                 │ ││
│ │  │ Secure all API endpoints with proper auth.              │ ││
│ │  └─────────────────────────────────────────────────────────┘ ││
│ │                                                               ││
│ │  Tags          auth • security • backend                      ││
│ │                                                               ││
│ │  ┌─ Timeline ───────────────────────────────────────────────┐││
│ │  │ Created    Jan 15, 2026 at 10:30 AM                      │││
│ │  │ Started    Jan 20, 2026 at 2:15 PM                       │││
│ │  │ Updated    2 hours ago                                   │││
│ │  └──────────────────────────────────────────────────────────┘││
│ │                                                               ││
│ │  ┌─ Dependencies ───────────────────────────────────────────┐││
│ │  │ Blocked by: Setup Database ✓, Create Models ✓            │││
│ │  │ Blocks: Write Tests, Deploy to Prod                      │││
│ │  └──────────────────────────────────────────────────────────┘││
│ │                                                               ││
│ │  ID: my-project-abc1-task-5                                   ││
│ │                                                               ││
│ │  [Complete]  [Block]  [Edit]                                  ││
│ │                                                               ││
│ └───────────────────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────┘
```

**Formatting Functions:**
```rust
fn format_date(iso_string: &str) -> String {
    // "2026-01-15T10:30:00Z" -> "Jan 15, 2026 at 10:30 AM"
}

fn format_relative_time(iso_string: &str) -> String {
    // "2026-02-02T08:30:00Z" -> "2 hours ago"
}

fn format_tags(tags_json: &Option<String>) -> Vec<String> {
    // "[\"auth\", \"security\"]" -> ["auth", "security"]
}
```

#### 5.5 Task Status Icons

Replace text badges with visual icons for each status:

| Status | Icon | Color | Behavior |
|--------|------|-------|----------|
| Draft | ○ (empty circle) | Gray | Static |
| Todo | ○ (empty circle) | Blue | Static |
| In Progress | ◐ (arc spinner) | Blue | **Animated rotation** |
| Done | ✓ (checkmark) | Green | Static |
| Blocked | ⊘ (blocked) | Red | Static |

**In Progress Spinner:** Rendered as a circle with a 270° arc stroke that rotates continuously. Drawn using Iced's canvas API rather than an emoji.

**Spinner Implementation:**

Use Iced subscription for animation:

```rust
fn subscription(&self) -> Subscription<Message> {
    let refresh = /* ... */;

    let spinner = if self.has_in_progress_tasks() {
        iced::time::every(Duration::from_millis(100))
            .map(|_| Message::SpinnerTick)
    } else {
        Subscription::none()
    };

    Subscription::batch([refresh, spinner])
}

// In app state
spinner_rotation: f32,  // 0.0 to 360.0

// In update
Message::SpinnerTick => {
    self.spinner_rotation = (self.spinner_rotation + 30.0) % 360.0;
    Task::none()
}
```

**New Widget:** `src/widget/status_icon.rs`

```rust
pub fn status_icon<'a>(
    status: TaskStatus,
    spinner_rotation: f32,
    palette: &Palette,
) -> Element<'a, Message> {
    match status {
        TaskStatus::Draft => circle_outline(palette.text_muted),
        TaskStatus::Todo => circle_outline(palette.status_todo),
        TaskStatus::InProgress => spinner(spinner_rotation, palette.status_progress),
        TaskStatus::Done => checkmark(palette.status_done),
        TaskStatus::Blocked => blocked_icon(palette.status_blocked),
    }
}
```

### 6. Configuration View

The GUI should provide access to granary's configuration system, which consists of:
- **Global config**: `~/.granary/config.toml` (runner definitions)
- **Workspace config**: Key-value pairs in workspace database
- **Steering files**: Scoped guidance files for workers

#### 6.1 Global Config Location & Format

```
~/.granary/
├── config.toml           # Global configuration (runners)
├── workers.db            # Global database (workers, runs)
└── daemon/
    ├── granaryd.sock     # Unix socket
    ├── granaryd.pid      # Process ID
    └── auth.token        # Auth token (0600 permissions)
```

#### 6.2 Runner Configuration

Runners are named command configurations that workers can reference:

```rust
pub struct RunnerConfig {
    pub command: String,              // Binary to execute (e.g., "claude", "python")
    pub args: Vec<String>,            // Arguments (supports ${VAR} expansion)
    pub concurrency: Option<u32>,     // Max concurrent executions
    pub on: Option<String>,           // Default event type (e.g., "task.next")
    pub env: HashMap<String, String>, // Environment variables
}
```

**Example `~/.granary/config.toml`:**
```toml
[runners.claude]
command = "claude"
args = ["--print", "--message", "Execute task {task.id}"]
concurrency = 2
on = "task.next"

[runners.claude.env]
GRANARY_DEBUG = "true"
API_KEY = "${GRANARY_TOKEN}"
```

#### 6.3 Steering Files

Steering files provide scoped guidance to workers:

```rust
pub struct SteeringFile {
    pub id: i64,
    pub path: String,
    pub mode: String,              // "always" | "on-demand"
    pub scope_type: Option<String>, // None=global, "project", "task", "session"
    pub scope_id: Option<String>,
    pub created_at: String,
}
```

#### 6.4 Settings Screen Design

```
┌──────────────────────────────────────────────────────────────────┐
│ Settings                                                         │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│ ┌─ Runners ────────────────────────────────────────────────────┐│
│ │                                                    [+ Add]   ││
│ │ ┌────────────────────────────────────────────────────────┐  ││
│ │ │ claude                                                  │  ││
│ │ │ Command: claude --print --message "..."                 │  ││
│ │ │ Event: task.next • Concurrency: 2                       │  ││
│ │ │ [Edit] [Delete]                                         │  ││
│ │ └────────────────────────────────────────────────────────┘  ││
│ │ ┌────────────────────────────────────────────────────────┐  ││
│ │ │ python-runner                                           │  ││
│ │ │ Command: python scripts/worker.py                       │  ││
│ │ │ Event: task.unblocked • Concurrency: 1                  │  ││
│ │ │ [Edit] [Delete]                                         │  ││
│ │ └────────────────────────────────────────────────────────┘  ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
│ ┌─ Steering Files ─────────────────────────────────────────────┐│
│ │                                                    [+ Add]   ││
│ │ Global                                                       ││
│ │   • ~/.granary/steering/always.md (always)                   ││
│ │ Project: api-gateway-abc1                                    ││
│ │   • ./docs/api-guidelines.md (on-demand)                     ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
│ ┌─ Workspace Config ───────────────────────────────────────────┐│
│ │ Key                          Value                  [+ Add]  ││
│ │ default_owner                alice@example.com      [Delete] ││
│ │ default_priority             P2                     [Delete] ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

#### 6.5 CLI Commands for Config

| Action | CLI Command |
|--------|-------------|
| List runners | `granary config runners` |
| Add runner | `granary config runners add <name> --command <cmd> --arg <arg> --on <event> --concurrency <n>` |
| Update runner | `granary config runners update <name> ...` |
| Remove runner | `granary config runners rm <name>` |
| Show runner | `granary config runners show <name>` |
| Get config value | `granary config get <key>` |
| Set config value | `granary config set <key> <value>` |
| List config | `granary config list` |
| Delete config | `granary config delete <key>` |
| Edit config file | `granary config edit` |
| List steering | `granary steering list` |
| Add steering | `granary steering add <path> --mode <mode> [--project <id>] [--task <id>]` |
| Remove steering | `granary steering rm <path>` |

### 7. Workers View

Workers are long-running daemon processes that subscribe to events and spawn runner processes.

#### 7.1 Worker Data Model

```rust
pub struct Worker {
    pub id: String,                    // "worker-<8char>"
    pub runner_name: Option<String>,   // References configured runner
    pub command: String,               // Command to execute
    pub args: String,                  // JSON array of arguments
    pub event_type: String,            // Subscription: "task.unblocked", etc.
    pub filters: String,               // JSON array of filter expressions
    pub concurrency: i32,              // Max concurrent runner instances
    pub instance_path: String,         // Workspace root path
    pub status: String,                // "pending"|"running"|"stopped"|"error"
    pub error_message: Option<String>, // Error details if status=error
    pub pid: Option<i64>,              // OS process ID when running
    pub detached: bool,                // Daemon mode flag
    pub created_at: String,
    pub updated_at: String,
    pub stopped_at: Option<String>,
    pub poll_cooldown_secs: i64,       // For task.next/project.next (default 300s)
    pub last_event_id: i64,            // Cursor for event polling
}
```

#### 7.2 Worker Status Lifecycle

```
┌──────────┐
│ Pending  │  (Initial state after creation)
└─────┬────┘
      │ [Daemon starts worker]
      ▼
┌──────────┐
│ Running  │  (Actively polling for events, spawning runs)
└─────┬────┘
      │
      ├──[Graceful stop]──→ ┌─────────┐
      │                      │ Stopped │
      │                      └─────────┘
      │
      └──[Error: workspace deleted, etc.]──→ ┌───────┐
                                              │ Error │
                                              └───────┘
```

#### 7.3 Event Types

Workers can subscribe to these event types:

| Event Type | Description |
|------------|-------------|
| `task.created` | New task created |
| `task.unblocked` | Task dependencies satisfied |
| `task.done` | Task completed |
| `task.next` | Synthetic: next available task (polled) |
| `project.next` | Synthetic: next available project (polled) |
| `project.initialized` | Project created |
| `project.completed` | All project tasks done |

**Filters** allow selective matching:
```
["status=todo", "priority=P0", "owner!=", "project.name~=api"]
```

#### 7.4 Workers List Screen

The workers screen is split into two sections:
1. **Available Runners** - Configured runners from `~/.granary/config.toml` with quick-start buttons
2. **Active Workers** - Currently running or recently stopped workers

```
┌──────────────────────────────────────────────────────────────────┐
│ Workers                                                          │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│ ┌─ Available Runners ──────────────────────────────────────────┐│
│ │                                                               ││
│ │ ┌─────────────────────────────────────────────────────────┐  ││
│ │ │ claude                                                   │  ││
│ │ │ claude --print --message "Execute task {task.id}"        │  ││
│ │ │ Event: task.next • Concurrency: 2                        │  ││
│ │ │                                                          │  ││
│ │ │ [▶ Start]  [Customize...]                                │  ││
│ │ └─────────────────────────────────────────────────────────┘  ││
│ │                                                               ││
│ │ ┌─────────────────────────────────────────────────────────┐  ││
│ │ │ python-worker                                            │  ││
│ │ │ python scripts/worker.py --task {task.id}                │  ││
│ │ │ Event: task.unblocked • Concurrency: 1                   │  ││
│ │ │                                                          │  ││
│ │ │ [▶ Start]  [Customize...]                                │  ││
│ │ └─────────────────────────────────────────────────────────┘  ││
│ │                                                               ││
│ │ No runners configured? [Add Runner in Settings]               ││
│ │                                                               ││
│ └───────────────────────────────────────────────────────────────┘│
│                                                                  │
│ ┌─ Active Workers ─────────────────────────────── [+ Custom...] ┐│
│ │                                                               ││
│ │ ┌─────────────────────────────────────────────────────────┐  ││
│ │ │ ● worker-abc12345 (claude)                     Running  │  ││
│ │ │   Event: task.next • Concurrency: 2/2 active            │  ││
│ │ │   Workspace: ~/projects/myapp                           │  ││
│ │ │   PID: 12345 • Started: 2 hours ago                     │  ││
│ │ │                                                          │  ││
│ │ │   [View Runs]  [View Logs]  [Stop]                      │  ││
│ │ └─────────────────────────────────────────────────────────┘  ││
│ │                                                               ││
│ │ ┌─────────────────────────────────────────────────────────┐  ││
│ │ │ ○ worker-def67890 (python-worker)              Stopped  │  ││
│ │ │   Event: task.unblocked • Filters: priority=P0          │  ││
│ │ │   Workspace: ~/work/backend                             │  ││
│ │ │   Stopped: 1 day ago                                    │  ││
│ │ │                                                          │  ││
│ │ │   [View Runs]  [View Logs]  [Restart]  [Delete]         │  ││
│ │ └─────────────────────────────────────────────────────────┘  ││
│ │                                                               ││
│ │ ┌─────────────────────────────────────────────────────────┐  ││
│ │ │ ⚠ worker-ghi01234                                Error  │  ││
│ │ │   Error: Workspace directory no longer exists           │  ││
│ │ │   Event: project.next                                   │  ││
│ │ │                                                          │  ││
│ │ │   [View Logs]  [Delete]                                 │  ││
│ │ └─────────────────────────────────────────────────────────┘  ││
│ │                                                               ││
│ └───────────────────────────────────────────────────────────────┘│
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

**Quick Start Flow:**
- Clicking **[▶ Start]** immediately runs: `granary worker start --runner=<name> -d`
- Uses all defaults from the runner config
- Worker starts in detached (daemon) mode

**Customize Flow:**
- Clicking **[Customize...]** opens the Start Worker Form (7.5) pre-populated with runner config
- Allows overriding event type, filters, concurrency, etc.

**Custom Worker:**
- Clicking **[+ Custom...]** opens a blank Start Worker Form for inline command configuration

#### 7.5 Start Worker Form (Customize)

Opened when clicking [Customize...] on a runner or [+ Custom...] for a new inline worker.
Pre-populated with runner config values when customizing an existing runner.

```
┌──────────────────────────────────────────────────────────────────┐
│ ← Cancel                  Start Worker                [Start]    │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│  Based on runner: claude                    [Clear to customize] │
│                                                                  │
│  Command                                                         │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ claude                                                      │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Arguments (one per line, supports {task.id} templates)          │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ --print                                                     │ │
│  │ --message                                                   │ │
│  │ Execute task {task.id}: {task.title}                        │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Subscribe to Event                                              │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ task.next                                                ▾ │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Filters (optional, one per line)                                │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ priority=P0                                                 │ │
│  │ status=todo                                                 │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Concurrency             Poll Cooldown (for task.next)           │
│  ┌──────────────┐        ┌──────────────────────────────────┐   │
│  │ 2            │        │ 300 seconds                      │   │
│  └──────────────┘        └──────────────────────────────────┘   │
│                                                                  │
│  ☑ Run as daemon (detached)                                      │
│                                                                  │
│  ┌─ Environment Variables (from runner config) ────────────────┐│
│  │ GRANARY_DEBUG = true                                        ││
│  │ API_KEY = ${GRANARY_TOKEN}                                  ││
│  └──────────────────────────────────────────────────────────────┘│
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

**CLI Commands Generated:**
- Quick start: `granary worker start --runner=claude -d`
- Customized: `granary worker start --runner=claude --filter "priority=P0" --concurrency 1 -d`
- Inline: `granary worker start --command claude --arg "--print" --on task.next -d`

#### 7.6 CLI Commands for Workers

| Action | CLI Command |
|--------|-------------|
| List workers | `granary workers [--all] [--json]` |
| Start worker | `granary worker start --runner <name> --on <event> [--filter <expr>] [--concurrency <n>] [-d]` |
| Start inline | `granary worker start --command <cmd> --arg <arg> --on <event>` |
| Worker status | `granary worker status <worker_id>` |
| Worker logs | `granary worker logs <worker_id> [-f] [-n <lines>]` |
| Stop worker | `granary worker stop <worker_id> [--runs]` |
| Prune workers | `granary worker prune` |

### 8. Runs View

A **Run** is a single execution of a command spawned by a Worker in response to an event.

#### 8.1 Run Data Model

```rust
pub struct Run {
    pub id: String,                    // "run-<8char>"
    pub worker_id: String,             // Parent worker
    pub event_id: i64,                 // Triggering event ID (0 for synthetic)
    pub event_type: String,            // E.g., "task.unblocked"
    pub entity_id: String,             // What triggered it (task ID, etc.)
    pub command: String,               // Resolved command
    pub args: String,                  // JSON array (after template substitution)
    pub status: String,                // "pending"|"running"|"completed"|"failed"|"paused"|"cancelled"
    pub exit_code: Option<i32>,        // Exit code (0 = success)
    pub error_message: Option<String>, // Error details if failed
    pub attempt: i32,                  // Retry attempt (1-based)
    pub max_attempts: i32,             // Max retry attempts (default 3)
    pub next_retry_at: Option<String>, // RFC3339 timestamp for retry
    pub pid: Option<i64>,              // OS process ID
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

#### 8.2 Run Status Lifecycle

```
┌─────────┐
│ Pending │ (Queued, waiting to start or retry)
└────┬────┘
     │ [Spawn process]
     ▼
┌─────────┐
│ Running │ (Has PID, executing)
└────┬────┘
     │ [Process exits]
     │
     ├──[exit code 0]──────→ ┌───────────┐
     │                        │ Completed │
     │                        └───────────┘
     │
     └──[exit code != 0]───→ ┌────────┐
                              │ Failed │
                              └───┬────┘
                                  │ [can_retry?]
                                  │
                           ┌──────┴──────┐
                           │             │
                      [Yes]             [No]
                           │             │
                           ▼             │
                      ┌─────────┐        │
                      │ Pending │        │
                      │(retry)  │        │
                      └─────────┘        │
                                         ▼
                                  ┌────────┐
                                  │ Failed │
                                  │(final) │
                                  └────────┘

Additional states:
┌────────┐  SIGSTOP   ┌────────┐
│Running │───────────▶│ Paused │
└────────┘            └────────┘
                          │ SIGCONT
                          ▼
                      ┌────────┐
                      │Running │
                      └────────┘

┌─────────────┐  SIGTERM   ┌───────────┐
│Running/     │───────────▶│ Cancelled │
│Pending/     │            └───────────┘
│Paused       │
└─────────────┘
```

#### 8.3 Retry Logic

Failed runs are retried with exponential backoff:
- **Base delay**: 5 seconds
- **Max attempts**: 3 (configurable)
- **Backoff formula**: `base * 2^(attempt-1)` → 5s, 10s, 20s, 40s...

#### 8.4 Runs List Screen

```
┌──────────────────────────────────────────────────────────────────┐
│ Runs                                    [Filter: All ▾] [🔄]     │
├──────────────────────────────────────────────────────────────────┤
│                                                                  │
│ ID            Worker          Event           Entity      Status │
│ ─────────────────────────────────────────────────────────────────│
│ run-abc123    worker-xyz...   task.next       proj-task-5  ● Running │
│ run-def456    worker-xyz...   task.next       proj-task-3  ✓ Completed │
│ run-ghi789    worker-abc...   task.unblocked  proj-task-8  ✗ Failed (1/3) │
│ run-jkl012    worker-abc...   task.unblocked  proj-task-8  ⏸ Paused │
│                                                                  │
├──────────────────────────────────────────────────────────────────┤
│ Selected: run-abc123                                             │
│ ┌──────────────────────────────────────────────────────────────┐│
│ │ Run: run-abc123                                               ││
│ │ Worker: worker-xyz12345                                       ││
│ │ Event: task.next (ID: 0)                                      ││
│ │ Entity: my-project-abc1-task-5                                ││
│ │                                                               ││
│ │ Status: Running                                               ││
│ │ Attempt: 1/3                                                  ││
│ │ PID: 54321                                                    ││
│ │                                                               ││
│ │ Command: claude --print --message "Execute task..."           ││
│ │                                                               ││
│ │ Started: Feb 2, 2026 at 10:30 AM                              ││
│ │ Created: Feb 2, 2026 at 10:30 AM                              ││
│ │                                                               ││
│ │ [View Logs]  [Pause]  [Stop]                                  ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

#### 8.5 Run Status Icons

| Status | Icon | Color |
|--------|------|-------|
| Pending | ○ | Gray |
| Running | ● (spinner) | Blue |
| Completed | ✓ | Green |
| Failed | ✗ | Red |
| Paused | ⏸ | Orange |
| Cancelled | ⊘ | Gray |

#### 8.6 CLI Commands for Runs

| Action | CLI Command |
|--------|-------------|
| List runs | `granary runs [--worker <id>] [--status <s>] [--all] [--limit <n>] [--json]` |
| Run status | `granary run status <run_id>` |
| Run logs | `granary run logs <run_id> [-f] [-n <lines>]` |
| Stop run | `granary run stop <run_id>` |
| Pause run | `granary run pause <run_id>` |
| Resume run | `granary run resume <run_id>` |

### 9. Logs View

Both workers and runs produce logs that should be viewable in the GUI. Logs are accessed via the granary CLI, not directly from the filesystem.

#### 9.1 Log Viewer Component

```
┌──────────────────────────────────────────────────────────────────┐
│ Logs: worker-abc12345                        [Follow ☑] [Clear]  │
├──────────────────────────────────────────────────────────────────┤
│ ┌──────────────────────────────────────────────────────────────┐│
│ │ [10:30:01] Starting worker with event subscription: task.next││
│ │ [10:30:01] Workspace: /Users/daniel/projects/myapp           ││
│ │ [10:30:02] Polling for events...                             ││
│ │ [10:30:05] Event matched: task my-project-abc1-task-5        ││
│ │ [10:30:05] Spawning run: run-abc123                          ││
│ │ [10:30:05] Run started with PID 54321                        ││
│ │ [10:32:15] Run run-abc123 completed (exit code 0)            ││
│ │ [10:32:15] Polling for events...                             ││
│ │ █                                                             ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
│ Lines: 1-50 of 250                    [Load More ↑] [Jump to End]│
└──────────────────────────────────────────────────────────────────┘
```

#### 9.2 Run Log Viewer

```
┌──────────────────────────────────────────────────────────────────┐
│ Logs: run-abc123 (Running)                   [Follow ☑] [Stop]   │
├──────────────────────────────────────────────────────────────────┤
│ ┌──────────────────────────────────────────────────────────────┐│
│ │ Executing task: Implement user authentication                ││
│ │ Priority: P1                                                  ││
│ │                                                               ││
│ │ Reading project context...                                    ││
│ │ Found 3 steering files                                        ││
│ │                                                               ││
│ │ Starting implementation...                                    ││
│ │ - Created src/auth/mod.rs                                     ││
│ │ - Created src/auth/jwt.rs                                     ││
│ │ - Modified src/main.rs                                        ││
│ │                                                               ││
│ │ Running tests...                                              ││
│ │ █                                                             ││
│ └──────────────────────────────────────────────────────────────┘│
│                                                                  │
│ Status: Running • PID: 54321 • Attempt: 1/3                      │
└──────────────────────────────────────────────────────────────────┘
```

#### 9.3 Log Streaming Implementation

**CLI Commands:**
- `granary worker logs <worker_id> [-f] [-n <lines>]`
- `granary run logs <run_id> [-f] [-n <lines>]`

**Follow mode (`-f`):**
1. Initial fetch: last N lines (default 100)
2. Poll for new lines continuously
3. Continue until run finishes and no more output
4. User can toggle follow on/off in the GUI

### 10. New File Structure

```
crates/silo/src/
├── main.rs
├── lib.rs
├── app.rs
├── message.rs
├── config.rs
├── granary_cli.rs
├── util/
│   ├── mod.rs
│   └── date_format.rs
├── screen/
│   ├── mod.rs
│   ├── select_workspace.rs
│   ├── initiatives.rs
│   ├── initiative_detail.rs
│   ├── projects.rs
│   ├── project_detail.rs
│   ├── tasks.rs
│   ├── create_project.rs
│   ├── create_task.rs
│   ├── edit_task.rs
│   ├── workers.rs
│   ├── runs.rs
│   └── settings.rs
├── appearance/
│   ├── mod.rs
│   └── button.rs
└── widget/
    ├── mod.rs
    ├── sidebar.rs
    ├── workspace_selector.rs
    ├── status_icon.rs
    ├── progress_bar.rs
    ├── task_graph.rs
    ├── log_viewer.rs
    └── form/
        ├── mod.rs
        ├── text_input.rs
        ├── text_area.rs
        ├── select.rs
        └── date_picker.rs
```

### 11. Message Enum Expansion

```rust
pub enum Message {
    // === Workspace ===
    SelectWorkspace,
    WorkspaceSelected(Option<PathBuf>),
    SwitchWorkspace(PathBuf),
    OpenWorkspaceMenu,
    CloseWorkspaceMenu,

    // === Navigation ===
    Navigate(Screen),
    GoBack,

    // === Auto-refresh ===
    AutoRefresh,
    ToggleAutoRefresh,
    RefreshComplete,
    SpinnerTick,

    // === Initiatives ===
    InitiativesLoaded(Result<Vec<Initiative>, String>),
    SelectInitiative(String),
    InitiativeDetailLoaded(Result<InitiativeSummary, String>),
    ArchiveInitiative(String),
    InitiativeUpdated(Result<(), String>),

    // === Projects ===
    ProjectsLoaded(Result<Vec<Project>, String>),
    SelectProject(String),
    RefreshProjects,
    ArchiveProject(String),
    UnarchiveProject(String),
    ProjectUpdated(Result<(), String>),

    // === Create Project Form ===
    OpenCreateProject,
    CreateProjectName(String),
    CreateProjectDescription(String),
    CreateProjectOwner(String),
    CreateProjectTags(String),
    SubmitCreateProject,
    ProjectCreated(Result<Project, String>),
    CancelCreateProject,

    // === Tasks ===
    TasksLoaded(Result<Vec<GranaryTask>, String>),
    RefreshTasks,
    ToggleTaskExpand(String),
    StartTask(String),
    CompleteTask(String),
    BlockTask(String),
    TaskUpdated(Result<(), String>),
    ToggleTaskView(TaskViewMode), // List or Graph

    // === Create Task Form ===
    OpenCreateTask(String), // project_id
    CreateTaskTitle(String),
    CreateTaskDescription(String),
    CreateTaskPriority(TaskPriority),
    CreateTaskStatus(TaskStatus),
    CreateTaskOwner(String),
    CreateTaskDueDate(String),
    CreateTaskTags(String),
    AddTaskDependency(String),
    RemoveTaskDependency(String),
    SubmitCreateTask,
    TaskCreated(Result<GranaryTask, String>),
    CancelCreateTask,

    // === Edit Task Form ===
    OpenEditTask(String), // task_id
    EditTaskTitle(String),
    EditTaskDescription(String),
    EditTaskPriority(TaskPriority),
    EditTaskOwner(String),
    EditTaskDueDate(String),
    EditTaskTags(String),
    SubmitEditTask,
    CancelEditTask,

    // === Workers ===
    RunnersLoaded(Result<Vec<RunnerConfig>, String>),  // Available runners from config
    WorkersLoaded(Result<Vec<Worker>, String>),
    RefreshWorkers,
    SelectWorker(String),
    QuickStartRunner(String),         // runner_name -> runs `granary worker start --runner=<name> -d`
    RestartWorker(String),            // worker_id
    StopWorker(String),
    DeleteWorker(String),
    WorkerUpdated(Result<(), String>),

    // === Start Worker Form (Customize) ===
    OpenCustomizeRunner(String),      // runner_name -> pre-populates form with runner config
    OpenCustomWorker,                 // blank form for inline command
    StartWorkerCommand(String),
    StartWorkerArgs(String),
    StartWorkerEventType(String),
    StartWorkerFilters(String),
    StartWorkerConcurrency(i32),
    StartWorkerCooldown(i64),
    StartWorkerDetached(bool),
    ClearRunnerBase,                  // Clear runner reference to fully customize
    SubmitStartWorker,
    WorkerStarted(Result<Worker, String>),
    CancelStartWorker,

    // === Runs ===
    RunsLoaded(Result<Vec<Run>, String>),
    RefreshRuns,
    SelectRun(String),
    StopRun(String),
    PauseRun(String),
    ResumeRun(String),
    RunUpdated(Result<(), String>),

    // === Logs ===
    OpenWorkerLogs(String),
    OpenRunLogs(String),
    LogsLoaded(Result<Vec<String>, String>),
    LogsAppended(Vec<String>),
    ToggleLogFollow,
    CloseLogs,

    // === Error Handling ===
    ClearError,
    ShowError(String),
}
```

### 12. Dependencies

`crates/silo/Cargo.toml`:

```toml
[dependencies]
iced = { version = "0.13", features = ["tokio", "canvas"] }
rfd = "0.15"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
granary-types = { path = "../granary-types" }
chrono = { version = "0.4", features = ["serde"] }
dirs = "5"
petgraph = "0.6"
```

### 13. Future Considerations

Beyond this RFC scope, consider for future iterations:

1. **Session Management** - View and switch between granary sessions
2. **Checkpoint/Restore** - Visual checkpoint management
3. **Real-time Events** - WebSocket or file-watch for instant updates
4. **Keyboard Shortcuts** - Power user navigation
5. **Search** - Global search across all entities
6. **Theming** - Light mode option
7. **Multi-workspace** - Side-by-side workspace comparison

## Appendix A: CLI Commands Reference

### Commands for GUI Integration

#### Initiatives

| Feature | CLI Command | JSON Output |
|---------|-------------|-------------|
| List initiatives | `granary initiatives --json` | `Vec<Initiative>` |
| Initiative detail | `granary initiative <id> summary --json` | `InitiativeSummary` |
| Archive initiative | `granary initiative <id> archive` | - |

#### Projects

| Feature | CLI Command | JSON Output |
|---------|-------------|-------------|
| List projects | `granary projects --json` | `Vec<Project>` |
| Create project | `granary projects create <name> --json` | `Project` |
| Archive project | `granary project <id> archive` | - |
| Update project | `granary project <id> update --status active` | - |

#### Tasks

| Feature | CLI Command | JSON Output |
|---------|-------------|-------------|
| List tasks | `granary project <id> tasks --json` | `Vec<Task>` |
| Create task | `granary project <id> tasks create <title>` | - |
| Update task | `granary task <id> update ...` | - |
| Start task | `granary task <id> start` | - |
| Complete task | `granary task <id> done` | - |
| Block task | `granary task <id> block <reason>` | - |
| Add dependency | `granary task <id> deps add <dep_id>` | - |

#### Workers

| Feature | CLI Command | JSON Output |
|---------|-------------|-------------|
| List workers | `granary workers [--all] --json` | `Vec<Worker>` |
| Start worker (runner) | `granary worker start --runner <name> --on <event>` | `Worker` |
| Start worker (inline) | `granary worker start --command <cmd> --arg <arg> --on <event>` | `Worker` |
| Worker status | `granary worker status <worker_id>` | `Worker` |
| Worker logs | `granary worker logs <worker_id> [-f] [-n <lines>]` | - |
| Stop worker | `granary worker stop <worker_id> [--runs]` | - |
| Prune workers | `granary worker prune` | - |

#### Runs

| Feature | CLI Command | JSON Output |
|---------|-------------|-------------|
| List runs | `granary runs [--worker <id>] [--status <s>] [--all] --json` | `Vec<Run>` |
| Run status | `granary run status <run_id>` | `Run` |
| Run logs | `granary run logs <run_id> [-f] [-n <lines>]` | - |
| Stop run | `granary run stop <run_id>` | - |
| Pause run | `granary run pause <run_id>` | - |
| Resume run | `granary run resume <run_id>` | - |

#### Config

| Feature | CLI Command | JSON Output |
|---------|-------------|-------------|
| List runners | `granary config runners` | - |
| Add runner | `granary config runners add <name> --command <cmd> --on <event>` | - |
| Update runner | `granary config runners update <name> ...` | - |
| Remove runner | `granary config runners rm <name>` | - |
| Show runner | `granary config runners show <name>` | `RunnerConfig` |
| Get config | `granary config get <key>` | - |
| Set config | `granary config set <key> <value>` | - |
| List config | `granary config list` | - |
| Delete config | `granary config delete <key>` | - |
| List steering | `granary steering list` | `Vec<SteeringFile>` |
| Add steering | `granary steering add <path> --mode <mode>` | - |
| Remove steering | `granary steering rm <path>` | - |

## Appendix B: Type Definitions

Key types from `granary-types` that the GUI will use:

```rust
// Initiative
pub struct Initiative {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub status: String,  // "active" | "archived"
    pub tags: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// Project
pub struct Project {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub status: String,  // "active" | "archived"
    pub tags: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// Task
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,  // "draft" | "todo" | "in_progress" | "done" | "blocked"
    pub priority: String,  // "P0" - "P4"
    pub owner: Option<String>,
    pub tags: Option<String>,
    pub blocked_reason: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub due_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// TaskStatus enum
pub enum TaskStatus {
    Draft,
    Todo,
    InProgress,
    Done,
    Blocked,
}

// TaskPriority enum
pub enum TaskPriority {
    P0, P1, P2, P3, P4,
}

// Worker
pub struct Worker {
    pub id: String,
    pub runner_name: Option<String>,
    pub command: String,
    pub args: String,              // JSON array
    pub event_type: String,
    pub filters: String,           // JSON array
    pub concurrency: i32,
    pub instance_path: String,
    pub status: String,            // "pending" | "running" | "stopped" | "error"
    pub error_message: Option<String>,
    pub pid: Option<i64>,
    pub detached: bool,
    pub created_at: String,
    pub updated_at: String,
    pub stopped_at: Option<String>,
    pub poll_cooldown_secs: i64,
    pub last_event_id: i64,
}

// WorkerStatus enum
pub enum WorkerStatus {
    Pending,
    Running,
    Stopped,
    Error,
}

// Run
pub struct Run {
    pub id: String,
    pub worker_id: String,
    pub event_id: i64,
    pub event_type: String,
    pub entity_id: String,
    pub command: String,
    pub args: String,              // JSON array
    pub status: String,            // "pending" | "running" | "completed" | "failed" | "paused" | "cancelled"
    pub exit_code: Option<i32>,
    pub error_message: Option<String>,
    pub attempt: i32,
    pub max_attempts: i32,
    pub next_retry_at: Option<String>,
    pub pid: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// RunStatus enum
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Paused,
    Cancelled,
}

// RunnerConfig (from global config)
pub struct RunnerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub concurrency: Option<u32>,
    pub on: Option<String>,
    pub env: HashMap<String, String>,
}
```
