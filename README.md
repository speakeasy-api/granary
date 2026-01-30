# Granary

A CLI context hub for agentic work. Granary supercharges your agentic workflows. It seamlessly integrates into your existing AI tools and teaches them how to share and manage context more efficiently.

## Features

- **Session-centric**: Explicit "what's in context" for each agent run
- **LLM-first I/O**: Every command has `--json` and `--format prompt` for machine consumption
- **Local-first**: All state stored locally (SQLite), no network dependency
- **Concurrency-tolerant**: Task claiming with leases for multi-agent safety
- **Context packs**: Generate summaries and handoffs optimized for LLM context windows

## Getting started for Claude Code

1. Add the granary marketplace

```sh
claude plugin marketplace add speakeasy-api/granary
```

2. Install the granary plugin from the marketplace

```sh
claude plugin install granary@granary
```

3. Launch Claude and verify skills are available with `/skills` - you should see something like:

```sh
  granary-orchestrate · ~43 tokens
  granary-initiative-planner · ~40 tokens
  granary-plan-work · ~38 tokens
  granary-setup · ~33 tokens
  granary-execute-task · ~28 tokens
```

4. Prompt Claude to `set up granary for this project`

Claude will install and initialize `granary` in your project.

## How to get the most out of Granary

Granary works best when used with the Claude Code skills. The skills teach Claude how to use Granary effectively.

### Example workflows

Use similar prompts to see Granary in action.

- `use granary to plan a new audit service`
- Once the plan is complete, you can review it with `granary summary`
- Start implementation by telling Claude: `use granary to implement audit service`

## Installation

### macOS / Linux

```sh
curl -sSfL https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.sh | sh
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.ps1 | iex
```

### Installing a specific version

You can install a specific version (including pre-releases) by setting the `GRANARY_VERSION` environment variable:

**macOS / Linux:**

```sh
GRANARY_VERSION=0.6.2 curl -sSfL https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.sh | sh
```

**Windows (PowerShell):**

```powershell
$env:GRANARY_VERSION='0.6.2'; irm https://raw.githubusercontent.com/speakeasy-api/granary/main/scripts/install.ps1 | iex
```

### Updating

To update to the latest stable version:

```sh
granary update
```

To install a specific version (including pre-releases):

```sh
granary update --to=0.6.3-pre.1
```

### From source

Requires [Rust](https://rustup.rs/):

```sh
cargo install --git https://github.com/speakeasy-api/granary.git
```

## Quick Start

```sh
# Initialize a workspace
granary init

# Create a project
granary projects create "My Project" --description "Building something great"

# Start a session
granary session start "feature-work" --owner "Claude Code"

# Add tasks
granary project my-project-xxxx tasks create "Implement login" --priority P0
granary project my-project-xxxx tasks create "Add tests" --priority P1

# Get the next actionable task
granary next

# Start working on a task
granary start my-project-xxxx-task-1

# Mark it done
granary task my-project-xxxx-task-1 done

# Get a summary for your LLM
granary summary --format prompt
```

## Why Granary?

Granary is designed for the agentic loop pattern:

1. **Plan**: Create projects and tasks, set dependencies
2. **Execute**: Agents claim tasks, work on them, report progress
3. **Coordinate**: Multiple agents can work safely in parallel with leases
4. **Handoff**: Generate context packs for sub-agents or human review

### Key Concepts

- **Workspace**: A directory (typically a repo) containing `.granary/`
- **Project**: Long-lived initiative with tasks and steering references
- **Task**: Unit of work with status, priority, dependencies, and claiming
- **Session**: Container for "what's in context" for a run
- **Checkpoint**: Snapshot of state for pause/resume or rollback

## Commands

```
granary init          # Initialize workspace
granary projects      # List/create projects
granary tasks         # List tasks in session scope
granary next          # Get next actionable task
granary start <id>    # Start working on a task
granary summary       # Generate work summary
granary context       # Export context pack for LLM
granary handoff       # Generate handoff for sub-agent
granary checkpoint    # Create/restore checkpoints
granary search        # Search projects and tasks by title
granary workers       # List all workers
granary worker start  # Start a new event-driven worker
granary runs          # List all runner executions
```

Use `granary --help` or `granary <command> --help` for detailed usage.

## Output Formats

Every command supports multiple output formats:

```sh
granary tasks                    # Human-readable table
granary tasks --json             # JSON for parsing
granary tasks --format yaml      # YAML
granary tasks --format md        # Markdown
granary tasks --format prompt    # Optimized for LLM context

granary search "api"             # Search in human-readable table
granary search "api" --json      # JSON for parsing
```

## Watch Mode

Monitor changes in real-time with `--watch`. The output refreshes automatically at a configurable interval:

```sh
# Watch tasks with default 2-second refresh
granary tasks --watch

# Watch workers with 5-second refresh
granary workers --watch --interval 5

# Watch runs filtered by status
granary runs --watch --status running

# Watch search results
granary search "api" --watch
```

Supported commands: `tasks`, `projects`, `workers`, `runs`, `sessions`, `initiatives`, `search`, `summary`

Press `Ctrl+C` to exit watch mode.

## Integration with Claude Code

Granary works seamlessly with Claude Code and other LLM coding assistants:

```sh
# Set session for sub-agents
eval $(granary session env)

# Generate context for prompts
granary context --format prompt --token-budget 2000

# Handoff to a review agent
granary handoff --to "Code Review Agent" --tasks task-1,task-2
```

## Workers (Event-driven Automation)

Workers are long-running processes that subscribe to granary events and automatically spawn commands. For example, automatically run Claude Code when tasks become unblocked:

```sh
# Configure a runner
granary config runners add claude \
  --command "claude" \
  --arg "--print" \
  --arg "--message" \
  --arg "Execute task {task.id}"

# Start a worker
granary worker start --runner claude --on task.unblocked
```

See [docs/workers.md](docs/workers.md) for complete documentation on workers, runners, filters, and template substitution.

## License

MIT
