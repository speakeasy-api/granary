---
description: Show current granary session status, tasks, and next actions
user-invocable: true
allowed-tools: Bash(granary:*)
---

# Granary Status Command

When the user runs /granary:status, provide a summary of the current granary state.

## Steps

1. **Current Session:**
   Run: `granary session current --json`
   Display: Session name, mode, owner

2. **Task Summary:**
   Run: `granary tasks --json`
   Display: Count by status (todo, in_progress, done, blocked)

3. **Next Action:**
   Run: `granary next --json`
   Display: The next recommended task

4. **Blockers:**
   Run: `granary tasks --status blocked --json`
   Display: Any blocked tasks and their reasons

## Output Format

Format as clean markdown:

## Granary Status

**Session:** [name] ([mode] mode)
**Owner:** [owner]

### Tasks

- Todo: [count]
- In Progress: [count]
- Done: [count]
- Blocked: [count]

### Next Action

[Task title and ID]

### Blockers

[List blocked tasks with reasons, or "None"]
