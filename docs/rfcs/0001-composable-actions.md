# RFC 0001: Composable Actions (Pipelines)

## Problem

Actions today are single-command definitions. Real workflows require chaining
multiple actions in sequence, with outputs from earlier steps feeding into later
ones. There's no way to express "create a git worktree, then run Claude in that
worktree, then send a notification" as a single deployable unit.

Users currently work around this by writing shell scripts that glue commands
together, which defeats the purpose of the actions system (reusability,
registry, discoverability, per-step retry/logging).

## Design

Extend `ActionConfig` with an optional `steps` field. When present, the action
is a **pipeline** - an ordered sequence of steps executed serially. Each step
either references an existing standalone action or defines an inline command.

Key principle: **an action with `steps` is a pipeline, an action with `command`
is a simple action.** They are mutually exclusive. All simple actions continue to
work unchanged.

### Registry Actions as Building Blocks

Simple actions from the registry are first-class pipeline building blocks. A
step with `action = "worktree-create"` resolves the installed action and
executes it as that step. This means the registry naturally evolves into a
library of composable primitives - small, focused actions that do one thing well
and can be wired together into pipelines.

Users install the pieces they need, then compose them:

```bash
granary action install git/worktree-create
granary action install agents/claude-work
granary action install notify/macos
# then write a 10-line pipeline TOML that wires them together
```

### Namespaced Actions

Action names support `/` as a namespace separator. This maps directly to
directory structure on disk and in the registry:

```
~/.granary/actions/
  git/
    worktree-create.toml
    worktree-remove.toml
    branch.toml
    diff.toml
  agents/
    claude-work.toml
    codex-work.toml
    cursor-work.toml
    opencode-work.toml
  notify/
    macos.toml
    slack.toml
```

Registry layout mirrors this exactly:

```
actions/
  git/worktree-create.toml
  agents/claude-work.toml
  notify/macos.toml
```

Namespaces are purely organizational - no special semantics. The full name
including the namespace is the action's identity everywhere:

```bash
granary action install git/worktree-create
granary action show agents/claude-work
granary action run notify/macos --set id=task-1
```

```toml
[[steps]]
action = "git/worktree-create"

[[steps]]
action = "agents/claude-work"
cwd = "{steps.git/worktree-create.stdout}"
```

#### Implementation

Trivial - the action name is already used as a path component:

- **`load_action(name)`**: `actions_dir.join(format!("{name}.toml"))` already
  handles `/` correctly since it becomes a path separator
- **`install`**: create parent directories with `create_dir_all` before writing
- **`list_all_actions()`**: walk `actions_dir` recursively instead of flat
  listing, derive name from relative path
- **`remove`**: delete file, then remove empty parent dirs
- **Inline config**: TOML quoted keys handle `/` fine:
  `[actions."git/worktree-create"]`
- **Templates**: `{steps.git/worktree-create.stdout}` parses unambiguously since
  `.` is the only field separator

#### Backwards Compatibility

Existing flat action names (`claude-work`, `slack-message`) continue to work -
they're just actions with no namespace. Migration to namespaces is opt-in.
The registry can reorganize files into namespaces and add redirects or aliases
for the old flat names during a transition period.

### Output Passing

Each step's **stdout** is captured and made available to subsequent steps via
template variables:

- `{steps.<name>.stdout}` - trimmed stdout of a named step
- `{steps.<name>.exit_code}` - exit code of a named step
- `{prev.stdout}` - shorthand for the immediately preceding step's stdout
- `{prev.exit_code}` - shorthand for the immediately preceding step's exit code

Stderr continues to go to the run's log file (for debugging). Stdout is *also*
appended to the log file (prefixed with the step name) so nothing is lost.

This design keeps actions **pipeline-unaware** - a step that prints a path to
stdout works identically whether run standalone or as part of a pipeline.

### Error Handling

Default: pipeline **stops on the first non-zero exit code**. The run is marked
failed and retries (if configured) restart the entire pipeline.

Per-step override via `on_error`:

| Value      | Behavior                                        |
|------------|-------------------------------------------------|
| `stop`     | Stop pipeline, mark run as failed (default)     |
| `continue` | Record failure, continue to next step           |

When `on_error = "continue"`, subsequent steps still receive the failed step's
exit code and stdout (which may be empty/partial).

## Configuration Format

### Pipeline Action (new)

```toml
# ~/.granary/actions/worktree-claude.toml
description = "Create isolated worktree, run Claude, notify on completion"
on = "task.next"
concurrency = 2

[[steps]]
action = "git/worktree-create"

[[steps]]
action = "agents/claude-work"
cwd = "{steps.git/worktree-create.stdout}"

[[steps]]
action = "notify/macos"
on_error = "continue"
```

Three registry actions, composed into a pipeline with zero inline commands.
Step names are auto-derived from the action names.

### Simple Action (unchanged)

```toml
# ~/.granary/actions/agents/claude-work.toml
description = "Run a task using Claude Code"
command = "claude"
args = ["-p", "$(granary work start {id})", "--output-format", "stream-json"]
concurrency = 3
on = "task.next"

[env]
```

Same file. Same format. The `agents/claude-work` action above is used standalone
*and* referenced from the pipeline. Zero changes required.

### Step Fields

| Field      | Type              | Required | Description                                                        |
|------------|-------------------|----------|--------------------------------------------------------------------|
| `name`     | string            | no       | Step identifier for output references. Defaults to `action` value when set. |
| `action`   | string            | no*      | Reference to an existing action by name                            |
| `command`  | string            | no*      | Inline command (mutually exclusive with `action`)                  |
| `args`     | string[]          | no       | Arguments (overrides action's args if both present)                |
| `env`      | map               | no       | Additional env vars (merged with action's env, step wins)          |
| `cwd`      | string            | no       | Working directory override (supports pipeline templates)           |
| `on_error` | `stop`/`continue` | no       | Error handling for this step (default: `stop`)                     |

\* Exactly one of `action` or `command` must be set.

### Step Name Resolution

`name` is optional. When omitted:
- If `action` is set, `name` defaults to the action name as-is:
  `action = "git/worktree-create"` → name `git/worktree-create`
- If `command` is set, `name` defaults to `step_N` where N is the 1-based index

This means `{steps.git/worktree-create.stdout}` works without ever writing an
explicit `name`. The template parser uses `.` as the only field separator, so
`/` and `-` in names are unambiguous.

Names must be unique within a pipeline. Duplicate names (e.g. two steps
referencing the same action) are a validation error - use explicit `name` to
disambiguate:

```toml
[[steps]]
name = "plan"
action = "agents/claude-work"

[[steps]]
name = "implement"
action = "agents/claude-work"
```

When a step references an `action`, the step's fields override the action's
fields using the same merge logic as runner-action merging today: step env is
merged (step wins on conflict), empty step args fall through to action args,
step command overrides action command.

### Pipeline-Level Fields

These fields live at the top level of the TOML file alongside `[[steps]]`:

| Field         | Type   | Description                                      |
|---------------|--------|--------------------------------------------------|
| `description` | string | Human-readable description                       |
| `on`          | string | Default event type (same as simple actions)      |
| `concurrency` | u32    | Max concurrent pipeline executions               |
| `env`         | map    | Env vars inherited by all steps                  |

## Template Extensions

The existing template system (`src/services/template.rs`) gains a new context
source: **pipeline step outputs**.

### Current Templates (unchanged)

```
{id}                 - top-level payload field
{event.id}           - event ID
{event.type}         - event type
{event.entity_id}    - entity ID
{task.field}         - nested payload lookup
```

### New Pipeline Templates

```
{steps.<name>.stdout}    - captured stdout of step <name> (trimmed)
{steps.<name>.exit_code} - exit code of step <name>
{prev.stdout}            - stdout of the immediately preceding step
{prev.exit_code}         - exit code of the immediately preceding step
```

Pipeline templates are only resolved during pipeline execution. If used in a
standalone action, they resolve to empty string (same as any unknown
placeholder today).

## Runtime Execution Model

### Current Flow (single action)

```
Event → Worker → spawn_runner_with_env(command, args, cwd, env) → monitor
```

### Pipeline Flow (new)

```
Event → Worker → detect pipeline →
  for each step:
    1. Resolve action reference (if step.action is set)
    2. Merge step overrides with resolved action
    3. Expand templates (event + pipeline context)
    4. Spawn process: stdout=piped, stderr=log_file
    5. Wait for completion
    6. Capture stdout, store in pipeline context
    7. Check exit code vs on_error policy
    8. Continue or stop
  → mark run as completed/failed
```

### Process Spawning Changes

`src/services/runner.rs` needs a new spawn variant for pipeline steps:

```rust
pub async fn spawn_runner_piped(
    command: &str,
    args: &[String],
    working_dir: &Path,
    env_vars: &[(String, String)],
    log_writer: &mut impl Write,  // for stderr + stdout echo
) -> Result<PipelineStepHandle>
```

Key difference from `spawn_runner_with_env`: stdout is `Stdio::piped()` instead
of going directly to the log file. The caller reads stdout to completion, writes
it to the log (prefixed), and stores the trimmed value.

```rust
pub struct PipelineStepHandle {
    child: Child,
    pid: u32,
    stdout: tokio::process::ChildStdout,
}

pub struct StepOutput {
    pub stdout: String,    // trimmed
    pub exit_code: i32,
}
```

### Pipeline Context

```rust
/// Accumulated outputs from completed pipeline steps.
pub struct PipelineContext {
    outputs: HashMap<String, StepOutput>,
    last_step: Option<String>,  // name of most recently completed step
}

impl PipelineContext {
    pub fn resolve(&self, path: &str) -> Option<String> {
        if let Some(rest) = path.strip_prefix("steps.") {
            // Step names may contain `/` and `-`, so we can't naively
            // split on `.`. Instead, strip known suffixes from the right.
            if let Some(name) = rest.strip_suffix(".stdout") {
                return self.outputs.get(name).map(|o| o.stdout.clone());
            }
            if let Some(name) = rest.strip_suffix(".exit_code") {
                return self.outputs.get(name).map(|o| o.exit_code.to_string());
            }
            None
        } else if let Some(field) = path.strip_prefix("prev.") {
            let last = self.last_step.as_ref()?;
            let output = self.outputs.get(last)?;
            match field {
                "stdout" => Some(output.stdout.clone()),
                "exit_code" => Some(output.exit_code.to_string()),
                _ => None,
            }
        } else {
            None
        }
    }
}
```

The resolver strips known suffixes (`.stdout`, `.exit_code`) from the end,
treating everything between `steps.` and the suffix as the step name. This
handles names with `/`, `-`, or any other character cleanly.

### Template Substitution Extension

`template::substitute` gains an optional `PipelineContext` parameter:

```rust
pub fn substitute_with_context(
    template: &str,
    event: &Event,
    pipeline_ctx: Option<&PipelineContext>,
) -> Result<String>
```

Pipeline templates are checked first (so `{steps.x.stdout}` can't collide with
a payload field called `steps`). The existing `substitute()` function becomes a
thin wrapper that passes `None` for the pipeline context, keeping all existing
call sites unchanged.

## Worker Runtime Changes

In `WorkerRuntime::handle_event`, the runtime needs to determine whether the
worker's backing action is a pipeline or simple action:

```rust
async fn handle_event(&mut self, event: Event) -> Result<()> {
    if self.is_pipeline() {
        self.execute_pipeline(event).await
    } else {
        self.execute_single(event).await  // existing logic, unchanged
    }
}
```

The `Worker` model gains an `is_pipeline` flag (or the runtime loads the action
config at startup and caches it). The simplest approach: store the serialized
steps in the worker's `args` field as a JSON-encoded pipeline descriptor, or add
a new column.

**Recommended approach**: add a `pipeline_steps` column to the `workers` table
(TEXT, default NULL, JSON-encoded). When non-null, the worker executes as a
pipeline. The existing `command`/`args` columns are unused for pipelines but
remain populated (with the first step's command) for backwards-compatible display
in `worker list`.

```sql
-- migrations/YYYYMMDD_pipeline_steps.sql
ALTER TABLE workers ADD COLUMN pipeline_steps TEXT DEFAULT NULL;
```

### Run Tracking

Each pipeline execution is still a single `Run` record (one event = one run).
Individual step outcomes are logged to the run's log file with clear delimiters:

```
=== [step:git/worktree-create] started ===
/tmp/granary-proj-abc1-task-3
=== [step:git/worktree-create] exit_code=0 ===

=== [step:agents/claude-work] started cwd=/tmp/granary-proj-abc1-task-3 ===
... claude output ...
=== [step:agents/claude-work] exit_code=0 ===

=== [step:notify/macos] started ===
=== [step:notify/macos] exit_code=0 ===
```

This keeps the data model simple (no new tables) while providing full
visibility into pipeline execution.

## CLI Changes

### `granary action add` (extended)

No changes needed. Pipeline actions are TOML files with `[[steps]]`. The
existing `action add` with inline config doesn't need to support pipelines -
pipelines are defined in files (too complex for CLI flags).

### `granary action show` (extended)

Implements `Output` trait. All formats support pipeline display.

**Text** (default):
```
$ granary action show worktree-claude

  Name:        worktree-claude
  Description: Create isolated worktree, run Claude, notify on completion
  Event:       task.next
  Concurrency: 2
  Type:        pipeline (3 steps)

  Steps:
    1. git/worktree-create   [action]
    2. agents/claude-work    [action]  cwd={steps.git/worktree-create.stdout}
    3. notify/macos          [action]  on_error=continue
```

**JSON** (`--format json`):
```json
{
  "name": "worktree-claude",
  "description": "Create isolated worktree, run Claude, notify on completion",
  "on": "task.next",
  "concurrency": 2,
  "type": "pipeline",
  "steps": [
    { "name": "git/worktree-create", "action": "git/worktree-create" },
    { "name": "agents/claude-work", "action": "agents/claude-work", "cwd": "{steps.git/worktree-create.stdout}" },
    { "name": "notify/macos", "action": "notify/macos", "on_error": "continue" }
  ]
}
```

**Prompt** (`--format prompt`):
```
Action "worktree-claude" is a pipeline with 3 steps triggered on task.next (concurrency: 2).
Steps:
1. git/worktree-create - runs action git/worktree-create
2. agents/claude-work - runs action agents/claude-work with cwd={steps.git/worktree-create.stdout}
3. notify/macos - runs action notify/macos (on_error: continue)
```

### `granary action run` (new subcommand)

One-shot execution of an action outside the worker/event system. Useful for
testing pipelines:

```bash
# Run a pipeline action directly with mock event data
granary action run worktree-claude --set id=proj-abc1-task-5

# Run a simple action the same way
granary action run agents/claude-work --set id=proj-abc1-task-5
```

Flags:
- `--set key=value` - set template variables (repeatable)
- `--cwd <path>` - override working directory
- `--dry-run` - print resolved commands without executing

Implements `Output` trait. Result contains per-step outcomes for pipelines.

**Text** (default):
```
$ granary action run worktree-claude --set id=proj-abc1-task-5

  Step 1/3  git/worktree-create    ok   /tmp/granary-proj-abc1-task-5
  Step 2/3  agents/claude-work     ok   (1024 bytes)
  Step 3/3  notify/macos           ok

  Pipeline completed (3/3 steps succeeded)
```

**JSON** (`--format json`):
```json
{
  "action": "worktree-claude",
  "status": "completed",
  "steps": [
    { "name": "git/worktree-create", "exit_code": 0, "stdout": "/tmp/granary-proj-abc1-task-5" },
    { "name": "agents/claude-work", "exit_code": 0, "stdout_bytes": 1024 },
    { "name": "notify/macos", "exit_code": 0, "stdout": "" }
  ]
}
```

**Prompt** (`--format prompt`):
```
Pipeline "worktree-claude" completed successfully. All 3 steps passed.
Step outputs: git/worktree-create produced "/tmp/granary-proj-abc1-task-5".
```

This is useful beyond pipelines (testing any action) but is essential for
pipeline development where the feedback loop needs to be tight.

### `granary worker start` (unchanged)

```bash
# Works exactly as today - the runtime detects pipeline actions automatically
granary worker start --action worktree-claude --on task.next
```

No new flags needed. The worker start flow loads the action, sees `steps`,
stores the pipeline config in the worker record, and the runtime handles the
rest.

## Type Changes

### `ActionConfig` (extended)

```rust
pub struct ActionConfig {
    pub description: Option<String>,
    pub command: Option<String>,              // now Optional (was required)
    pub args: Vec<String>,
    pub concurrency: Option<u32>,
    pub on: Option<String>,
    pub env: HashMap<String, String>,
    pub action: Option<String>,
    pub steps: Option<Vec<StepConfig>>,       // NEW
}
```

`command` becomes `Option<String>`. Validation ensures exactly one of `command`
or `steps` is set. For backwards compatibility, deserialization accepts both
`command: String` and `command: Option<String>` (serde handles this naturally
with `#[serde(default)]`).

### `StepConfig` (new)

```rust
pub struct StepConfig {
    pub name: Option<String>,               // auto-derived when None
    pub action: Option<String>,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub cwd: Option<String>,
    pub on_error: Option<OnError>,
}

pub enum OnError {
    Stop,
    Continue,
}

impl StepConfig {
    /// Resolve the effective name for this step.
    /// Priority: explicit name > action name > step_N
    pub fn resolved_name(&self, index: usize) -> String {
        if let Some(name) = &self.name {
            name.clone()
        } else if let Some(action) = &self.action {
            action.clone()
        } else {
            format!("step_{}", index + 1)
        }
    }
}
```

## Examples

### 1. Pure Registry Composition

Install building blocks, compose locally:

```bash
granary action install git/worktree-create
granary action install agents/claude-work
granary action install notify/macos
```

```toml
# ~/.granary/actions/worktree-claude.toml
description = "Isolated worktree workflow with Claude"
on = "task.next"
concurrency = 2

[[steps]]
action = "git/worktree-create"

[[steps]]
action = "agents/claude-work"
cwd = "{steps.git/worktree-create.stdout}"

[[steps]]
action = "notify/macos"
on_error = "continue"
```

### 2. Mixed Registry + Inline Steps

```toml
# ~/.granary/actions/worktree-claude-cleanup.toml
description = "Worktree workflow with cleanup"
on = "task.next"

[[steps]]
action = "git/worktree-create"

[[steps]]
action = "agents/claude-work"
cwd = "{steps.git/worktree-create.stdout}"

[[steps]]
name = "cleanup"
command = "git"
args = ["worktree", "remove", "--force", "{steps.git/worktree-create.stdout}"]
on_error = "continue"

[[steps]]
action = "notify/macos"
on_error = "continue"
```

### 3. Code Review Pipeline

```toml
# ~/.granary/actions/review-pipeline.toml
description = "Generate diff, review with Claude, post to Slack"
on = "task.completed"

[[steps]]
action = "git/diff"

[[steps]]
name = "review"
command = "claude"
args = [
    "-p",
    "Review this diff and provide a summary:\n\n{prev.stdout}",
    "--output-format", "text",
]

[[steps]]
action = "notify/slack"

[steps.env]
SLACK_MESSAGE = "{steps.review.stdout}"
```

### 4. Multi-Agent Workflow

```toml
# ~/.granary/actions/multi-agent.toml
description = "Plan with one model, implement with another"
on = "task.next"
concurrency = 1

[[steps]]
name = "plan"
command = "claude"
args = [
    "-p",
    "$(granary work start {id})\n\nCreate a detailed implementation plan. Output ONLY the plan, no code.",
    "--model", "opus",
    "--output-format", "text",
]

[[steps]]
name = "implement"
command = "claude"
args = [
    "-p",
    "Implement the following plan:\n\n{steps.plan.stdout}",
    "--model", "sonnet",
    "--dangerously-skip-permissions",
    "--output-format", "stream-json",
]
```

### 5. Conditional Notification (using on_error)

```toml
# ~/.granary/actions/work-and-notify.toml
description = "Run Claude, notify on success or failure differently"
on = "task.next"

[[steps]]
action = "agents/claude-work"
on_error = "continue"

[[steps]]
name = "notify"
command = "sh"
args = [
    "-c",
    "if [ '{steps.agents/claude-work.exit_code}' = '0' ]; then echo 'Task {id} succeeded' | notify; else echo 'Task {id} FAILED' | notify; fi"
]
```

### 6. Pipeline in config.toml (inline)

```toml
# ~/.granary/config.toml

[actions.my-workflow]
description = "Inline pipeline"
on = "task.next"

[[actions.my-workflow.steps]]
action = "git/worktree-create"

[[actions.my-workflow.steps]]
action = "agents/claude-work"
cwd = "{prev.stdout}"
```

## Backwards Compatibility

| Concern | Impact |
|---------|--------|
| `ActionConfig.command` becomes `Option<String>` | Existing configs always have `command` set, so `Some(command)` round-trips identically. Serde default handles missing field. |
| New `steps` field on `ActionConfig` | `#[serde(default)]` means existing TOML files without `steps` deserialize to `None`. Zero breakage. |
| New `pipeline_steps` column on `workers` table | `ALTER TABLE ADD COLUMN ... DEFAULT NULL`. Existing workers have NULL, behave exactly as before. |
| Worker runtime dispatch | Simple actions take the existing code path. Pipeline path is new code, only reached when `pipeline_steps` is non-null. |
| Registry | Pipeline action files are just TOML with `[[steps]]`. `granary action install` already handles arbitrary TOML. Namespaced paths are new but the registry hasn't shipped yet, so no migration needed. |
| Runners referencing actions | Runners that reference a pipeline action inherit the pipeline. The runner's command/args overrides are ignored (a runner can't partially override a pipeline). This is validated at `worker start` time with a clear error if a runner tries to set command/args while referencing a pipeline action. |

## Registry Building Blocks

The registry should grow a set of small, composable actions designed to be
pipeline steps. Each one does exactly one thing and communicates its result via
stdout.

### Convention

Actions intended as pipeline building blocks should:
1. Print their primary output to **stdout** (a path, a URL, a summary)
2. Send progress/debug info to **stderr** (so it lands in logs but doesn't
   pollute the output channel)
3. Exit 0 on success, non-zero on failure

This convention works naturally - actions following it are usable both standalone
(`granary action run git/worktree-create --set id=task-1`) and as pipeline steps.

### Proposed Registry Layout

```
actions/
  git/
    worktree-create.toml    # stdout: worktree path
    worktree-remove.toml    # stdin: path to remove
    diff.toml               # stdout: diff output
    branch.toml             # stdout: branch name
  agents/
    claude-work.toml        # existing, moved from flat
    codex-work.toml         # existing, moved from flat
    cursor-work.toml        # existing, moved from flat
    opencode-work.toml      # existing, moved from flat
  notify/
    macos.toml              # macOS system notification
    slack.toml              # post to Slack webhook
```

## Implementation Order

1. Add namespaced action support (`load_action`, `list_all_actions`, `install`, `remove`)
2. Reorganize existing registry actions into namespaces (`git/`, `agents/`, `notify/`)
3. Add `StepConfig` and `steps` to `ActionConfig` in `granary-types` with validation
4. Add `PipelineContext` and extend template substitution
5. Add `spawn_runner_piped` to `runner.rs`
6. Add pipeline execution logic to `worker_runtime.rs`
7. Add `pipeline_steps` migration and wire through worker start flow
8. Extend `action show` for pipeline display
9. Add `action run` subcommand
10. Add pipeline-focused building-block actions to registry (`git/worktree-create`, `notify/macos`, etc.)
