---
name: granary-orchestrate
description: Orchestrate sub-agents and coordinate multi-agent workflows with granary. Use when delegating tasks, spawning workers, or managing parallel execution.
---

# Orchestrating Sub-Agents with Granary

Use this skill when you need to delegate tasks to sub-agents or coordinate parallel work.

## 1. Start an Orchestration Session

```bash
granary session start "feature-impl" --owner "Orchestrator" --mode execute
granary session add <project-id>
```

## 2. The Orchestrator Loop

The core pattern for processing tasks:

```bash
# Get next actionable task (respects dependencies)
granary next --json

# Returns the highest priority task with all dependencies satisfied
# Returns null when no more tasks
```

### Basic Loop

```bash
while TASK=$(granary next --json) && [ "$(echo $TASK | jq -r '.task')" != "null" ]; do
  TASK_ID=$(echo $TASK | jq -r '.task.id')

  # Start the task
  granary task $TASK_ID start --owner "Orchestrator"

  # Get context for sub-agent
  CONTEXT=$(granary context --format prompt)

  # Spawn sub-agent with task context
  # ... your agent spawning logic here ...

  # When sub-agent completes, mark done
  granary task $TASK_ID done
done
```

## 3. Preparing Context for Sub-Agents

### Quick Summary

```bash
granary summary
```

### Detailed Context Pack

```bash
# Full context for LLM consumption
granary context --format prompt

# With specific includes
granary context --include tasks,decisions,blockers --format prompt
```

### Task-Specific Context

```bash
# Just the task details
granary show <task-id> --format prompt
```

## 4. Handoff Documents

Generate structured handoffs for sub-agents:

```bash
granary handoff --to "Implementation Agent" \
  --tasks task-1,task-2 \
  --constraints "Do not modify production code" \
  --acceptance-criteria "All tests pass"
```

## 5. Parallel Execution

For parallel workers, pass the session ID:

```bash
# Export session for sub-agents
eval $(granary session env)
# Sets GRANARY_SESSION environment variable

# Each sub-agent can then use the same session
GRANARY_SESSION=sess-xxx granary task <id> start --owner "Worker-1" --lease 30
```

### Preventing Conflicts

Sub-agents should claim tasks with leases:

```bash
# Sub-agent claims task (fails if already claimed)
granary task <task-id> claim --owner "Worker-1" --lease 30

# Exit code 4 means conflict - task claimed by another
```

## 6. Checkpointing

Before risky operations, create a checkpoint:

```bash
granary checkpoint create "before-major-refactor"

# If things go wrong
granary checkpoint restore before-major-refactor
```

## 7. Steering for Sub-Agents

Steering files provide standards, conventions, and context that sub-agents should follow. Steering can be scoped to prevent context pollution:

| Scope | When Included | Use Case |
|-------|---------------|----------|
| Global (default) | Always in context/handoffs | Project-wide standards |
| `--project <id>` | When project is in session scope | Project-specific patterns |
| `--task <id>` | When handing off that specific task | Task-specific research |
| `--for-session` | During session, auto-deleted on close | Temporary research notes |

### Adding Steering Files

```bash
# Global steering (always included)
granary steering add docs/coding-standards.md

# Project-attached (only when this project is in context)
granary steering add docs/auth-patterns.md --project auth-proj-abc1

# Task-attached (only in handoffs for this specific task)
granary steering add .granary/task-research.md --task auth-proj-abc1-task-3

# Session-attached (temporary, auto-deleted when session closes)
granary steering add .granary/temp-notes.md --for-session

# List current steering files
granary steering list
# Output:
# docs/coding-standards.md [global]
# docs/auth-patterns.md [project: auth-proj-abc1]
# .granary/task-research.md [task: auth-proj-abc1-task-3]
# .granary/temp-notes.md [session: sess-xxx]

# Remove steering (specify scope to match)
granary steering rm docs/auth-patterns.md --project auth-proj-abc1
```

### Use Case: Research Before Delegating

Before spawning sub-agents, orchestrators can research the codebase and add findings as session-scoped steering (auto-cleaned on session close):

```bash
# 1. Do research (as orchestrator)
#    - Explore codebase structure
#    - Identify patterns and conventions
#    - Note relevant files and dependencies

# 2. Write findings to a file
cat > .granary/research-notes.md << 'EOF'
# Research Notes: Authentication Implementation

## Existing Patterns
- Auth middleware in src/middleware/auth.rs uses JWT tokens
- User model in src/models/user.rs with bcrypt password hashing
- Session storage uses Redis (see src/services/session.rs)

## Key Conventions
- All API endpoints return JSON with {data, error, meta} structure
- Use `ApiError` type for error handling
- Tests go in tests/ directory, not inline
EOF

# 3. Add as session-scoped steering (auto-deleted on session close)
granary steering add .granary/research-notes.md --for-session

# 4. Now when you spawn sub-agents, they receive this context
granary context --format prompt  # Includes the research notes
# When session closes, research-notes.md steering is automatically removed!
```

### Task-Specific Steering for Handoffs

When you need steering only for a specific task handoff:

```bash
# Research specific to one task
cat > .granary/task-3-notes.md << 'EOF'
# Task-Specific Notes
- This endpoint needs rate limiting
- See existing rate limiter in src/middleware/rate_limit.rs
EOF

# Attach to the specific task
granary steering add .granary/task-3-notes.md --task auth-proj-abc1-task-3

# Only included when handing off that task
granary handoff --to "Agent" --tasks auth-proj-abc1-task-3  # Includes notes
granary handoff --to "Agent" --tasks auth-proj-abc1-task-4  # Does NOT include
```

### When to Use Each Scope

- **Global**: Project-wide coding standards, architecture decisions
- **Project-attached**: Module-specific patterns (e.g., auth module conventions)
- **Task-attached**: Research specific to one task (avoid polluting other handoffs)
- **Session-attached**: Temporary research that shouldn't persist after the work is done

## 8. Close Session When Done

```bash
granary session close --summary "Completed feature implementation"
```

## Example: Orchestrating a Feature Build

```bash
# Setup
granary session start "auth-feature" --owner "Orchestrator" --mode execute
granary session add auth-proj-abc1

# Research phase: explore codebase and document findings
# ... do research ...

# Write research as session-scoped steering (auto-cleaned on close)
cat > .granary/auth-research.md << 'EOF'
# Auth Implementation Notes
- Use existing JWT middleware pattern
- Follow error handling in src/error.rs
EOF
granary steering add .granary/auth-research.md --for-session

# Process tasks
while TASK=$(granary next --json) && [ "$(echo $TASK | jq -r '.task')" != "null" ]; do
  TASK_ID=$(echo $TASK | jq -r '.task.id')
  TITLE=$(echo $TASK | jq -r '.task.title')

  echo "Processing: $TITLE"
  granary task $TASK_ID start --owner "Orchestrator"

  # Generate handoff (includes global + session steering automatically)
  granary handoff --to "Worker" --tasks $TASK_ID

  # Spawn sub-agent with handoff context
  # ... your agent spawning logic here ...

  # Wait for completion, then:
  granary task $TASK_ID done
done

# Cleanup - session steering is automatically removed on close!
granary session close --summary "Auth feature complete"
```
