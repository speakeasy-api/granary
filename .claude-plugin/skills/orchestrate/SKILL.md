---
name: granary-orchestrate
description: Orchestrate sub-agents and coordinate multi-agent workflows with granary. Use when delegating tasks, spawning workers, or managing parallel execution.
---

# Orchestrating Implementation with Granary

This skill is for **implementation-time orchestration**. Your job is to coordinate sub-agents to implement planned work. You do NOT plan or design—you delegate and track.

## Critical: Trust the Scheduler

**NEVER** determine task parallelism by:

- Checking if `blocked_by` is null/empty on individual tasks
- Assuming tasks are independent because they don't have explicit blockers
- Launching more tasks than `granary next --all` returns
- Inspecting task metadata to make your own parallelism decisions

`granary next --all` is the **ONLY** source of truth for what can run in parallel. It considers:

- Explicit task dependencies (task_dependencies table)
- Project dependencies (project_dependencies table)
- Task status (draft tasks are never returned)
- Claims and leases
- Implicit ordering requirements

**If `granary next --all` returns 2 tasks, spawn exactly 2 agents. Not 6. Not 4. Exactly 2.**

## Output Format

**Always use `--format=prompt` instead of `--json`** for granary commands. The `--format=prompt` output is specifically designed for LLM consumption—it's token-efficient and easy to parse without wasting context on JSON syntax.

Only use `--json` when you need to extract specific fields via `jq`, and always filter the output:

```bash
# Only if you need specific fields
granary task <id> --json | jq '.id, .status'
```

## Primary Use-Case

You are prompted to implement a project. Follow these steps:

## Step 1: Get Granary Overview

```bash
granary summary
```

This shows all projects, their status, and task counts. Understand what exists before proceeding.

## Step 2: Find Your Target Project

If given a project name, find it:

```bash
granary projects
granary project <project-id>
```

### Finding Projects and Tasks

Search for projects and tasks by title:

```bash
granary search "query"                # Find matching projects and tasks
granary search "api" --format=prompt  # LLM-friendly output
```

This is useful when you know part of the project or task name but not the exact ID.

## Step 3: Assess Project Readiness

Examine the project structure:

```bash
granary project <project-id> tasks
```

**Decision tree:**

| Situation                                    | Action                                       |
| -------------------------------------------- | -------------------------------------------- |
| Project has no tasks                         | Use `/granary:plan-work` skill to plan first |
| Project has tasks but they lack detail       | Use `/granary:plan-work` skill to refine     |
| Single simple project with clear description | Implement directly as one task               |
| Project has tasks with dependencies          | **Happy path** - proceed to Step 4           |

## Step 4: Start Orchestration Session

```bash
granary session start "implementing-<project-name>" --mode execute
granary session add <project-id>
```

## Step 5: The Orchestration Loop

Your sole focus: **hand off tasks to sub-agents**. You do NOT implement tasks yourself.

**Key principle: Maximize parallelism.** Always look for opportunities to run multiple tasks concurrently. Tasks without dependencies on each other can and should run in parallel.

### Get ONLY the Tasks Granary Says Are Ready

```bash
# Get ONLY tasks the scheduler has determined are ready
granary next --all --format=prompt
```

This returns **only** the tasks that are ready to execute. The scheduler has already evaluated dependencies, project ordering, claims, and status. **Spawn agents for exactly the tasks returned—no more, no fewer.**

If no tasks are returned, either all tasks are complete or remaining tasks are blocked.

**DO NOT** bypass this by:

- Listing all project tasks and filtering by `blocked_by == null`
- Assuming tasks without explicit blockers can run in parallel
- Making your own judgment calls about task independence

The scheduler knows things you don't. Trust it.

### Spawning Agents in Parallel

When multiple tasks are unblocked, spawn them all at once using parallel Task tool calls:

```
# If granary next --all returns task-1, task-2, task-3:

Use Task tool (call all three in a SINGLE message):
  1. prompt: "Execute granary task task-1. Use /granary:execute-task skill."
     subagent_type: "general-purpose"
     run_in_background: true

  2. prompt: "Execute granary task task-2. Use /granary:execute-task skill."
     subagent_type: "general-purpose"
     run_in_background: true

  3. prompt: "Execute granary task task-3. Use /granary:execute-task skill."
     subagent_type: "general-purpose"
     run_in_background: true
```

**Important:** Use `run_in_background: true` for parallel execution. This allows multiple agents to work simultaneously.

### The Loop

1. **Ask the scheduler what's ready** → `granary next --all --format=prompt`
2. **Spawn agents for exactly those tasks** → One Task tool call per task, all in the same message
3. **Monitor progress** → Check on background agents, wait for completions
4. **Repeat** → When agents complete, ask the scheduler again for newly unblocked tasks

**Critical:** Never spawn more agents than tasks returned by `granary next --all`. If it returns 2 tasks, spawn 2 agents. Period.

### Single Task Fallback

If only one task is available or tasks must be sequential:

```bash
granary next --format=prompt
```

Then spawn a single agent and wait for completion before checking for the next task.

### What Sub-Agents Do

Each sub-agent is responsible for:

- Building context (`granary handoff --tasks <id>`)
- Starting the task (`granary task <id> start`)
- Doing the actual implementation
- Marking the task done (`granary task <id> done`) or blocked

**Note:** You do NOT need to run `granary handoff` yourself. Sub-agents handle their own context building. Your job is to identify unblocked tasks and spawn agents.

## Step 6: Handle Completion

When all tasks are done:

```bash
granary session close --summary "Completed implementation of <project-name>"
```

## Handling Edge Cases

### Sub-Agent Gets Blocked

If a sub-agent cannot complete:

```bash
# Sub-agent should have blocked the task:
granary task <task-id> block --reason "..."

# You see it when checking next task
granary next --format=prompt  # Will skip blocked tasks
```

### Conflict Prevention

Sub-agents should claim tasks with leases:

```bash
granary task <task-id> claim --owner "Worker-1" --lease 30
# Exit code 4 = conflict (task claimed by another)
```

## Checkpointing

Before risky operations:

```bash
granary checkpoint create "before-major-change"

# If things go wrong
granary checkpoint restore before-major-change
```

## Common Mistakes

### Inspecting tasks to determine parallelism

```bash
# WRONG - Don't do this
granary project <id> tasks --json | jq '.[] | select(.blocked_by == null)'
```

This bypasses the scheduler and leads to race conditions. Just because a task has no explicit `blocked_by` value doesn't mean it's ready to run. The scheduler considers implicit dependencies, project ordering, and other factors you can't see in task metadata.

### Assuming task independence

```bash
# WRONG - Don't assume tasks can run together
granary show task-1  # blocked_by: null
granary show task-2  # blocked_by: null
granary show task-3  # blocked_by: null
# "All three have no blockers, I'll run them all!"
```

Tasks may have implicit logical dependencies. Task-3 might implement a service that uses schemas from task-1. Running them in parallel causes compiler errors.

### Overriding scheduler decisions

```bash
# WRONG - Don't second-guess the scheduler
granary next --all  # Returns 2 tasks
# "But I checked and 6 tasks have no blockers, I'll launch 6 agents"
```

If `granary next --all` returns 2 tasks, there's a reason. Trust it.

### The right approach

```bash
# RIGHT - Trust the scheduler completely
granary next --all --format=prompt
# Returns task-1, task-2
# Spawn exactly 2 agents, one for each task
```

Only spawn agents for tasks returned by `granary next --all`. When they complete, run the command again to discover what's newly unblocked.

## Summary

1. **Get overview** → `granary summary`
2. **Find project** → `granary projects`
3. **Assess readiness** → Are tasks planned? If not, use plan-work skill
4. **Start session** → `granary session start`
5. **Loop: delegate tasks** → Use `granary next --all`, spawn agents for **exactly** the tasks returned
6. **Close session** → `granary session close`

**Your job is coordination, not implementation.** Sub-agents use `granary handoff` to build their own context and do the actual work—you just spawn agents for the tasks that `granary next --all` returns.

**Remember:** `granary next --all` is the scheduler. Trust it completely. If it returns 2 tasks, spawn 2 agents. Don't inspect task metadata to make your own parallelism decisions—that path leads to race conditions and compiler errors.
