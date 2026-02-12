# Granary

Shared memory and coordination for AI coding agents. Single binary, local-first, no cloud.

```sh
# Install
curl -sSfL https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.sh | sh

# Init + plan + watch
granary init
claude -p "use granary to plan: Migrate endpoints to v2" & granary summary --watch
```

## Getting Started

### Install

**macOS / Linux:**

```sh
curl -sSfL https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.sh | sh
```

**Windows (PowerShell):**

```powershell
irm https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.ps1 | iex
```

**From source** (requires [Rust](https://rustup.rs/)):

```sh
cargo install --git https://github.com/speakeasy-api/granary.git
```

### LLM-First workflow

```sh
granary init
granary plan "User authentication"
granary next
granary work <task-id>
granary task <task-id> done
granary summary --format prompt
```

`plan` creates a project and breaks it into tasks. `work` claims a task and gives your agent full context. `summary` produces an LLM-optimized handoff of everything that happened.

## Runners & Workers

Runners are reusable command configs. Workers subscribe to events and spawn runners automatically. This is the core automation loop — plan work, then let workers drive execution.

### 1. Configure a runner

```sh
granary config runners add claude-implementer \
  --command "claude" \
  --arg "-p" \
  --arg "$(granary work start {event.entity_id})" \
  --arg "--allowedTools" \
  --arg "Bash,Read,Write,Edit,Glob,Grep" \
  --concurrency 3
```

### 2. Start a worker

```sh
granary worker start --runner claude-implementer --on task.next
```

Now every time a task is available, a Claude Code session is spawned with the task context piped in. Add filters to narrow scope:

```sh
granary worker start --runner claude-implementer --on task.next --filter "priority=P0"
```

### 3. Monitor

```sh
granary workers              # List active workers
granary runs                 # List runner executions
granary runs --watch         # Live-updating view
granary run <run-id> logs    # Inspect a specific run
```

Runner args support `{event.entity_id}`, `{id}`, `{title}`, `{project_id}`, and other event payload fields. See [docs/workers.md](docs/workers.md) for the full reference on events, filters, template substitution, retry behavior, and concurrency control.

## Why Granary?

Agents don't coordinate well on their own. Without shared infrastructure they lose context between sessions, duplicate work, and create silent conflicts.

- **Session-centric context** — explicit "what's in context" for each agent run, so nothing is lost between handoffs
- **Lossless planning** — agents can clear their working context freely; granary persists decisions and progress for the next agent
- **Concurrency safety** — task claiming with leases prevents multiple agents from colliding on the same work
- **LLM-native commands** — `plan`, `work`, and `initiate` bundle multiple operations into single calls, reducing tool invocations
- **Event-driven automation** — workers react to state changes and spawn agent sessions without human intervention

## CLI Workflows

### Planning

```sh
granary plan "Audit service"
granary initiate "Q1 Platform Migration"
```

### Task management

```sh
granary project <project-id> tasks create "Implement login" --priority P0
granary next
granary start <task-id>
granary search "auth"
```

### Context & handoffs

```sh
granary context --format prompt --token-budget 2000
granary handoff --to "Code Review Agent" --tasks task-1,task-2
granary summary
```

## Output Formats

Every command supports multiple output formats:

```sh
granary tasks                    # Human-readable table
granary tasks --json             # JSON for parsing
granary tasks --format prompt    # Optimized for LLM context
granary tasks --format yaml      # YAML
granary tasks --format md        # Markdown
```

## Watch Mode

```sh
granary tasks --watch
granary workers --watch --interval 5
granary runs --watch --status running
```

Supported commands: `tasks`, `projects`, `workers`, `runs`, `sessions`, `initiatives`, `search`, `summary`.

## Key Concepts

| Concept       | Description                                                           |
| ------------- | --------------------------------------------------------------------- |
| **Workspace** | A directory (typically a repo) containing `.granary/`                 |
| **Project**   | Long-lived initiative with tasks and steering references              |
| **Task**      | Unit of work with status, priority, dependencies, and claiming        |
| **Runner**    | A reusable command configuration (stored in `~/.granary/config.toml`) |
| **Worker**    | A process that subscribes to events and spawns runners                |
| **Session**   | Container for "what's in context" for an agent run                    |

## Commands

```
granary init              # Initialize workspace
granary plan              # Plan a feature interactively
granary initiate          # Plan a multi-project initiative
granary work <task-id>    # Claim and work on a task
granary next              # Get next actionable task
granary start <id>        # Start working on a task
granary search            # Search projects and tasks
granary summary           # Generate work summary
granary context           # Export context pack for LLM
granary handoff           # Generate handoff for sub-agent
granary config runners    # Manage runner configurations
granary worker start      # Start an event-driven worker
granary workers           # List all workers
granary runs              # List all runner executions
granary checkpoint        # Create/restore checkpoints
```

Use `granary --help` or `granary <command> --help` for detailed usage.

## License

MIT
