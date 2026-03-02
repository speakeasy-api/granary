# Review Mode

## Summary

Add a configurable review gate to granary's task and project lifecycle.

- When enabled, completed work transitions to `in_review` instead of `done`/`completed`
- Review events are emitted so reviewer agents can act
- Reviewers can approve work to complete it
- Task rejections send work back to `todo` with review feedback
- Project rejections reopen work using an explicit review workflow

Key design decision in this plan revision:
- `review_mode` is stored in the workspace database (`config` table), not TOML
- `granary config` remains lean: existing behavior stays as-is, and review mode is a dedicated config action

---

## Problem Statement

### Current State

- `granary work done <task-id>` transitions tasks to `done`
- `trg_project_auto_complete` transitions projects to `completed` when all tasks are done
- No review step between `in_progress` and terminal states
- No dedicated reviewer workflow
- No review-specific comment kind

### Target State

- Workspace-scoped review mode is stored in DB config key: `workflow.review_mode`
  - Values: `task`, `project`, or unset (disabled)
- `review_mode: task`
  - `work done` / `task done` transitions task to `in_review` and emits `task.review`
- `review_mode: project`
  - Task completion still transitions task to `done`
  - When all project tasks are done, `trg_project_auto_complete` transitions project to `in_review` and emits `project.review`
- `granary review <id>` outputs reviewer context
- `granary review <id> approve "comment"` completes the entity
- `granary review <task-id> reject "comment"` sends task back to `todo` and emits `task.next`
- `granary review <project-id> reject "comment"` reopens the project for follow-up work
  - Reviewer workflow:
    1. Create follow-up tasks in `draft` state (default)
    2. Reject the project review
  - Rejection transitions project `in_review -> active`, then tasks `draft -> todo`
- Review comments use `CommentKind::Review`

---

## Design

### 1. Config: Workspace DB Source of Truth

Store review mode in workspace DB `config` table:

- Key: `workflow.review_mode`
- Value: `task`, `project`, or missing

This avoids cross-backend routing complexity in generic `config set/get` and works natively with multi-workspace behavior.

#### 1a. CLI shape (lean config UX)

Keep existing config commands unchanged for existing keys.

Add a focused config action for review mode:

```bash
granary config review-mode                # show current mode (task/project/disabled)
granary config review-mode task           # set task review mode for current workspace
granary config review-mode project        # set project review mode for current workspace
granary config review-mode off            # disable review mode for current workspace
```

Implementation notes:
- Uses `Workspace::find()` + workspace pool
- Reads/writes `config` table key `workflow.review_mode`
- `off` deletes `workflow.review_mode` (does not store literal `"off"`)
- Does not change `config edit` (TOML)
- Does not change behavior of existing `config set/get/list/delete`

### 2. Schema Changes (Types)

#### 2a. New `in_review` status values

**crates/granary-types/src/task.rs** — `TaskStatus`:

```rust
pub enum TaskStatus {
    Draft,
    Todo,
    InProgress,
    InReview,  // NEW
    Done,
    Blocked,
}
```

Update `as_str()` => `"in_review"`, `FromStr` to accept `"in_review" | "in-review" | "inreview"`.

`is_terminal()` remains `Done` only. Add `is_in_review()` helper.

**crates/granary-types/src/project.rs** — `ProjectStatus`:

```rust
pub enum ProjectStatus {
    Active,
    InReview,    // NEW
    Completed,
    Archived,
}
```

Update `as_str()` / `FromStr` similarly and add `is_in_review()`.

Implementation note:
- Adding `InReview` to shared enums requires updating exhaustive `match` sites across CLI and `crates/silo` UI codepaths.

#### 2b. New `review` comment kind

**crates/granary-types/src/comment.rs** — `CommentKind`:

```rust
pub enum CommentKind {
    Note,
    Progress,
    Decision,
    Blocker,
    Handoff,
    Incident,
    Context,
    Review,  // NEW
}
```

Update `as_str()`, `FromStr`, `all()`.

#### 2c. New event types

**crates/granary-types/src/event.rs** — `EventType`:

```rust
// Task events
TaskReview,    // task.review

// Project events
ProjectReview, // project.review
```

Update `as_str()` and `FromStr`.

### 3. Migration: Review Triggers (Keep Auto-Complete Trigger)

Create migration via SQLx CLI from repo root:

```bash
sqlx migrate add review_mode_triggers
```

#### 3a. Task review trigger

Emit `task.review` when task transitions to `in_review`:

```sql
CREATE TRIGGER trg_task_review
AFTER UPDATE ON tasks
WHEN OLD.status != 'in_review' AND NEW.status = 'in_review'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.review', 'task', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;
```

#### 3b. Project review trigger

Emit `project.review` when project transitions to `in_review`:

```sql
CREATE TRIGGER trg_project_review
AFTER UPDATE ON projects
WHEN OLD.status != 'in_review' AND NEW.status = 'in_review'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'project.review', 'project', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'slug', NEW.slug,
      'name', NEW.name, 'description', NEW.description,
      'status', NEW.status, 'owner', NEW.owner
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;
```

#### 3c. Update project auto-complete trigger (do not drop)

Keep `trg_project_auto_complete`, but make target status depend on `workflow.review_mode` in workspace DB config.

```sql
DROP TRIGGER IF EXISTS trg_project_auto_complete;

CREATE TRIGGER trg_project_auto_complete
AFTER UPDATE ON tasks
WHEN OLD.status != 'done' AND NEW.status = 'done'
  AND NOT EXISTS (
    SELECT 1 FROM tasks t
    WHERE t.project_id = NEW.project_id
      AND t.id != NEW.id
      AND t.status != 'done'
  )
  AND EXISTS (
    SELECT 1 FROM projects p
    WHERE p.id = NEW.project_id
      AND p.status = 'active'
  )
BEGIN
  UPDATE projects
  SET status = CASE
      WHEN (
        SELECT c.value FROM config c WHERE c.key = 'workflow.review_mode' LIMIT 1
      ) = 'project' THEN 'in_review'
      ELSE 'completed'
    END,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    version = version + 1
  WHERE id = NEW.project_id;
END;
```

This preserves compatibility for all paths that set task status to `done`, including direct `task update --status done`.

#### 3d. task.next on task rejection

No new trigger needed.

Existing `trg_task_next_on_status_todo` handles `in_review -> todo` when claim is cleared and dependencies are satisfied.

**Dependency blocking during review (`review_mode: task`):** When `complete_task` sets a task to `in_review` instead of `done`, `trg_task_next_on_dep_completed` does NOT fire (it requires `NEW.status = 'done'`). Downstream dependent tasks remain blocked until the task is approved and transitions to `done`. This is correct — dependents should not unblock until review passes.

#### 3e. Project auto-reactivation trigger remains completed-only

Keep `trg_project_auto_reactivate` behavior for `completed -> active` on task insert unchanged.

Do **not** auto-reactivate from `in_review` on task insert. Reopen from `in_review` is handled explicitly by `granary review <project-id> reject ...`, which preserves the required ordering:
1. `project: in_review -> active`
2. `tasks: draft -> todo`

#### 3f. Update `trg_project_unarchived` to include `in_review`

The unarchived event trigger must also fire when a project transitions from `in_review` to `active` (e.g. during `reject_project` reopen flow).

```sql
DROP TRIGGER IF EXISTS trg_project_unarchived;

CREATE TRIGGER trg_project_unarchived
AFTER UPDATE ON projects
WHEN OLD.status IN ('archived', 'completed', 'in_review') AND NEW.status = 'active'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'project.unarchived', 'project', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'slug', NEW.slug, 'name', NEW.name,
      'description', NEW.description, 'owner', NEW.owner,
      'status', NEW.status, 'tags', NEW.tags,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'version', NEW.version,
      'old_status', OLD.status
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;
```

### 4. Service Layer Changes

#### 4a. Read workspace review mode from DB config

Add helper:

```rust
pub async fn get_review_mode(pool: &SqlitePool) -> Result<Option<String>> {
    // Reads config key workflow.review_mode from workspace DB
}
```

Validation can happen in CLI (`task|project|off`) and defensively in service logic.

#### 4b. `complete_task` — task-review aware only

**src/services/task_service.rs** — `complete_task()`:

- If review mode is `task`, set task `status = in_review`
- Else set task `status = done` + `completed_at`
- Keep completion comment logic unchanged
- Do **not** inline project auto-complete logic; trigger remains source of truth

This keeps project transition behavior consistent for all routes to `done`.

#### 4c. Review actions

Add:

- `approve_task(pool, id, comment)` => `in_review -> done`
- `reject_task(pool, id, comment)` => `in_review -> todo`, clear claim, add required review comment
- `approve_project(pool, id, comment)` => `in_review -> completed`
- `reject_project(pool, id, comment)` => **single DB transaction** with ordered operations:
  1. Transition project `in_review -> active`
  2. Transition project tasks `draft -> todo`
  3. Add required review comment with `verdict = rejected`

Ordering is required so task transitions to `todo` happen while the project is already `active`, guaranteeing existing `task.next` triggers can fire.
Reject comments are required for both task and project reject operations.

#### 4d. Review comment helper

Add shared helper to create `CommentKind::Review` comments with structured verdict metadata.

### 5. CLI: `granary review` Command

#### 5a. Args

Add:

- `granary review <id>` (show review context)
- `granary review <id> approve "optional comment"`
- `granary review <task-id> reject "required comment"`
- `granary review <project-id> reject "required comment"`

#### 5b. Context output

Task review context includes:
- task/project metadata
- progress comments
- review history
- steering context
- exact approve/reject commands

Project review context includes:
- project status and task table
- recent activity
- approve command
- explicit reject workflow for follow-up work:
  1. Create follow-up tasks in `draft` state (default)
  2. Reject project review to reopen and enqueue new work

Command example for follow-up tasks (correct CLI shape):

```bash
granary project {project.id} tasks create "Fix: {issue description}"
```

```bash
granary review {project.id} reject "Follow-up tasks created; reopening project for implementation"
```

### 6. Worker Re-pickup After Task Rejection

When `reject_task` sets `in_review -> todo` and clears claim, existing `task.next` trigger flow allows same or different worker to pick it up. No worker runtime changes required.

### 7. `work done` Output Update

`WorkDoneOutput` should reflect actual transition:

- `Submitted for review.` when task becomes `in_review`
- `Done.` when task becomes `done`

Use returned task status from `complete_task` to drive output.

---

## Implementation Order

### Phase 1: Config Command + Workspace Key

1. Add config action for review mode (`granary config review-mode [task|project|off]`)
2. Persist mode in workspace DB key `workflow.review_mode`
3. Keep existing config behavior unchanged for all other keys/commands

### Phase 2: Types & Schema

4. Add `InReview` to `TaskStatus` and `ProjectStatus`
5. Add `Review` to `CommentKind`
6. Add `TaskReview` and `ProjectReview` to `EventType`
7. Create migration:
   - add `trg_task_review`
   - add `trg_project_review`
   - recreate `trg_project_auto_complete` with config-aware status
   - recreate `trg_project_unarchived` to include `in_review`

### Phase 3: Service Layer

8. Add helper to read review mode from workspace DB config
9. Update `complete_task()` for `task` review mode gate only
10. Add `approve_task()`, `reject_task()`, `approve_project()`, `reject_project()`
11. Add `create_review_comment()` helper

### Phase 4: CLI Review Flow

12. Add `Review` command with reject support for task and project IDs
13. Create `src/cli/review.rs` handlers
14. Add `ReviewTaskOutput` and `ReviewProjectOutput`
15. Wire command into `main.rs`
16. Update `WorkDoneOutput` based on returned task status

### Phase 5: Integration

17. Update docs/help text where needed
18. `cargo fmt` and test

---

## Tests

### Trigger behavior tests (integration tests against real SQLite with migrations)

1. **`review_mode: task` — complete_task sets `in_review`**
   - Set `workflow.review_mode = 'task'` in config table
   - Create project + task, start task, complete task
   - Assert task status is `in_review`, not `done`
   - Assert `task.review` event emitted
   - Assert project remains `active` (not auto-completed)

2. **`review_mode: task` — approve_task triggers project auto-complete**
   - Set `workflow.review_mode = 'task'`
   - Create project with single task, complete task (→ `in_review`), approve task (→ `done`)
   - Assert project transitions to `completed` (trigger reads config, sees `task` not `project`)
   - Assert `task.completed` and `project.completed` events emitted

3. **`review_mode: task` — dependent tasks blocked until approval**
   - Set `workflow.review_mode = 'task'`
   - Create project with task A and task B, B depends on A
   - Complete A (→ `in_review`)
   - Assert NO `task.next` event for B (dependency not satisfied)
   - Approve A (→ `done`)
   - Assert `task.next` event for B fires

4. **`review_mode: project` — all tasks done triggers project `in_review`**
   - Set `workflow.review_mode = 'project'`
   - Create project with 2 tasks, complete both
   - Assert both tasks are `done`
   - Assert project status is `in_review`, not `completed`
   - Assert `project.review` event emitted

5. **`review_mode: project` — approve_project completes project**
   - Continue from test 4, approve project
   - Assert project status is `completed`
   - Assert `project.completed` event emitted

6. **No review mode — existing behavior unchanged**
   - No `workflow.review_mode` key in config
   - Complete task → `done`, project auto-completes to `completed`
   - No `task.review` or `project.review` events

7. **Task rejection — back to `todo`, `task.next` fires**
   - Set `workflow.review_mode = 'task'`
   - Complete task (→ `in_review`), reject task with comment
   - Assert task status is `todo`, claim cleared
   - Assert `task.next` event emitted
   - Assert review comment with `kind = 'review'` and `meta.verdict = 'rejected'`

8. **Project rejection workflow reopens and enqueues follow-up work**
   - Set `workflow.review_mode = 'project'`
   - Complete all tasks → project goes to `in_review`
   - Create follow-up tasks (default `draft`)
   - Reject project review
   - Assert project transitions `in_review -> active`
   - Assert project draft tasks transition `draft -> todo`
   - Assert `task.next` events fire for newly actionable tasks
   - Assert `project.unarchived` event fires (with `old_status = 'in_review'`)

9. **`project.next` — project dependency unblocking respects review**
    - Create project A and project B, B depends on A
    - Set `workflow.review_mode = 'project'`
    - Complete all tasks in A → A goes to `in_review`
    - Assert NO `task.next` for tasks in B (A is `in_review`, not `completed`)
    - Approve project A (→ `completed`)
    - Assert `task.next` fires for actionable tasks in B

---

## Notes

- This design intentionally keeps trigger-based project completion semantics so `task update --status done` remains fully functional.
- Project rejection is explicit for project reviews: create follow-up tasks in `draft`, then reject project review to reopen and enqueue work.
