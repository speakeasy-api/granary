# Trigger-Based Event System

## Summary

Replace granary's programmatic event emission (`db::events::create()` calls scattered across service functions) with SQLite `AFTER INSERT/UPDATE` triggers. Events become an automatic, guaranteed consequence of data mutations — impossible to forget, atomic with the change, and correct by construction.

This also replaces the polled `task.next` / `project.next` system with trigger-emitted events, eliminating `PolledEventEmitter`, in-memory cooldown tracking, and the special-case code paths in `WorkerRuntime`.

---

## Problem Statement

### Current State

- Every service function manually calls `db::events::create()` after mutations
- Easy to forget (we just shipped `project done` / `project unarchive` and almost missed events)
- `task.next` and `project.next` are "synthetic" polled events — not persisted, generated on-demand with in-memory cooldown state
- Workers have two code paths: regular event polling vs. polled event emission
- Event payloads are hand-assembled JSON fragments, often incomplete or inconsistent
- Context fields (`actor`, `session_id`) are threaded manually through every call

### Target State

- Triggers on `projects`, `tasks`, `comments`, `sessions`, `checkpoints`, `artifacts`, `task_dependencies` fire events automatically
- Payload is always a full JSON snapshot of the entity (via `json_object()`)
- `actor` context lives on the entity itself (`last_edited_by` column) — triggers read it
- `task.next` events are emitted by triggers when a task becomes actionable, not by polling
- Workers have one code path: consume events from the `events` table
- `PolledEventEmitter`, `polled_events.rs`, and the worker special-casing are deleted
- Service functions contain only business logic — no event boilerplate

---

## Design

### 1. Schema Changes

#### Add `last_edited_by` to mutable entities

```sql
ALTER TABLE projects ADD COLUMN last_edited_by TEXT;
ALTER TABLE tasks ADD COLUMN last_edited_by TEXT;
ALTER TABLE sessions ADD COLUMN last_edited_by TEXT;
```

Service functions set `last_edited_by` on the entity before saving. Triggers read it from `NEW.last_edited_by` and write it to `events.actor`. This replaces the `actor` parameter on `CreateEvent`.

#### Add index for trigger-emitted `.next` events

```sql
CREATE INDEX IF NOT EXISTS idx_events_type_entity ON events(event_type, entity_id);
```

### 2. Trigger Catalog

Every trigger follows the same pattern:

```sql
CREATE TRIGGER <name>
AFTER INSERT|UPDATE ON <table>
WHEN <condition>
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (<type>, <entity>, NEW.id, NEW.last_edited_by, NULL, json_object(...), datetime('now'));
END;
```

Payload is always the full entity snapshot via `json_object()` with all columns.

#### 2a. Project Triggers

| Trigger                  | Fires on                   | Condition                                                          | Event Type           |
| ------------------------ | -------------------------- | ------------------------------------------------------------------ | -------------------- |
| `trg_project_created`    | `AFTER INSERT ON projects` | —                                                                  | `project.created`    |
| `trg_project_updated`    | `AFTER UPDATE ON projects` | `OLD.version != NEW.version AND OLD.status = NEW.status`           | `project.updated`    |
| `trg_project_completed`  | `AFTER UPDATE ON projects` | `OLD.status != 'completed' AND NEW.status = 'completed'`           | `project.completed`  |
| `trg_project_archived`   | `AFTER UPDATE ON projects` | `OLD.status != 'archived' AND NEW.status = 'archived'`             | `project.archived`   |
| `trg_project_unarchived` | `AFTER UPDATE ON projects` | `OLD.status IN ('archived','completed') AND NEW.status = 'active'` | `project.unarchived` |

Payload:

```sql
json_object(
  'id', NEW.id, 'slug', NEW.slug, 'name', NEW.name,
  'description', NEW.description, 'owner', NEW.owner,
  'status', NEW.status, 'tags', NEW.tags,
  'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
  'version', NEW.version,
  'old_status', OLD.status  -- for status-change triggers
)
```

#### 2b. Task Triggers

| Trigger              | Fires on                | Condition                                                    | Event Type       |
| -------------------- | ----------------------- | ------------------------------------------------------------ | ---------------- |
| `trg_task_created`   | `AFTER INSERT ON tasks` | —                                                            | `task.created`   |
| `trg_task_updated`   | `AFTER UPDATE ON tasks` | `OLD.version != NEW.version AND OLD.status = NEW.status`     | `task.updated`   |
| `trg_task_started`   | `AFTER UPDATE ON tasks` | `OLD.status != 'in_progress' AND NEW.status = 'in_progress'` | `task.started`   |
| `trg_task_completed` | `AFTER UPDATE ON tasks` | `OLD.status != 'done' AND NEW.status = 'done'`               | `task.completed` |
| `trg_task_blocked`   | `AFTER UPDATE ON tasks` | `OLD.status != 'blocked' AND NEW.status = 'blocked'`         | `task.blocked`   |
| `trg_task_unblocked` | `AFTER UPDATE ON tasks` | `OLD.status = 'blocked' AND NEW.status != 'blocked'`         | `task.unblocked` |
| `trg_task_claimed`   | `AFTER UPDATE ON tasks` | `OLD.claim_owner IS NULL AND NEW.claim_owner IS NOT NULL`    | `task.claimed`   |
| `trg_task_released`  | `AFTER UPDATE ON tasks` | `OLD.claim_owner IS NOT NULL AND NEW.claim_owner IS NULL`    | `task.released`  |

Note on `task.status_changed`: the current code emits this as a catch-all when status changes during `update_task`. With triggers, every specific status transition already has its own event (`started`, `completed`, `blocked`, `unblocked`). The generic `task.updated` trigger fires for non-status changes. `task.status_changed` is retired — consumers should subscribe to specific transitions.

Payload (same full-snapshot pattern):

```sql
json_object(
  'id', NEW.id, 'project_id', NEW.project_id,
  'task_number', NEW.task_number, 'title', NEW.title,
  'description', NEW.description, 'status', NEW.status,
  'priority', NEW.priority, 'owner', NEW.owner,
  'tags', NEW.tags, 'blocked_reason', NEW.blocked_reason,
  'started_at', NEW.started_at, 'completed_at', NEW.completed_at,
  'due_at', NEW.due_at,
  'claim_owner', NEW.claim_owner,
  'pinned', NEW.pinned, 'focus_weight', NEW.focus_weight,
  'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
  'version', NEW.version,
  'old_status', OLD.status  -- for status-change triggers
)
```

#### 2c. Task Dependency Triggers

| Trigger                  | Fires on                            | Condition | Event Type           |
| ------------------------ | ----------------------------------- | --------- | -------------------- |
| `trg_dependency_added`   | `AFTER INSERT ON task_dependencies` | —         | `dependency.added`   |
| `trg_dependency_removed` | `AFTER DELETE ON task_dependencies` | —         | `dependency.removed` |

Payload:

```sql
json_object('task_id', NEW.task_id, 'depends_on_task_id', NEW.depends_on_task_id)
```

Note: dependency removal uses `AFTER DELETE` (not `AFTER UPDATE`), so it references `OLD.*`.

#### 2d. Comment Triggers

| Trigger               | Fires on                   | Condition                    | Event Type        |
| --------------------- | -------------------------- | ---------------------------- | ----------------- |
| `trg_comment_created` | `AFTER INSERT ON comments` | —                            | `comment.created` |
| `trg_comment_updated` | `AFTER UPDATE ON comments` | `OLD.version != NEW.version` | `comment.updated` |

#### 2e. Session Triggers

| Trigger                     | Fires on                        | Condition                                             | Event Type              |
| --------------------------- | ------------------------------- | ----------------------------------------------------- | ----------------------- |
| `trg_session_started`       | `AFTER INSERT ON sessions`      | —                                                     | `session.started`       |
| `trg_session_updated`       | `AFTER UPDATE ON sessions`      | `OLD.closed_at IS NULL AND NEW.closed_at IS NULL`     | `session.updated`       |
| `trg_session_closed`        | `AFTER UPDATE ON sessions`      | `OLD.closed_at IS NULL AND NEW.closed_at IS NOT NULL` | `session.closed`        |
| `trg_session_scope_added`   | `AFTER INSERT ON session_scope` | —                                                     | `session.scope_added`   |
| `trg_session_scope_removed` | `AFTER DELETE ON session_scope` | —                                                     | `session.scope_removed` |
| `trg_session_focus_changed` | `AFTER UPDATE ON sessions`      | `OLD.focus_task_id IS NOT NEW.focus_task_id`          | `session.focus_changed` |

Note: `session_scope` doesn't have `last_edited_by`. These events fire with `actor = NULL` (same as today). `session.focus_changed` condition uses `IS NOT` (handles NULL comparison correctly in SQLite).

#### 2f. Checkpoint/Artifact Triggers

| Trigger                  | Fires on                      | Condition | Event Type           |
| ------------------------ | ----------------------------- | --------- | -------------------- |
| `trg_checkpoint_created` | `AFTER INSERT ON checkpoints` | —         | `checkpoint.created` |
| `trg_artifact_added`     | `AFTER INSERT ON artifacts`   | —         | `artifact.added`     |
| `trg_artifact_removed`   | `AFTER DELETE ON artifacts`   | —         | `artifact.removed`   |

Note: `checkpoint.restored` has no trigger equivalent because restoring is an application-level batch operation (multiple UPDATEs). It is not currently consumed by anything, so we drop it entirely rather than keeping it as a special case. It can be re-added later if needed.

### 3. Replacing `.next` Events with Triggers

This is the most impactful change. Today, `task.next` and `project.next` are synthetic events generated by polling queries in `PolledEventEmitter`. With triggers, we emit them reactively when a task **becomes actionable**.

A task is actionable when:

- `status = 'todo'`
- `blocked_reason IS NULL`
- Not claimed (`claim_owner IS NULL` or lease expired)
- All dependencies are done (no row in `task_dependencies` pointing to a non-done task)
- Project is active (`projects.status = 'active'`)

#### 3a. `task.next` Triggers

There are **five entry points** that can make a task actionable:

**1. Task created with status 'todo'** (or transitioned to 'todo')

```sql
CREATE TRIGGER trg_task_next_on_status_todo
AFTER UPDATE ON tasks
WHEN NEW.status = 'todo'
  AND NEW.blocked_reason IS NULL
  AND (NEW.claim_owner IS NULL OR NEW.claim_lease_expires_at < datetime('now'))
  AND NOT EXISTS (
    SELECT 1 FROM task_dependencies td
    JOIN tasks dep ON dep.id = td.depends_on_task_id
    WHERE td.task_id = NEW.id AND dep.status != 'done'
  )
  AND EXISTS (
    SELECT 1 FROM projects p WHERE p.id = NEW.project_id AND p.status = 'active'
  )
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES ('task.next', 'task', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    datetime('now'));
END;
```

Also for INSERT (new task created as 'todo' directly):

```sql
CREATE TRIGGER trg_task_next_on_insert_todo
AFTER INSERT ON tasks
WHEN NEW.status = 'todo'
  AND NEW.blocked_reason IS NULL
  AND NOT EXISTS (
    SELECT 1 FROM task_dependencies td
    JOIN tasks dep ON dep.id = td.depends_on_task_id
    WHERE td.task_id = NEW.id AND dep.status != 'done'
  )
  AND EXISTS (
    SELECT 1 FROM projects p WHERE p.id = NEW.project_id AND p.status = 'active'
  )
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES ('task.next', 'task', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    datetime('now'));
END;
```

**2. Dependency completed → unblocks dependents**

When a task completes, find tasks that were waiting on it and are now fully unblocked:

```sql
CREATE TRIGGER trg_task_next_on_dep_completed
AFTER UPDATE ON tasks
WHEN OLD.status != 'done' AND NEW.status = 'done'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  SELECT 'task.next', 'task', t.id, NULL, NULL,
    json_object(
      'id', t.id, 'project_id', t.project_id,
      'title', t.title, 'priority', t.priority,
      'status', t.status, 'owner', t.owner
    ),
    datetime('now')
  FROM tasks t
  JOIN task_dependencies td ON td.task_id = t.id
  WHERE td.depends_on_task_id = NEW.id
    AND t.status = 'todo'
    AND t.blocked_reason IS NULL
    AND (t.claim_owner IS NULL OR t.claim_lease_expires_at < datetime('now'))
    AND NOT EXISTS (
      SELECT 1 FROM task_dependencies td2
      JOIN tasks dep ON dep.id = td2.depends_on_task_id
      WHERE td2.task_id = t.id
        AND dep.id != NEW.id
        AND dep.status != 'done'
    )
    AND EXISTS (
      SELECT 1 FROM projects p WHERE p.id = t.project_id AND p.status = 'active'
    );
END;
```

**3. Task unblocked** (blocked_reason cleared)

```sql
CREATE TRIGGER trg_task_next_on_unblocked
AFTER UPDATE ON tasks
WHEN OLD.blocked_reason IS NOT NULL AND NEW.blocked_reason IS NULL
  AND NEW.status = 'todo'
  AND (NEW.claim_owner IS NULL OR NEW.claim_lease_expires_at < datetime('now'))
  AND NOT EXISTS (
    SELECT 1 FROM task_dependencies td
    JOIN tasks dep ON dep.id = td.depends_on_task_id
    WHERE td.task_id = NEW.id AND dep.status != 'done'
  )
  AND EXISTS (
    SELECT 1 FROM projects p WHERE p.id = NEW.project_id AND p.status = 'active'
  )
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES ('task.next', 'task', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    datetime('now'));
END;
```

**4. Task released** (claim cleared)

```sql
CREATE TRIGGER trg_task_next_on_released
AFTER UPDATE ON tasks
WHEN OLD.claim_owner IS NOT NULL AND NEW.claim_owner IS NULL
  AND NEW.status = 'todo'
  AND NEW.blocked_reason IS NULL
  AND NOT EXISTS (
    SELECT 1 FROM task_dependencies td
    JOIN tasks dep ON dep.id = td.depends_on_task_id
    WHERE td.task_id = NEW.id AND dep.status != 'done'
  )
  AND EXISTS (
    SELECT 1 FROM projects p WHERE p.id = NEW.project_id AND p.status = 'active'
  )
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES ('task.next', 'task', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    datetime('now'));
END;
```

**5. Dependency removed** → task may become unblocked

```sql
CREATE TRIGGER trg_task_next_on_dep_removed
AFTER DELETE ON task_dependencies
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  SELECT 'task.next', 'task', t.id, NULL, NULL,
    json_object(
      'id', t.id, 'project_id', t.project_id,
      'title', t.title, 'priority', t.priority,
      'status', t.status, 'owner', t.owner
    ),
    datetime('now')
  FROM tasks t
  WHERE t.id = OLD.task_id
    AND t.status = 'todo'
    AND t.blocked_reason IS NULL
    AND (t.claim_owner IS NULL OR t.claim_lease_expires_at < datetime('now'))
    AND NOT EXISTS (
      SELECT 1 FROM task_dependencies td2
      JOIN tasks dep ON dep.id = td2.depends_on_task_id
      WHERE td2.task_id = t.id
        AND dep.status != 'done'
    )
    AND EXISTS (
      SELECT 1 FROM projects p WHERE p.id = t.project_id AND p.status = 'active'
    );
END;
```

#### 3b. `project.next` Triggers

`project.next` fires when a project has at least one actionable task. Rather than maintaining separate triggers, derive it from `task.next`:

```sql
CREATE TRIGGER trg_project_next_on_task_next
AFTER INSERT ON events
WHEN NEW.event_type = 'task.next'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  SELECT 'project.next', 'project', p.id, NULL, NULL,
    json_object(
      'id', p.id, 'name', p.name, 'status', p.status
    ),
    datetime('now')
  FROM projects p
  WHERE p.id = json_extract(NEW.payload, '$.project_id')
    AND p.status = 'active';
END;
```

This requires `PRAGMA recursive_triggers = ON` since it's a trigger on the `events` table that itself inserts into `events`. Set this pragma on connection open.

**Alternative** (if recursive triggers are undesirable): emit `project.next` from the same triggers that emit `task.next`, alongside the task event. This duplicates the project lookup but avoids recursion.

#### 3c. Implicit Project Completion via Trigger

When a task completes and all sibling tasks are also done, auto-complete the project:

```sql
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
  SET status = 'completed', updated_at = datetime('now'), version = version + 1
  WHERE id = NEW.project_id;
END;
```

The `project.completed` event is then emitted by `trg_project_completed` — no programmatic code needed.

#### 3d. Implicit Project Reactivation via Trigger

When a task is created in a completed project, reactivate it:

```sql
CREATE TRIGGER trg_project_auto_reactivate
AFTER INSERT ON tasks
WHEN EXISTS (
  SELECT 1 FROM projects p
  WHERE p.id = NEW.project_id AND p.status = 'completed'
)
BEGIN
  UPDATE projects
  SET status = 'active', updated_at = datetime('now'), version = version + 1
  WHERE id = NEW.project_id;
END;
```

The `project.unarchived` event is then emitted by `trg_project_unarchived`.

### 4. Deduplication

Trigger-emitted `task.next` events may fire multiple times for the same task (e.g., a dependency completes AND the task is unblocked in the same transaction). This is fine — the `EventConsumerService` already uses claim-based consumption with `event_consumptions` tracking. Workers process each event independently. If a task was already claimed by the time the second event is processed, the worker skips it (existing concurrency logic).

If deduplication is desired at the DB level, add a unique constraint or check:

```sql
-- Optional: skip if a task.next for this entity was emitted in the last N seconds
AND NOT EXISTS (
  SELECT 1 FROM events
  WHERE event_type = 'task.next'
    AND entity_id = NEW.id
    AND created_at > datetime('now', '-5 seconds')
)
```

This is optional. The consumer layer already handles it.

### 5. What Gets Deleted

| File/Component                                                              | Status                                                          |
| --------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `src/services/polled_events.rs`                                             | **Delete entirely**                                             |
| `PolledEventEmitter` usage in `worker_runtime.rs`                           | **Remove** — workers just use `EventPoller` for all event types |
| `polled_emitter` field in `WorkerRuntime`                                   | **Remove**                                                      |
| Special-casing for `task.next`/`project.next` in `poll_and_handle_events()` | **Remove** — unified path                                       |
| All `db::events::create()` calls in service functions                       | **Remove**                                                      |
| `CreateEvent` struct usage in services                                      | **Remove**                                                      |
| `EventType` enum in `granary-types`                                         | **Keep** — still used for filtering/display                     |
| `db::events::create()` function                                             | **Delete**                                                      |
| `db::tasks::all_done_in_project()`                                          | **Remove** — replaced by `trg_project_auto_complete`            |
| Auto-complete logic in `task_service::complete_task`                        | **Remove** — trigger handles it                                 |
| Auto-reactivate logic in `task_service::create_task`                        | **Remove** — trigger handles it                                 |

### 6. Worker Runtime Simplification

Before (two code paths):

```rust
let events = if let Some(ref mut emitter) = self.polled_emitter {
    match self.worker.event_type.as_str() {
        "task.next" => emitter.poll_task_next(&self.workspace_pool, None).await?,
        "project.next" => emitter.poll_project_next(&self.workspace_pool).await?,
        _ => vec![],
    }
} else {
    self.poller.poll().await?
};
```

After (one code path):

```rust
let events = self.poller.poll().await?;
```

All event types — including `task.next` and `project.next` — are now persisted rows in the `events` table. Workers consume them identically.

---

## Migration Strategy

### Phase 1: Schema + Triggers (non-breaking)

1. Add migration: `ALTER TABLE` for `last_edited_by` columns
2. Add migration: all `CREATE TRIGGER` statements
3. Set `PRAGMA recursive_triggers = ON` in connection setup (if using `project.next` chained trigger)
4. Add index: `idx_events_type_entity`

At this point, **both** programmatic events and trigger events fire. Events are duplicated but that's safe — consumers are idempotent.

### Phase 2: Remove Programmatic Events

1. Remove all `db::events::create()` calls from service functions
2. Remove `db::events::create()` function itself
3. Set `last_edited_by` in service functions where `actor` was previously set
4. Remove `CreateEvent` struct and its usage
5. Remove auto-complete/auto-reactivate logic from `task_service.rs` (triggers do it)

### Phase 3: Remove Polling Infrastructure

1. Delete `src/services/polled_events.rs`
2. Remove `PolledEventEmitter` from `WorkerRuntime`
3. Remove `polled_emitter` field and the `if let Some(emitter)` branch
4. Remove `poll_cooldown_secs` from `Worker` model (no longer needed)
5. Simplify `poll_and_handle_events` to single code path

### Phase 4: Cleanup

1. Remove `TaskNext`/`ProjectNext` special handling from `EventType` (they're now regular persisted events)
2. Remove `TaskStatusChanged` from `EventType` (replaced by specific transition events)
3. Clean up any dead code (`list_with_available_tasks`, `get_all_next` if unused)
4. Update tests

---

## Event Type Changes

| Old                        | New                        | Notes                                                                          |
| -------------------------- | -------------------------- | ------------------------------------------------------------------------------ |
| `task.status_changed`      | _removed_                  | Replaced by `task.started`, `task.completed`, `task.blocked`, `task.unblocked` |
| `task.next` (synthetic)    | `task.next` (persisted)    | Same semantics, now in `events` table                                          |
| `project.next` (synthetic) | `project.next` (persisted) | Same semantics, now in `events` table                                          |
| `project.completed`        | `project.completed`        | New — was missing before                                                       |
| `project.unarchived`       | `project.unarchived`       | New — was missing before                                                       |

All other event types remain unchanged in name and semantics.

---

## Risks and Mitigations

| Risk                                                         | Mitigation                                                                                                                                                                |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Trigger bugs are harder to debug than Rust code              | Test each trigger in isolation with SQL test fixtures. Log trigger-emitted events in `granary events` for visibility.                                                     |
| `task.next` may emit duplicates                              | Consumer layer already handles this. Optional dedup window in trigger WHEN clause.                                                                                        |
| Recursive triggers (`project.next` from `task.next`)         | Use `PRAGMA recursive_triggers = ON`. Alternative: co-emit in same trigger body.                                                                                          |
| Performance: triggers add overhead to every write            | SQLite triggers are fast. The `NOT EXISTS` subqueries in `.next` triggers are the only concern — mitigated by existing indexes on `task_dependencies` and `tasks.status`. |
| Migration: existing events table has no trigger-emitted rows | Phase 1 runs both systems in parallel. Phase 2 cuts over. No data migration needed.                                                                                       |
| `checkpoint.restored` dropped                                | Not consumed by anything. Re-add as a trigger if needed later.                                                                                                            |
