---
name: granary-session-management
description: Manage granary sessions to control what's in scope for an agent loop. Use when starting work sessions, switching context, or managing what projects/tasks are active.
---

# Session Management

## What is a Session?

A **session** is a context container that defines the scope of work for an agent loop. Sessions track:

- **What projects and tasks are in scope** - Only items added to the session are considered active
- **Current mode** - Whether you're planning, executing, or reviewing work
- **Session metadata** - Owner, creation time, and summary when closed

Sessions provide boundaries for agent work, preventing scope creep and maintaining focus on specific objectives.

## Session Lifecycle

### Starting a Session

Create a new session with a descriptive name:

```bash
granary session start "feature-implementation" --owner "Agent" --mode plan
```

Options:

- `--owner` - Who owns this session (e.g., "Agent", "User", or a specific name)
- `--mode` - Initial mode: `plan`, `execute`, or `review`

### Viewing Current Session

Check which session is active:

```bash
granary session current
```

This shows the session ID, name, mode, owner, and what's currently in scope.

### Switching Sessions

Switch to a different existing session:

```bash
granary session use <session-id>
```

### Closing a Session

When work is complete, close the session with a summary:

```bash
granary session close --summary "Implemented user authentication feature with OAuth2 support"
```

The summary should describe what was accomplished during the session.

## Managing Scope

Sessions define what projects and tasks are actively being worked on.

### Adding Items to Scope

Add a project to the current session:

```bash
granary session add project <project-id>
```

Add a task to the current session:

```bash
granary session add task <task-id>
```

### Removing Items from Scope

Remove a project from scope:

```bash
granary session rm project <project-id>
```

Remove a task from scope:

```bash
granary session rm task <task-id>
```

## Session Modes

Sessions operate in one of three modes that signal the current phase of work:

### Plan Mode

```bash
granary session start "planning" --mode plan
```

Use when:

- Analyzing requirements
- Breaking down work into tasks
- Designing solutions
- Creating or updating project plans

### Execute Mode

```bash
granary session start "implementation" --mode execute
```

Use when:

- Writing code
- Making changes to files
- Running commands
- Implementing planned tasks

### Review Mode

```bash
granary session start "code-review" --mode review
```

Use when:

- Reviewing completed work
- Running tests and validation
- Checking quality
- Preparing summaries

Change mode during a session:

```bash
granary session mode execute
```

## Environment Variables

Export session context to environment variables:

```bash
eval $(granary session env)
```

This sets `GRANARY_SESSION` and other relevant variables, allowing tools and scripts to be aware of the current session context.

## When to Create New vs Reuse Sessions

### Create a New Session When:

- Starting work on a distinct feature or objective
- Switching to an unrelated task
- Beginning a fresh planning cycle
- The previous session's scope no longer applies

### Reuse an Existing Session When:

- Continuing work from a previous interaction
- The scope (projects/tasks) remains relevant
- You're in the middle of a multi-step process
- The mode and context still apply

### Best Practices

1. **Check for existing sessions first** - Use `granary session current` before creating new ones
2. **Use descriptive names** - Session names should indicate the work being done
3. **Keep scope focused** - Only add projects/tasks that are actively being worked on
4. **Close sessions cleanly** - Always provide a summary when closing to maintain history
5. **Match mode to activity** - Switch modes as your work phase changes
