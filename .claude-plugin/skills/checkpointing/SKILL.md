---
name: granary-checkpointing
description: Use checkpoints for pause/resume and rollback in granary. Use before risky operations, when pausing work, or to recover from mistakes.
---

# Checkpointing

Checkpoints capture the full state of a granary session, enabling pause/resume workflows, rollback from mistakes, and time-travel debugging.

## Checkpoint Operations

### Create a Checkpoint

```bash
granary checkpoint create "name"
```

Creates a named snapshot of the current session state. Use descriptive names that indicate the point in your workflow (e.g., "before-auth-refactor", "tests-passing", "end-of-day-jan-11").

### List Checkpoints

```bash
granary checkpoint list
```

Shows all checkpoints for the current session with timestamps and names.

### Compare Checkpoints

```bash
granary checkpoint diff before-refactor now --format md
```

Compare two checkpoints to see what changed. Use `now` to compare against the current state. The `--format md` option produces markdown output suitable for review.

### Restore a Checkpoint

```bash
granary checkpoint restore name
```

Reverts the session to the specified checkpoint state. This restores all captured metadata and task states.

## What Checkpoints Capture

- **Session metadata** - Session ID, creation time, description
- **Scope** - Current working scope (files, directories, patterns)
- **Focus task** - The currently active task being worked on
- **Session variables** - All key-value pairs stored in the session
- **Task states** - Status and progress of all tracked tasks

## Use Cases

### Before Risky Refactors

Create a checkpoint before attempting significant code changes:

```bash
granary checkpoint create "before-auth-refactor"
# ... attempt the refactor ...
# If it goes wrong:
granary checkpoint restore "before-auth-refactor"
```

### Pause and Resume (End of Day)

Capture your exact working state when stepping away:

```bash
granary checkpoint create "end-of-day-jan-11"
# Tomorrow, resume exactly where you left off
granary checkpoint restore "end-of-day-jan-11"
```

### Debugging with Time Travel

When tracking down when something broke, use checkpoints to navigate through time:

```bash
granary checkpoint list
granary checkpoint diff "tests-passing" "now" --format md
```

This reveals exactly what changed since the tests were last green.

### Experiment Branches (Compare Approaches)

Try multiple approaches and compare results:

```bash
granary checkpoint create "baseline"
# Try approach A
granary checkpoint create "approach-a"
granary checkpoint restore "baseline"
# Try approach B
granary checkpoint create "approach-b"
# Compare the two approaches
granary checkpoint diff "approach-a" "approach-b" --format md
```

## Best Practices

1. **Create before destructive operations** - Always checkpoint before operations that are difficult to undo manually
2. **Use descriptive names** - Names should indicate the workflow stage or purpose (e.g., "pre-migration", "api-stable", "friday-eod")
3. **Checkpoint at milestones** - Create checkpoints when reaching stable states like "tests-passing" or "feature-complete"
4. **Clean up old checkpoints** - Periodically review and remove checkpoints that are no longer needed
