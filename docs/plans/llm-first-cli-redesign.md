# LLM-First CLI Redesign Proposal

## Executive Summary

Redesign granary's CLI so that **command outputs guide agents through workflows**. Skills become obsolete - an agent told to "use granary" can complete any workflow by following command output alone.

---

## Problem Statement

### Current State

- Skills teach command sequences externally
- CRUD-focused outputs return data, not guidance
- Agents need pre-loaded knowledge to use granary effectively

### Target State

- Command outputs are the complete tutorial
- Contextual guidance based on what agent was commanded to do
- Skills are obsolete - CLI output is sufficient

---

## Design Principles

### 1. Directive-Based Context

Output adapts to what the agent was commanded to do:

| User Prompt                                            | Workflow        |
| ------------------------------------------------------ | --------------- |
| "use granary to implement proj-abc123-task-4"          | Work            |
| "use granary to plan adding x feature"                 | Plan Project    |
| "I want to build x, use granary"                       | Plan Project    |
| "use granary to build complicated multi-service thing" | Plan Initiative |

### 2. Token Efficiency

High signal, no noise. Only output information the agent needs to act.

### 3. Single Commands

One action = one command. No multi-step sequences for atomic operations.

### 4. Fail Fast

If something can't be done, tell agent to bail. Don't suggest alternatives.

---

## Core Workflows

### Workflow 1: Work

**Trigger**: "use granary to implement `<task-id>`"

Agent runs:

```sh
granary work <task-id> --owner "Agent Name, e.g.: Opus 4.5 Worker 83"
```

This single command claims the task, starts work, and outputs everything needed.

**Output** (success):

````
## add-instagram-oauth2-f7g8-task-2: Implement token exchange

Project: add-instagram-oauth2-f7g8
Priority: P1

**Goal:** Implement OAuth2 token exchange with Instagram API

**Files to modify:**
- src/auth/providers/instagram.rs:45-80 (add exchange function)
- src/auth/token.rs:23-40 (add Instagram token type)

**Pattern (from Meta provider):**
```rust
pub async fn exchange_code(code: &str) -> Result<Token> {
    // Follow pattern in src/auth/providers/meta.rs:52-78
}
````

**Acceptance criteria:**

- [ ] Exchange endpoint called correctly
- [ ] Token stored in session
- [ ] Error handling matches other providers
- [ ] Tests pass

## Steering

Contents of docs/auth-patterns.md:
[... file contents inline ...]

## When Done

granary work done <task-id> "summary of changes"

## If Blocked

granary work block <task-id> "reason"

## If Cannot Complete

granary work release <task-id>

```

**Output** (task has unmet dependencies):
```

Task blocked by dependencies. Exiting.

```

**Output** (task not found):
```

Task not found. Exiting.

```

**Output** (task already claimed):
```

Task claimed by another worker. Exiting.

```

**Output** (after `granary work done <task-id> "Implemented token exchange..."`):
```

Done.

```

**Output** (after `granary work block <task-id> "Waiting for API credentials"`):
```

Blocked.

```

**Output** (after `granary work release <task-id>`):
```

Released.

````

---

### Workflow 2: Plan Project

**Trigger**: "use granary to plan adding x feature"

Agent runs:
```sh
granary plan "Add Instagram OAuth2 provider"
````

**Output**:

```
Project created: add-instagram-oauth2-f7g8

## Prior Art

- add-meta-oauth2-abc1: Add Meta OAuth2 provider (completed)
- auth-system-d4e5: Authentication system (3/7 tasks done)

View details:
  granary project <project-id> summary

## Research

Before creating tasks, research the codebase:
- Find all files that need modification (exact paths, line numbers)
- Document existing patterns to follow
- Identify test patterns to replicate

## Create Tasks

Task descriptions are the ONLY context workers receive.

  granary project add-instagram-oauth2-f7g8 task create "Task title" --description "
  **Goal:** What this accomplishes

  **Files to modify:**
  - path/to/file.rs:10-20 (what to change)

  **Pattern:**
  \`\`\`rust
  // code example from existing similar code
  \`\`\`

  **Acceptance criteria:**
  - [ ] Criterion 1
  - [ ] Criterion 2
  "

## Set Dependencies

  granary task <task-id> deps add <other-task-id>

## Attach Steering Files

Context files included in worker handoffs:

  granary project add-instagram-oauth2-f7g8 steer add <path>

## Finish

When all tasks created:
  granary project add-instagram-oauth2-f7g8 ready
```

**Output** (after creating a task):

```
Task created: add-instagram-oauth2-f7g8-task-1
```

**Output** (after `granary project <id> ready`):

```
Project ready: add-instagram-oauth2-f7g8

Tasks: 4
Dependencies configured: 3
Steering files: 2
```

---

### Workflow 3: Plan Initiative

**Trigger**: "use granary to build complicated multi-service thing"

Agent runs:

```sh
granary initiative "User authentication system"
```

**Output**:

```
Initiative created: user-auth-system-m3n4

## When to Use

Initiatives coordinate multiple projects. Use when:
- Work spans multiple services (API + frontend + workers)
- Clear phases with cross-project dependencies

If work fits in a single project, use `granary plan "name"` instead.

## Prior Art

- auth-api-abc1: Auth API endpoints (completed)
- user-service-d4e5: User service (in_progress)

View details:
  granary project <project-id> summary

## Plan Projects

Each project in the initiative needs full task planning.

If you can spawn sub-agents, spawn one per project with prompt:
  run `granary initiative user-auth-system-m3n4 plan "Auth API endpoints"`
  run `granary initiative user-auth-system-m3n4 plan "Auth frontend components"`
  run `granary initiative user-auth-system-m3n4 plan "Background auth workers"`

Otherwise, run these commands sequentially yourself.

## Set Project Dependencies

After projects are created:
  granary initiative user-auth-system-m3n4 dep <project-id> --on <other-project-id>

View dependency graph:
  granary initiative user-auth-system-m3n4 graph

## Finish

When all projects planned:
  granary initiative user-auth-system-m3n4 ready
```

**Output** (after `granary initiative <id> plan "Auth API endpoints"`):

Same as `granary plan`, but project is added to initiative:

```
Project created: auth-api-f7g8
Initiative: user-auth-system-m3n4

## Prior Art
...

## Research
...

## Create Tasks

  granary project auth-api-f7g8 task create "Task title" --description "..."

## Set Dependencies

  granary task <task-id> deps add <other-task-id>

## Attach Steering Files

  granary project auth-api-f7g8 steer add <path>

## Finish

  granary project auth-api-f7g8 ready
```

**Output** (after `granary initiative <id> graph`):

```
auth-api-f7g8 ─┬─> auth-frontend-h9i0
               └─> auth-workers-j1k2
```

**Output** (after `granary initiative <id> ready`):

```
Initiative ready: user-auth-system-m3n4

Projects: 3
All projects have tasks: yes
```

---

## Entry Point: `granary`

When agent runs `granary` without knowing which workflow:

**Output** (uninitialized):

```
Not initialized. Run: granary init
```

**Output** (initialized):

```
Plan a feature:
  granary plan "Feature name"

Plan multi-project work:
  granary initiative "Initiative name"

Work on task:
  granary work <task-id>

Search:
  granary search "keyword"
```

---

## Command Reference

### Work

| Command                                 | Use                            |
| --------------------------------------- | ------------------------------ |
| `granary work <task-id>`                | Start working on assigned task |
| `granary work done <task-id> "summary"` | Complete task                  |
| `granary work block <task-id> "reason"` | Block task                     |
| `granary work release <task-id>`        | Release task                   |

### Plan Project

| Command                                                        | Use                                   |
| -------------------------------------------------------------- | ------------------------------------- |
| `granary plan "name"`                                          | Create new project                    |
| `granary plan --project <id>`                                  | Plan existing project (in initiative) |
| `granary project <id> task create "title" --description "..."` | Add task                              |
| `granary task <id> deps add <other-id>`                        | Set dependency                        |
| `granary project <id> steer add <path>`                        | Attach steering file                  |
| `granary project <id> summary`                                 | View project details                  |
| `granary project <id> ready`                                   | Mark project ready                    |

### Plan Initiative

| Command                                           | Use                                                                     |
| ------------------------------------------------- | ----------------------------------------------------------------------- |
| `granary initiative "name"`                       | Create initiative                                                       |
| `granary initiative <id> plan "name"`             | Plan project in initiative (same as `granary plan`, adds to initiative) |
| `granary initiative <id> dep <proj> --on <other>` | Set project dependency                                                  |
| `granary initiative <id> graph`                   | View dependency graph                                                   |
| `granary initiative <id> ready`                   | Mark initiative ready                                                   |

### Discovery

| Command                    | Use                  |
| -------------------------- | -------------------- |
| `granary`                  | Get started          |
| `granary search "keyword"` | Find projects/tasks  |
| `granary init`             | Initialize workspace |

---

## Implementation Notes

### Preserved CRUD Commands

Unchanged for humans and scripting:

- `granary projects`, `granary project <id> ...`
- `granary tasks`, `granary task <id> ...`
- `granary show <id>`
- `granary config`, `granary steering`

### Orchestration

Invisible to agents. Granary workers:

- Watch for `task.unblocked` events
- Spawn agents with "use granary to implement `<task-id>`"

### Steering

- **Global**: `granary steering add <path>`
- **Project**: `granary project <id> steer add <path>`
- **Task**: `granary task <id> steer add <path>`

`granary work <task-id>` outputs all relevant steering inline.
