---
name: granary-multi-agent-safety
description: Ensure safe concurrent execution with multiple agents using task claiming, leases, and heartbeats. Use when coordinating parallel workers or preventing duplicate work.
allowed-tools: Read, Bash(granary:*)
---

# Multi-Agent Safety

This guide covers safe concurrent execution patterns for coordinating multiple agents working on the same granary project.

## Task Claiming with Leases

Before starting work on a task, claim it with a lease to prevent other agents from working on it simultaneously:

```bash
granary task <id> claim --owner "Worker-1" --lease 30m
```

**Key behaviors:**

- Returns exit code **0** on successful claim
- Returns exit code **4** (conflict) if another agent already holds the claim
- The lease automatically expires after the specified duration, enabling recovery from crashed workers

**Example with conflict handling:**

```bash
if ! granary task TASK-123 claim --owner "Agent-$(hostname)-$$" --lease 30m; then
  echo "Task already claimed by another worker, skipping"
  exit 0
fi
```

## Heartbeat for Long-Running Tasks

For tasks that take longer than the lease duration, send periodic heartbeats to extend the lease:

```bash
granary task <id> heartbeat --lease 30m
```

**Heartbeat loop pattern:**

```bash
# Start heartbeat in background
(
  while true; do
    sleep 600  # Every 10 minutes for a 30-minute lease
    granary task TASK-123 heartbeat --lease 30m || break
  done
) &
HEARTBEAT_PID=$!

# Do the actual work
perform_long_running_task

# Stop heartbeat
kill $HEARTBEAT_PID 2>/dev/null
```

## Explicit Release

When work is complete or if you need to abandon a task, explicitly release the claim:

```bash
granary task <id> release
```

This immediately allows other agents to claim the task.

## Exit Codes

All granary commands use consistent exit codes:

| Code | Meaning    | Description                        |
| ---- | ---------- | ---------------------------------- |
| 0    | Success    | Operation completed successfully   |
| 2    | User Error | Invalid arguments or usage         |
| 3    | Not Found  | Task or resource does not exist    |
| 4    | Conflict   | Claim conflict or version mismatch |
| 5    | Blocked    | Task is blocked by dependencies    |

## Optimistic Concurrency

Granary uses version numbers for optimistic concurrency control. When updating a task, the version is checked to prevent lost updates:

```bash
# Get current version
VERSION=$(granary task TASK-123 show --format json | jq -r '.version')

# Update with version check
if ! granary task TASK-123 update --status done --version "$VERSION"; then
  echo "Conflict detected, retrying..."
  # Re-fetch and retry
fi
```

**Retry pattern:**

```bash
retry_with_backoff() {
  local max_attempts=5
  local attempt=1
  local backoff=1

  while [ $attempt -le $max_attempts ]; do
    if "$@"; then
      return 0
    fi

    if [ $? -eq 4 ]; then  # Conflict
      echo "Attempt $attempt failed with conflict, retrying in ${backoff}s..."
      sleep $backoff
      backoff=$((backoff * 2))
      attempt=$((attempt + 1))
    else
      return 1  # Non-conflict error, don't retry
    fi
  done

  return 1
}
```

## Recommended Patterns

### Claim-Before-Work Pattern

Always claim a task before starting any work:

```bash
#!/bin/bash
set -e

TASK_ID="$1"
OWNER="Agent-$(hostname)-$$"

# Attempt to claim
if ! granary task "$TASK_ID" claim --owner "$OWNER" --lease 30m; then
  echo "Could not claim task $TASK_ID"
  exit 0
fi

# Do work
echo "Working on $TASK_ID..."
perform_work

# Mark complete and release
granary task "$TASK_ID" update --status done
granary task "$TASK_ID" release
```

### Graceful Failure Handling with Trap

Use shell traps to ensure claims are released even on failure:

```bash
#!/bin/bash
set -e

TASK_ID="$1"
OWNER="Agent-$(hostname)-$$"
CLAIMED=false

cleanup() {
  if [ "$CLAIMED" = true ]; then
    echo "Releasing claim on $TASK_ID..."
    granary task "$TASK_ID" release || true
  fi
}

trap cleanup EXIT

# Claim the task
if granary task "$TASK_ID" claim --owner "$OWNER" --lease 30m; then
  CLAIMED=true
else
  echo "Task already claimed"
  exit 0
fi

# Work proceeds here - cleanup runs automatically on exit
perform_work
granary task "$TASK_ID" update --status done
```

### Long-Running Task with Heartbeat and Trap

Complete pattern for long-running tasks:

```bash
#!/bin/bash
set -e

TASK_ID="$1"
OWNER="Agent-$(hostname)-$$"
LEASE="30m"
HEARTBEAT_INTERVAL=600  # 10 minutes
HEARTBEAT_PID=""
CLAIMED=false

cleanup() {
  # Stop heartbeat
  if [ -n "$HEARTBEAT_PID" ]; then
    kill "$HEARTBEAT_PID" 2>/dev/null || true
  fi

  # Release claim
  if [ "$CLAIMED" = true ]; then
    granary task "$TASK_ID" release || true
  fi
}

trap cleanup EXIT

# Claim
if ! granary task "$TASK_ID" claim --owner "$OWNER" --lease "$LEASE"; then
  echo "Could not claim $TASK_ID"
  exit 0
fi
CLAIMED=true

# Start heartbeat
(
  while true; do
    sleep $HEARTBEAT_INTERVAL
    granary task "$TASK_ID" heartbeat --lease "$LEASE" || exit 1
  done
) &
HEARTBEAT_PID=$!

# Perform long-running work
perform_long_running_task

# Complete
granary task "$TASK_ID" update --status done
echo "Task $TASK_ID completed successfully"
```

## Best Practices

1. **Use unique owner identifiers** - Include hostname and PID to uniquely identify the claiming agent
2. **Set appropriate lease durations** - Long enough to complete work, short enough for timely recovery
3. **Heartbeat at 1/3 of lease duration** - Provides buffer for network issues
4. **Always use traps** - Ensure claims are released on unexpected exits
5. **Handle conflicts gracefully** - Exit code 4 means another agent is handling the task
6. **Retry with exponential backoff** - For transient conflicts during updates
