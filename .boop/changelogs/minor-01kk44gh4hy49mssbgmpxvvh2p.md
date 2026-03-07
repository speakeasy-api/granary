### Review mode workflow

Granary now supports an optional review gate in the task and project lifecycle. When enabled, completed work transitions to `in_review` instead of going straight to `done`/`completed`, giving a reviewer (human or agent) the chance to approve or reject it before it's finalized.

**Two scopes:**

- **`task` mode** — `granary work done` moves the task to `in_review` and emits a `task.review` event. A reviewer approves or rejects individual tasks.
- **`project` mode** — Tasks still complete normally, but when all tasks are done, the project enters `in_review` instead of `completed`. Reviewers approve the project as a whole, or reject it by creating follow-up tasks and reopening the project.

**New `granary review` command:**

- `granary review <id>` — displays reviewer context (task/project details, comments, suggested actions)
- `granary review <id> approve ["comment"]` — approves and completes the entity
- `granary review <id> reject "feedback"` — rejects with feedback; tasks return to `todo`, projects reopen to `active` with draft tasks promoted to `todo`

Review comments use a new `review` comment kind, and review events (`task.review`, `project.review`) are emitted so downstream agents or integrations can react.

### Review mode configuration

Review mode is stored in the workspace database (`config` table) under the key `workflow.review_mode`. Enable it with:

```
granary config set workflow.review_mode task    # or 'project'
granary config unset workflow.review_mode       # disable
```

### Updated SQL triggers

The `trg_project_auto_complete` trigger is now config-aware — when `workflow.review_mode` is set to `project`, it transitions the project to `in_review` instead of `completed`. New triggers emit `task.review` and `project.review` events on status transitions.