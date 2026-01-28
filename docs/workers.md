# Granary Workers

Workers are long-running processes that subscribe to granary events and automatically spawn commands (runners) in response. This enables powerful automation workflows like triggering Claude Code when tasks become unblocked or sending notifications when milestones are reached.

## Architecture

```text
+------------------------------------------------------------+
|                     WorkerRuntime                           |
|                                                             |
|  +-------------+    +--------------+    +---------------+   |
|  | EventPoller |-->>| Run Manager  |-->>| Runner Procs  |   |
|  +-------------+    +--------------+    +---------------+   |
|        |                  |                    |            |
|        v                  v                    v            |
|  +-------------+    +--------------+    +---------------+   |
|  | Workspace DB|    |  Global DB   |    |   Log Files   |   |
|  |   (events)  |    |(workers,runs)|    |               |   |
|  +-------------+    +--------------+    +---------------+   |
+------------------------------------------------------------+
```

**Key components:**

- **Worker**: The controller that polls for events and manages runners
- **Event Poller**: Watches for new events matching the worker's subscription
- **Runner**: A child process spawned to handle a specific event
- **Run**: A record tracking a single runner execution (status, logs, retries)

## Quick Start

### 1. Configure a Runner

Runners are reusable command configurations stored in `~/.granary/config.toml`:

```bash
# Add a runner for Claude Code
granary config runners add claude \
  --command "claude" \
  --arg "--print" \
  --arg "--allowedTools" \
  --arg "Bash,Read,Write,Edit,Glob,Grep" \
  --arg "--message" \
  --arg "Execute granary task {task.id}" \
  --concurrency 2
```

This creates a runner that can execute Claude Code with specific arguments.

### 2. Start a Worker

Start a worker that uses the configured runner:

```bash
# Start a worker that responds to task.unblocked events
granary worker start --runner claude --on task.unblocked

# Or use an inline command
granary worker start \
  --command "echo" \
  --arg "Task {task.id} is now ready!" \
  --on task.unblocked
```

### 3. Monitor Workers

```bash
# List all active workers
granary workers

# List all workers including stopped ones
granary workers --all

# Check worker status
granary worker status worker-abc12345

# View worker logs
granary worker logs worker-abc12345

# Follow logs in real-time
granary worker logs worker-abc12345 -f
```

### 4. Monitor Runs

```bash
# List active runs
granary runs

# List all runs including completed ones
granary runs --all

# Filter by worker
granary runs --worker worker-abc12345

# Filter by status
granary runs --status failed

# Check run details
granary run status run-xyz12345

# View run logs
granary run logs run-xyz12345
```

## CLI Reference

### Worker Commands

#### `granary worker start`

Start a new worker.

```bash
granary worker start [OPTIONS] --on <EVENT_TYPE>
```

**Options:**

| Option | Description |
|--------|-------------|
| `--runner <NAME>` | Use a configured runner by name |
| `--command <CMD>` | Inline command to execute (alternative to --runner) |
| `--arg <ARG>`, `-a <ARG>` | Command arguments (can be repeated) |
| `--on <EVENT_TYPE>` | Event type to subscribe to (required) |
| `--filter <EXPR>`, `-f <EXPR>` | Filter expressions (can be repeated) |
| `--concurrency <N>` | Maximum concurrent runners (default: 1) |
| `--detached`, `-d` | Run in background as daemon |

**Examples:**

```bash
# Using a configured runner
granary worker start --runner claude --on task.unblocked

# Using inline command with filters
granary worker start \
  --command "slack-notify" \
  --arg "{task.title}" \
  --on task.done \
  --filter "task.priority=P0"

# High concurrency for parallel processing
granary worker start --runner claude --on task.unblocked --concurrency 4
```

#### `granary worker status <WORKER_ID>`

Show worker status and run statistics.

```bash
granary worker status worker-abc12345
```

#### `granary worker logs <WORKER_ID>`

View worker logs.

**Options:**

| Option | Description |
|--------|-------------|
| `-f`, `--follow` | Follow log output (like `tail -f`) |
| `-n`, `--lines <N>` | Number of lines to show (default: 50) |

#### `granary worker stop <WORKER_ID>`

Stop a worker.

**Options:**

| Option | Description |
|--------|-------------|
| `--runs` | Also cancel all active runs |

#### `granary worker prune`

Remove stopped/errored workers and clean up their logs.

#### `granary workers`

List all workers.

**Options:**

| Option | Description |
|--------|-------------|
| `--all` | Include stopped/errored workers |

### Run Commands

#### `granary run status <RUN_ID>`

Show run status and details.

#### `granary run logs <RUN_ID>`

View run logs.

**Options:**

| Option | Description |
|--------|-------------|
| `-f`, `--follow` | Follow log output |
| `-n`, `--lines <N>` | Number of lines to show (default: 100) |

#### `granary run stop <RUN_ID>`

Stop a running run (sends SIGTERM, marks as cancelled).

#### `granary run pause <RUN_ID>`

Pause a running run (sends SIGSTOP).

#### `granary run resume <RUN_ID>`

Resume a paused run (sends SIGCONT).

#### `granary runs`

List all runs.

**Options:**

| Option | Description |
|--------|-------------|
| `--worker <ID>` | Filter by worker ID |
| `--status <STATUS>` | Filter by status (pending, running, completed, failed, paused, cancelled) |
| `--all` | Include completed/failed/cancelled runs |
| `--limit <N>` | Maximum number of runs to show (default: 50) |

## Event Types

Workers subscribe to events using the `--on` option. Common event types:

| Event Type | Trigger |
|------------|---------|
| `task.created` | A new task is created |
| `task.started` | A task transitions to `in_progress` |
| `task.done` | A task transitions to `done` |
| `task.blocked` | A task transitions to `blocked` |
| `task.unblocked` | A task transitions from `blocked` to `todo` |
| `task.updated` | Any task field is updated |
| `project.created` | A new project is created |
| `project.archived` | A project is archived |
| `session.started` | A new session begins |
| `session.closed` | A session is closed |

## Filter Syntax

Filters narrow down which events a worker processes.

### Operators

| Operator | Meaning | Example |
|----------|---------|---------|
| `=` | Equals | `task.status=in_progress` |
| `!=` | Not equals | `task.priority!=P4` |
| `~=` | Contains (substring) | `task.title~=api` |

### Nested Fields

Access nested JSON fields using dot notation:

```bash
# Filter by task priority
--filter "task.priority=P0"

# Filter by project name
--filter "project.name=backend-api"

# Combine multiple filters (AND logic)
--filter "task.priority=P0" --filter "task.owner=claude"
```

### Array Indexing

Access array elements by index:

```bash
--filter "items.0.name=first"
```

### Special Values

- Empty string for null/missing: `--filter "task.owner="`
- "null" literal: `--filter "field=null"`

## Template Substitution

Command arguments support placeholder substitution from event payloads.

### Syntax

Use `{path.to.value}` to substitute values:

```bash
--arg "Execute task {task.id}"
--arg "--project={project.id}"
```

### Available Placeholders

**Event-level:**

| Placeholder | Description |
|-------------|-------------|
| `{event.id}` | Event ID |
| `{event.type}` | Event type (e.g., "task.unblocked") |
| `{event.entity_type}` | Entity type (e.g., "task") |
| `{event.entity_id}` | Entity ID |
| `{event.created_at}` | Event timestamp |

**Task fields (when event is task-related):**

| Placeholder | Description |
|-------------|-------------|
| `{task.id}` | Task ID |
| `{task.title}` | Task title |
| `{task.status}` | Task status |
| `{task.priority}` | Task priority |
| `{task.owner}` | Task owner |
| `{task.description}` | Task description |

**Project fields:**

| Placeholder | Description |
|-------------|-------------|
| `{project.id}` | Project ID |
| `{project.name}` | Project name |

**Session fields:**

| Placeholder | Description |
|-------------|-------------|
| `{session.id}` | Session ID |
| `{session.name}` | Session name |

### Unknown Placeholders

Unknown placeholders are replaced with empty strings, allowing graceful handling of optional fields.

## Runner Configuration

Runners are configured in `~/.granary/config.toml`:

```toml
[runners.claude]
command = "claude"
args = ["--print", "--message", "Execute task {task.id}"]
concurrency = 2
on = "task.unblocked"

[runners.slack]
command = "curl"
args = [
  "-X", "POST",
  "-H", "Content-Type: application/json",
  "-d", "{\"text\": \"Task {task.title} completed!\"}",
  "${SLACK_WEBHOOK_URL}"
]
concurrency = 10

[runners.custom-script]
command = "/path/to/script.sh"
args = ["{task.id}", "{project.id}"]
env = { API_KEY = "secret", DEBUG = "true" }
```

### Managing Runners

```bash
# List all configured runners
granary config runners

# Add a new runner
granary config runners add myrunner \
  --command "python" \
  --arg "script.py" \
  --arg "{task.id}"

# Update an existing runner
granary config runners update myrunner --concurrency 4

# Remove a runner
granary config runners rm myrunner

# Show runner details
granary config runners show myrunner
```

### Environment Variable Expansion

Runner args support `${VAR}` syntax for environment variable expansion:

```bash
granary config runners add api-caller \
  --command "curl" \
  --arg "-H" \
  --arg "Authorization: Bearer ${API_TOKEN}"
```

## Retry Behavior

Failed runs are automatically retried with exponential backoff:

- **Default max attempts:** 3
- **Backoff formula:** `base_delay * 2^(attempt-1)` + jitter
- **Default base delay:** 5 seconds
- **Jitter:** 0-25% of calculated delay

Example retry schedule:

| Attempt | Base Delay | With Jitter (approx) |
|---------|------------|---------------------|
| 1 | 5s | 5-6s |
| 2 | 10s | 10-12s |
| 3 | 20s | 20-25s |

Runs that fail all retry attempts are marked as `failed` and no longer retried.

## Concurrency Control

Each worker has a configurable concurrency limit:

```bash
# Single runner at a time (default)
granary worker start --runner claude --on task.unblocked --concurrency 1

# Up to 4 parallel runners
granary worker start --runner claude --on task.unblocked --concurrency 4
```

When the concurrency limit is reached, new events are queued and processed when a slot becomes available.

## Logging

### Log Locations

- **Worker logs:** `~/.granary/logs/{worker_id}/`
- **Run logs:** `~/.granary/logs/{worker_id}/{run_id}.log`

### Log Content

Each run's stdout and stderr are captured to its log file:

```bash
# View run output
granary run logs run-abc12345

# Follow in real-time
granary run logs run-abc12345 -f
```

## Graceful Shutdown

When a worker is stopped:

1. It stops polling for new events
2. Waits up to 30 seconds for active runs to complete
3. If runs don't complete, sends SIGKILL to remaining processes
4. Marks timed-out runs as `cancelled`

## Workspace Detection

Workers are tied to a specific workspace. If the workspace is deleted or becomes unavailable:

1. Worker detects the missing workspace
2. Transitions to `error` state
3. Stops polling and processing

Use `granary worker prune` to clean up workers whose workspaces no longer exist.

## Example Configurations

### Claude Code for Task Execution

```bash
# Configure the runner
granary config runners add claude-tasks \
  --command "claude" \
  --arg "--print" \
  --arg "--allowedTools" \
  --arg "Bash,Read,Write,Edit,Glob,Grep" \
  --arg "--message" \
  --arg "Execute granary task {task.id}. Use /granary:execute-task skill." \
  --concurrency 2

# Start the worker
granary worker start --runner claude-tasks --on task.unblocked
```

### Slack Notifications for P0 Tasks

```bash
# Configure the runner
granary config runners add slack-notify \
  --command "curl" \
  --arg "-X" \
  --arg "POST" \
  --arg "-H" \
  --arg "Content-Type: application/json" \
  --arg "-d" \
  --arg "{\"text\": \"P0 Task Ready: {task.title}\"}" \
  --arg "${SLACK_WEBHOOK_URL}" \
  --concurrency 10

# Start worker with P0 filter
granary worker start \
  --runner slack-notify \
  --on task.unblocked \
  --filter "task.priority=P0"
```

### Custom Script for Code Review

```bash
# Configure the runner
granary config runners add code-review \
  --command "/scripts/trigger-review.sh" \
  --arg "{task.id}" \
  --arg "{project.id}" \
  --env "GITHUB_TOKEN=${GITHUB_TOKEN}" \
  --concurrency 1

# Start worker
granary worker start --runner code-review --on task.done
```

### Multiple Workers for Different Priorities

```bash
# P0 tasks get dedicated high-priority worker
granary worker start \
  --runner claude-tasks \
  --on task.unblocked \
  --filter "task.priority=P0" \
  --concurrency 2

# P1-P2 tasks share a worker
granary worker start \
  --runner claude-tasks \
  --on task.unblocked \
  --filter "task.priority!=P0" \
  --filter "task.priority!=P3" \
  --filter "task.priority!=P4" \
  --concurrency 1
```

## Troubleshooting

### Worker shows as running but process is dead

This can happen if a worker crashes unexpectedly. Fix:

```bash
granary worker prune
```

### Events not being processed

1. Check worker status: `granary worker status <id>`
2. Verify event type matches: `--on task.unblocked`
3. Check filters aren't too restrictive
4. Look at worker logs for errors

### Runs failing immediately

1. Check run logs: `granary run logs <run_id>`
2. Verify the command exists and is executable
3. Check environment variables are set correctly

### High memory usage

Reduce concurrency to limit parallel processes:

```bash
granary worker stop <worker_id>
granary worker start --runner <name> --on <event> --concurrency 1
```

### Workspace not found errors

If a workspace is deleted while workers are running:

```bash
# Clean up orphaned workers
granary worker prune
```
