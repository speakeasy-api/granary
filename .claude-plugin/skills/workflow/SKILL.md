---
name: granary-workflow
description: Complete granary workflow guide for planning, executing, and completing work with agents. Use when unsure about the overall workflow or best practices.
---

# Granary Workflow Best Practices

This guide covers the complete granary workflow for planning, executing, and completing work with agents.

## Phase 1: Planning

Planning establishes clear scope and dependencies before any work begins.

### 1.1 Initialize and Start Planning

```bash
# Initialize granary in your project (if not already done)
granary init

# Start a planning session
granary session start --type planning
```

### 1.2 Create Project Structure

```bash
# Create a new project for the work
granary project create --name "feature-name" --description "Clear description of the goal"

# Add scope to define boundaries
granary scope add --project "feature-name" --include "src/components/**" --exclude "src/legacy/**"
```

### 1.3 Break Down Tasks

Create small, focused tasks with self-contained descriptions:

```bash
# Create tasks with clear, actionable descriptions
granary task create --project "feature-name" \
  --title "Implement user authentication" \
  --description "Create auth module in src/auth/ with login, logout, and session management. Must integrate with existing UserService."

# Each task should be completable in a single session
granary task create --project "feature-name" \
  --title "Add unit tests for auth module" \
  --description "Write Jest tests for src/auth/. Cover login success/failure, session expiry, and logout flows. Target 80% coverage."
```

### 1.4 Add Dependencies

Define task relationships to enforce ordering:

```bash
# Task B depends on Task A
granary dep add --from "task-b-id" --to "task-a-id"

# View dependency graph
granary dep show --project "feature-name"
```

### 1.5 Checkpoint Before Closing

Save state before ending the planning phase:

```bash
# Create a checkpoint
granary checkpoint create --message "Planning complete for feature-name"

# Close the planning session
granary session close
```

## Phase 2: Execution

Execution follows the orchestrator pattern: claim, work, complete.

### 2.1 Start Execute Session

```bash
granary session start --type execute
```

### 2.2 Orchestrator Loop

The orchestrator continuously processes ready tasks:

```bash
# Get the next available task (respects dependencies)
granary task next --project "feature-name"

# Start work on a task (claims a lease)
granary task start --id "task-id"

# Spawn an agent to do the work
# The agent receives the task description and works autonomously

# When agent completes, mark task done
granary task done --id "task-id" --summary "Implemented auth module with JWT tokens"
```

### 2.3 Progress Visibility

Keep progress visible through comments:

```bash
# Add progress updates
granary comment add --task "task-id" --message "Completed login flow, starting logout"

# Record blockers
granary comment add --task "task-id" --message "BLOCKED: Waiting for API spec clarification"
```

### 2.4 Close Execute Session

```bash
granary session close
```

## Phase 3: Review

Review validates completed work and records decisions.

### 3.1 Start Review Session

```bash
granary session start --type review
```

### 3.2 Handoff and Review

```bash
# List completed tasks for review
granary task list --project "feature-name" --status done

# Review each task's output
granary task show --id "task-id"
```

### 3.3 Record Decisions

Document important decisions made during the work:

```bash
# Record architectural decisions
granary decision add --project "feature-name" \
  --title "Use JWT for authentication" \
  --rationale "Stateless, scalable, industry standard"

# Record any deferred work
granary task create --project "feature-name" \
  --title "Future: Add OAuth support" \
  --description "Deferred from current scope. Add Google/GitHub OAuth as alternative login methods."
```

### 3.4 Close Review Session

```bash
granary session close
```

## Key Principles

### 1. Tasks Are Context Transfer

Task descriptions must be self-contained. An agent should be able to complete the task with only the description - no additional context needed.

**Good:**

```
Create auth module in src/auth/ with:
- login(email, password) -> returns JWT
- logout(token) -> invalidates session
- validateSession(token) -> returns user or throws
Must use existing UserService from src/services/user.ts
```

**Bad:**

```
Implement the auth stuff we discussed
```

### 2. Sessions Scope Work

Sessions define the boundaries of a work unit. Always work within a session:

- Planning sessions for design and task creation
- Execute sessions for implementation
- Review sessions for validation

### 3. Claim Before Work (Leases)

Always claim a task before starting work. This prevents conflicts when multiple agents operate concurrently:

```bash
# This claims a lease on the task
granary task start --id "task-id"

# If you abandon work, release the lease
granary task release --id "task-id"
```

### 4. Checkpoint Before Risk

Create checkpoints before risky operations:

```bash
# Before large refactors
granary checkpoint create --message "Before auth refactor"

# Before external integrations
granary checkpoint create --message "Before payment gateway integration"
```

### 5. Progress Is Visible

Use comments to maintain visibility into ongoing work:

```bash
# Regular progress updates
granary comment add --task "task-id" --message "50% complete - API done, starting UI"

# Document unexpected findings
granary comment add --task "task-id" --message "Found existing auth code in legacy/ - evaluating reuse"
```

### 6. Close Sessions Cleanly

Always close sessions when work is complete. Open sessions indicate incomplete work:

```bash
# Check for open sessions
granary session list --status open

# Close when done
granary session close
```

## Anti-Patterns to Avoid

### Vague Descriptions

**Anti-pattern:**

```bash
granary task create --title "Fix the bug" --description "It's broken"
```

**Better:**

```bash
granary task create --title "Fix null pointer in UserService.getById" \
  --description "UserService.getById throws NPE when user not found. Should return null or throw UserNotFoundException. See error in logs/app.log line 142."
```

### Monolithic Tasks

**Anti-pattern:**

```bash
granary task create --title "Build the entire feature" \
  --description "Implement everything for the new dashboard"
```

**Better:**
Break into focused tasks:

- "Create dashboard data API endpoint"
- "Build dashboard React component"
- "Add dashboard route and navigation"
- "Write dashboard integration tests"

### Forgetting to Close Sessions

**Anti-pattern:**
Starting new sessions without closing previous ones, leaving orphaned work.

**Better:**

```bash
# Always check and close
granary session list --status open
granary session close
```

### Not Using Dependencies

**Anti-pattern:**
Creating tasks without dependencies, leading to agents attempting work before prerequisites are complete.

**Better:**

```bash
# Define clear dependency chains
granary task create --title "Design API schema" --id schema
granary task create --title "Implement API" --id impl
granary task create --title "Write API tests" --id tests

granary dep add --from impl --to schema
granary dep add --from tests --to impl
```

## Quick Reference

| Phase    | Key Commands                                                                                     |
| -------- | ------------------------------------------------------------------------------------------------ |
| Planning | `session start --type planning`, `project create`, `task create`, `dep add`, `checkpoint create` |
| Execute  | `session start --type execute`, `task next`, `task start`, `task done`, `comment add`            |
| Review   | `session start --type review`, `task list --status done`, `decision add`, `session close`        |
