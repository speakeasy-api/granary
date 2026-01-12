---
name: granary-orchestration
description: Orchestrate sub-agents using granary for task delegation and handoffs. Use when spawning workers, delegating tasks, or coordinating multi-agent workflows.
---

# Granary Orchestration Skill

This skill enables orchestrator agents to spawn and coordinate sub-agents using granary for task delegation, handoffs, and multi-agent workflows.

## The Orchestrator Loop

The core pattern for an orchestrator is to continuously process available tasks:

```bash
while TASK=$(granary next --json) && [ "$TASK" != "null" ]; do
  TASK_ID=$(echo $TASK | jq -r '.task.id')

  # Claim the task
  granary start $TASK_ID --owner "Orchestrator"

  # Generate context for sub-agent
  CONTEXT=$(granary context --format prompt)

  # Spawn sub-agent with the context (implementation depends on your agent framework)
  # Example: claude-code --prompt "$CONTEXT" --session $GRANARY_SESSION

  # Mark task complete when sub-agent finishes
  granary task $TASK_ID done
done
```

### Loop Components

1. **`granary next --json`** - Fetches the next available task based on priority and dependencies
2. **`granary start`** - Claims the task with ownership tracking
3. **`granary context`** - Generates a prompt-ready context including task details and relevant state
4. **`granary task done`** - Marks completion and releases the task

## Handoff Command

Use `granary handoff` to delegate work to another agent with full context:

```bash
granary handoff \
  --to "CodeReviewAgent" \
  --tasks "TASK-123,TASK-124" \
  --constraints "Only modify files in src/api/" \
  --acceptance-criteria "All tests pass, no new linting errors"
```

### Handoff Options

| Option                  | Description                                  |
| ----------------------- | -------------------------------------------- |
| `--to`                  | Target agent identifier                      |
| `--tasks`               | Comma-separated task IDs to delegate         |
| `--constraints`         | Boundaries and limitations for the sub-agent |
| `--acceptance-criteria` | Conditions that must be met for completion   |
| `--context`             | Additional context string to include         |
| `--priority`            | Override priority for handed-off tasks       |
| `--timeout`             | Maximum time allowed for completion          |

### Example: Complex Handoff

```bash
granary handoff \
  --to "ImplementationAgent" \
  --tasks "IMPL-001" \
  --constraints "Do not modify public API signatures" \
  --constraints "Follow existing code style" \
  --acceptance-criteria "Unit tests cover new functionality" \
  --acceptance-criteria "Documentation updated" \
  --context "This is part of the v2.0 refactoring effort"
```

## Sub-Agent Pattern

Sub-agents receive their session context and work within the granary coordination framework.

### Receiving Session Context

Sub-agents receive the session via the `GRANARY_SESSION` environment variable:

```bash
# Sub-agent startup
export GRANARY_SESSION="$1"  # Passed from orchestrator

# Verify session
granary session info
```

### Claiming Tasks with Leases

Sub-agents should claim tasks with time-limited leases to prevent starvation:

```bash
# Claim with a 30-minute lease
granary start TASK-123 --lease 30m --owner "WorkerAgent-$$"

# Extend lease if needed
granary task TASK-123 extend --lease 15m

# Release without completing (if unable to finish)
granary task TASK-123 release --reason "Blocked on external dependency"
```

### Reporting Progress via Comments

Keep the orchestrator informed with progress updates:

```bash
# Add progress comment
granary task TASK-123 comment "Completed initial analysis, starting implementation"

# Add structured progress
granary task TASK-123 comment --json '{"phase": "testing", "progress": 75}'

# Report blockers
granary task TASK-123 comment --type blocker "Waiting for API credentials"
```

### Completing and Releasing Tasks

```bash
# Mark task as done with summary
granary task TASK-123 done --summary "Implemented feature X with 95% test coverage"

# Mark as done with artifacts
granary task TASK-123 done \
  --artifact "src/feature.ts" \
  --artifact "tests/feature.test.ts" \
  --summary "Added feature implementation and tests"

# Mark as failed if unable to complete
granary task TASK-123 fail --reason "Incompatible with current architecture"
```

## Parallel Worker Pattern

Spawn multiple workers that independently claim and process tasks:

### Orchestrator Spawning Workers

```bash
#!/bin/bash
# spawn-workers.sh

NUM_WORKERS=${1:-4}
SESSION=$(granary session create --name "parallel-processing")

for i in $(seq 1 $NUM_WORKERS); do
  # Spawn worker in background
  GRANARY_SESSION=$SESSION worker-agent.sh &
  echo "Spawned worker $i with PID $!"
done

# Wait for all workers
wait
echo "All workers completed"
```

### Worker Process

```bash
#!/bin/bash
# worker-agent.sh

WORKER_ID="Worker-$$"

while true; do
  # Try to claim next available task
  TASK=$(granary next --json --claim --owner "$WORKER_ID" --lease 15m)

  if [ "$TASK" = "null" ] || [ -z "$TASK" ]; then
    echo "[$WORKER_ID] No more tasks available"
    break
  fi

  TASK_ID=$(echo $TASK | jq -r '.task.id')
  echo "[$WORKER_ID] Processing $TASK_ID"

  # Process the task
  CONTEXT=$(granary context --task $TASK_ID --format prompt)

  # ... perform work ...

  # Complete the task
  granary task $TASK_ID done --owner "$WORKER_ID"
done
```

### Load Balancing Considerations

```bash
# Workers can specify capability filters
granary next --json --claim \
  --filter "type=code-review" \
  --filter "language=typescript"

# Or use weighted selection for heterogeneous workers
granary next --json --claim \
  --prefer "priority=high" \
  --prefer "estimated_time<30m"
```

## Task Selection Strategies

### Basic Selection with `granary next`

```bash
# Get next task by priority
granary next

# Get next task as JSON
granary next --json

# Include reasoning for selection
granary next --include-reason
# Output: "Selected TASK-123 because: highest priority (P1), no blockers, matches agent capabilities"
```

### Filtered Selection

```bash
# Filter by type
granary next --filter "type=implementation"

# Filter by label
granary next --filter "label=urgent"

# Multiple filters (AND logic)
granary next --filter "type=bugfix" --filter "priority>=high"

# Exclude certain tasks
granary next --exclude "label=deferred"
```

### Batch Selection

```bash
# Get multiple tasks for batch processing
granary next --count 5 --json

# Get all available tasks matching criteria
granary list --status ready --json
```

### Selection with Dependencies

```bash
# Only get tasks with all dependencies met
granary next --deps-satisfied

# Get tasks that can unblock others
granary next --prefer "has_dependents=true"

# Preview what would unblock
granary next --include-reason --show-unlocks
```

## Handoff Best Practices

### 1. Include Comprehensive Context

Always provide enough context for the sub-agent to work independently:

```bash
granary handoff \
  --to "ImplementationAgent" \
  --tasks "TASK-456" \
  --context "$(cat <<EOF
## Background
This task is part of the authentication refactoring project.
The goal is to migrate from JWT to session-based auth.

## Relevant Files
- src/auth/jwt.ts (to be replaced)
- src/auth/session.ts (new implementation)
- tests/auth/*.test.ts (need updates)

## Dependencies
- Redis must be running for session storage
- Use the existing User model without modifications
EOF
)"
```

### 2. Set Clear Acceptance Criteria

Define measurable completion conditions:

```bash
granary handoff \
  --to "QAAgent" \
  --tasks "TEST-789" \
  --acceptance-criteria "All unit tests pass" \
  --acceptance-criteria "Code coverage >= 80%" \
  --acceptance-criteria "No TypeScript errors" \
  --acceptance-criteria "Integration tests pass against staging"
```

### 3. Define Output Format

Specify what artifacts or outputs are expected:

```bash
granary handoff \
  --to "DocumentationAgent" \
  --tasks "DOC-101" \
  --output-format "markdown" \
  --output-path "docs/api/" \
  --acceptance-criteria "Includes examples for all public methods" \
  --acceptance-criteria "Follows existing documentation style"
```

### 4. Set Appropriate Constraints

Limit scope to prevent unintended side effects:

```bash
granary handoff \
  --to "RefactorAgent" \
  --tasks "REFACTOR-202" \
  --constraints "Only modify files in src/legacy/" \
  --constraints "Do not change public interfaces" \
  --constraints "Maintain backward compatibility" \
  --constraints "No new dependencies"
```

### 5. Establish Communication Channels

Set up progress reporting expectations:

```bash
granary handoff \
  --to "LongRunningAgent" \
  --tasks "MIGRATE-303" \
  --progress-interval 5m \
  --checkpoint-at "25%,50%,75%" \
  --escalate-after 1h
```

## Common Orchestration Patterns

### Sequential Pipeline

```bash
# Stage 1: Analysis
granary handoff --to "AnalysisAgent" --tasks "ANALYZE-1" \
  --wait  # Block until complete

# Stage 2: Implementation (depends on analysis)
granary handoff --to "ImplementationAgent" --tasks "IMPL-1" \
  --depends-on "ANALYZE-1" \
  --wait

# Stage 3: Review
granary handoff --to "ReviewAgent" --tasks "REVIEW-1" \
  --depends-on "IMPL-1"
```

### Fan-Out / Fan-In

```bash
# Fan-out: Distribute work
for component in auth api database; do
  granary handoff \
    --to "ComponentAgent" \
    --tasks "UPDATE-$component" \
    --context "Component: $component" &
done

# Fan-in: Wait for all to complete
granary wait --tasks "UPDATE-auth,UPDATE-api,UPDATE-database"

# Continue with integration
granary handoff --to "IntegrationAgent" --tasks "INTEGRATE-1"
```

### Supervisor Pattern

```bash
# Orchestrator monitors sub-agents
while granary session active; do
  # Check for stuck tasks
  STUCK=$(granary list --status in_progress --stale 30m --json)

  if [ "$STUCK" != "[]" ]; then
    for task in $(echo $STUCK | jq -r '.[].id'); do
      granary task $task reassign --reason "Timed out"
    done
  fi

  # Check for failures
  FAILED=$(granary list --status failed --json)

  if [ "$FAILED" != "[]" ]; then
    # Decide: retry, escalate, or skip
    granary task $task retry --max-attempts 3
  fi

  sleep 60
done
```

## Error Handling

### Task Failure Recovery

```bash
# In sub-agent
if ! perform_task; then
  granary task $TASK_ID fail \
    --reason "Implementation failed: $ERROR" \
    --recoverable true \
    --suggested-action "Review requirements and retry"
  exit 1
fi
```

### Orchestrator Failure Handling

```bash
# Monitor task outcomes
RESULT=$(granary task $TASK_ID wait --timeout 30m)
STATUS=$(echo $RESULT | jq -r '.status')

case $STATUS in
  "done")
    echo "Task completed successfully"
    ;;
  "failed")
    REASON=$(echo $RESULT | jq -r '.failure_reason')
    if granary task $TASK_ID can-retry; then
      granary task $TASK_ID retry
    else
      granary task $TASK_ID escalate --to "HumanReview"
    fi
    ;;
  "timeout")
    granary task $TASK_ID release --reason "Timeout"
    granary task $TASK_ID reassign
    ;;
esac
```

## Session Management

### Creating Orchestration Sessions

```bash
# Create a new orchestration session
SESSION=$(granary session create \
  --name "feature-implementation" \
  --ttl 24h \
  --max-workers 8)

export GRANARY_SESSION=$SESSION
```

### Session Cleanup

```bash
# End session and cleanup
granary session end \
  --summarize \
  --archive-logs \
  --release-all-tasks
```
