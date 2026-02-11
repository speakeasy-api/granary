-- Trigger-based event system: automatically emit events for all entity mutations.
-- Replaces manual db::events::create() calls with SQLite AFTER triggers.

--------------------------------------------------------------------------------
-- 1. Project triggers
--------------------------------------------------------------------------------

CREATE TRIGGER trg_project_created
AFTER INSERT ON projects
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'project.created', 'project', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'slug', NEW.slug, 'name', NEW.name,
      'description', NEW.description, 'owner', NEW.owner,
      'status', NEW.status, 'tags', NEW.tags,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'version', NEW.version
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_project_updated
AFTER UPDATE ON projects
WHEN OLD.version != NEW.version AND OLD.status = NEW.status
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'project.updated', 'project', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'slug', NEW.slug, 'name', NEW.name,
      'description', NEW.description, 'owner', NEW.owner,
      'status', NEW.status, 'tags', NEW.tags,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'version', NEW.version
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_project_completed
AFTER UPDATE ON projects
WHEN OLD.status != 'completed' AND NEW.status = 'completed'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'project.completed', 'project', NEW.id, NEW.last_edited_by, NULL,
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

CREATE TRIGGER trg_project_archived
AFTER UPDATE ON projects
WHEN OLD.status != 'archived' AND NEW.status = 'archived'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'project.archived', 'project', NEW.id, NEW.last_edited_by, NULL,
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

CREATE TRIGGER trg_project_unarchived
AFTER UPDATE ON projects
WHEN OLD.status IN ('archived', 'completed') AND NEW.status = 'active'
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

--------------------------------------------------------------------------------
-- 2. Task triggers
--------------------------------------------------------------------------------

CREATE TRIGGER trg_task_created
AFTER INSERT ON tasks
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.created', 'task', NEW.id, NEW.last_edited_by, NULL,
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
      'version', NEW.version
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_task_updated
AFTER UPDATE ON tasks
WHEN OLD.version != NEW.version AND OLD.status = NEW.status
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.updated', 'task', NEW.id, NEW.last_edited_by, NULL,
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
      'version', NEW.version
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_task_started
AFTER UPDATE ON tasks
WHEN OLD.status != 'in_progress' AND NEW.status = 'in_progress'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.started', 'task', NEW.id, NEW.last_edited_by, NULL,
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
      'old_status', OLD.status
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_task_completed
AFTER UPDATE ON tasks
WHEN OLD.status != 'done' AND NEW.status = 'done'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.completed', 'task', NEW.id, NEW.last_edited_by, NULL,
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
      'old_status', OLD.status
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_task_blocked
AFTER UPDATE ON tasks
WHEN OLD.status != 'blocked' AND NEW.status = 'blocked'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.blocked', 'task', NEW.id, NEW.last_edited_by, NULL,
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
      'old_status', OLD.status
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_task_unblocked
AFTER UPDATE ON tasks
WHEN OLD.status = 'blocked' AND NEW.status != 'blocked'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.unblocked', 'task', NEW.id, NEW.last_edited_by, NULL,
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
      'old_status', OLD.status
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_task_claimed
AFTER UPDATE ON tasks
WHEN OLD.claim_owner IS NULL AND NEW.claim_owner IS NOT NULL
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.claimed', 'task', NEW.id, NEW.last_edited_by, NULL,
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
      'version', NEW.version
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_task_released
AFTER UPDATE ON tasks
WHEN OLD.claim_owner IS NOT NULL AND NEW.claim_owner IS NULL
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.released', 'task', NEW.id, NEW.last_edited_by, NULL,
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
      'version', NEW.version
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

--------------------------------------------------------------------------------
-- 3. Dependency triggers
--------------------------------------------------------------------------------

CREATE TRIGGER trg_dependency_added
AFTER INSERT ON task_dependencies
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'dependency.added', 'task_dependency', NEW.task_id, NULL, NULL,
    json_object('task_id', NEW.task_id, 'depends_on_task_id', NEW.depends_on_task_id),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_dependency_removed
AFTER DELETE ON task_dependencies
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'dependency.removed', 'task_dependency', OLD.task_id, NULL, NULL,
    json_object('task_id', OLD.task_id, 'depends_on_task_id', OLD.depends_on_task_id),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

--------------------------------------------------------------------------------
-- 4. Comment triggers
--------------------------------------------------------------------------------

CREATE TRIGGER trg_comment_created
AFTER INSERT ON comments
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'comment.created', 'comment', NEW.id, NEW.author, NULL,
    json_object(
      'id', NEW.id, 'parent_type', NEW.parent_type,
      'parent_id', NEW.parent_id, 'comment_number', NEW.comment_number,
      'kind', NEW.kind, 'content', NEW.content,
      'author', NEW.author, 'meta', NEW.meta,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'version', NEW.version
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_comment_updated
AFTER UPDATE ON comments
WHEN OLD.version != NEW.version
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'comment.updated', 'comment', NEW.id, NEW.author, NULL,
    json_object(
      'id', NEW.id, 'parent_type', NEW.parent_type,
      'parent_id', NEW.parent_id, 'comment_number', NEW.comment_number,
      'kind', NEW.kind, 'content', NEW.content,
      'author', NEW.author, 'meta', NEW.meta,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'version', NEW.version
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

--------------------------------------------------------------------------------
-- 5. Session triggers
--------------------------------------------------------------------------------

CREATE TRIGGER trg_session_started
AFTER INSERT ON sessions
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'session.started', 'session', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'name', NEW.name, 'owner', NEW.owner,
      'mode', NEW.mode, 'focus_task_id', NEW.focus_task_id,
      'variables', NEW.variables,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'closed_at', NEW.closed_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_session_updated
AFTER UPDATE ON sessions
WHEN OLD.closed_at IS NULL AND NEW.closed_at IS NULL
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'session.updated', 'session', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'name', NEW.name, 'owner', NEW.owner,
      'mode', NEW.mode, 'focus_task_id', NEW.focus_task_id,
      'variables', NEW.variables,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'closed_at', NEW.closed_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_session_closed
AFTER UPDATE ON sessions
WHEN OLD.closed_at IS NULL AND NEW.closed_at IS NOT NULL
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'session.closed', 'session', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'name', NEW.name, 'owner', NEW.owner,
      'mode', NEW.mode, 'focus_task_id', NEW.focus_task_id,
      'variables', NEW.variables,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'closed_at', NEW.closed_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_session_focus_changed
AFTER UPDATE ON sessions
WHEN OLD.focus_task_id IS NOT NEW.focus_task_id
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'session.focus_changed', 'session', NEW.id, NEW.last_edited_by, NULL,
    json_object(
      'id', NEW.id, 'name', NEW.name, 'owner', NEW.owner,
      'mode', NEW.mode, 'focus_task_id', NEW.focus_task_id,
      'old_focus_task_id', OLD.focus_task_id,
      'variables', NEW.variables,
      'created_at', NEW.created_at, 'updated_at', NEW.updated_at,
      'closed_at', NEW.closed_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_session_scope_added
AFTER INSERT ON session_scope
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'session.scope_added', 'session_scope', NEW.session_id, NULL, NULL,
    json_object(
      'session_id', NEW.session_id, 'item_type', NEW.item_type,
      'item_id', NEW.item_id, 'pinned_at', NEW.pinned_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_session_scope_removed
AFTER DELETE ON session_scope
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'session.scope_removed', 'session_scope', OLD.session_id, NULL, NULL,
    json_object(
      'session_id', OLD.session_id, 'item_type', OLD.item_type,
      'item_id', OLD.item_id, 'pinned_at', OLD.pinned_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

--------------------------------------------------------------------------------
-- 6. Checkpoint/Artifact triggers
--------------------------------------------------------------------------------

CREATE TRIGGER trg_checkpoint_created
AFTER INSERT ON checkpoints
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'checkpoint.created', 'checkpoint', NEW.id, NULL, NEW.session_id,
    json_object(
      'id', NEW.id, 'session_id', NEW.session_id,
      'name', NEW.name, 'snapshot', NEW.snapshot,
      'created_at', NEW.created_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_artifact_added
AFTER INSERT ON artifacts
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'artifact.added', 'artifact', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'parent_type', NEW.parent_type,
      'parent_id', NEW.parent_id, 'artifact_number', NEW.artifact_number,
      'artifact_type', NEW.artifact_type, 'path_or_url', NEW.path_or_url,
      'description', NEW.description, 'meta', NEW.meta,
      'created_at', NEW.created_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

CREATE TRIGGER trg_artifact_removed
AFTER DELETE ON artifacts
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'artifact.removed', 'artifact', OLD.id, NULL, NULL,
    json_object(
      'id', OLD.id, 'parent_type', OLD.parent_type,
      'parent_id', OLD.parent_id, 'artifact_number', OLD.artifact_number,
      'artifact_type', OLD.artifact_type, 'path_or_url', OLD.path_or_url,
      'description', OLD.description, 'meta', OLD.meta,
      'created_at', OLD.created_at
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

--------------------------------------------------------------------------------
-- 7. task.next triggers (fire when a task becomes actionable)
--------------------------------------------------------------------------------

-- 7a. New task created as 'todo' with no blockers
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
  VALUES (
    'task.next', 'task', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

-- 7b. Task transitions to 'todo' status
CREATE TRIGGER trg_task_next_on_status_todo
AFTER UPDATE ON tasks
WHEN NEW.status = 'todo'
  AND NEW.blocked_reason IS NULL
  AND (NEW.claim_owner IS NULL OR NEW.claim_lease_expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
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
  VALUES (
    'task.next', 'task', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

-- 7c. Dependency completed → unblocks dependents
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
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  FROM tasks t
  JOIN task_dependencies td ON td.task_id = t.id
  WHERE td.depends_on_task_id = NEW.id
    AND t.status = 'todo'
    AND t.blocked_reason IS NULL
    AND (t.claim_owner IS NULL OR t.claim_lease_expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
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

-- 7d. Task unblocked (blocked_reason cleared)
CREATE TRIGGER trg_task_next_on_unblocked
AFTER UPDATE ON tasks
WHEN OLD.blocked_reason IS NOT NULL AND NEW.blocked_reason IS NULL
  AND NEW.status = 'todo'
  AND (NEW.claim_owner IS NULL OR NEW.claim_lease_expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
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
  VALUES (
    'task.next', 'task', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

-- 7e. Task released (claim cleared)
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
  VALUES (
    'task.next', 'task', NEW.id, NULL, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'title', NEW.title, 'priority', NEW.priority,
      'status', NEW.status, 'owner', NEW.owner
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

-- 7f. Dependency removed → task may become actionable
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
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  FROM tasks t
  WHERE t.id = OLD.task_id
    AND t.status = 'todo'
    AND t.blocked_reason IS NULL
    AND (t.claim_owner IS NULL OR t.claim_lease_expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
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

--------------------------------------------------------------------------------
-- 8. project.next trigger (fires when a task.next event is emitted)
--    Requires PRAGMA recursive_triggers = ON set on connection open.
--------------------------------------------------------------------------------

CREATE TRIGGER trg_project_next_on_task_next
AFTER INSERT ON events
WHEN NEW.event_type = 'task.next'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  SELECT 'project.next', 'project', p.id, NULL, NULL,
    json_object(
      'id', p.id, 'name', p.name, 'status', p.status
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  FROM projects p
  WHERE p.id = json_extract(NEW.payload, '$.project_id')
    AND p.status = 'active';
END;

--------------------------------------------------------------------------------
-- 9. Project auto-complete: when all tasks in a project are done
--------------------------------------------------------------------------------

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
  SET status = 'completed', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), version = version + 1
  WHERE id = NEW.project_id;
END;

--------------------------------------------------------------------------------
-- 10. Project auto-reactivate: when a task is added to a completed project
--------------------------------------------------------------------------------

CREATE TRIGGER trg_project_auto_reactivate
AFTER INSERT ON tasks
WHEN EXISTS (
  SELECT 1 FROM projects p
  WHERE p.id = NEW.project_id AND p.status = 'completed'
)
BEGIN
  UPDATE projects
  SET status = 'active', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), version = version + 1
  WHERE id = NEW.project_id;
END;
