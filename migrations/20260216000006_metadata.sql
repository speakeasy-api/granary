-- Add metadata column (free-form JSON) to tasks, projects, and initiatives.
-- Metadata is optional, stored as TEXT containing a JSON object.
-- Only exposed in --json / --format json output, not in text or prompt formats.

ALTER TABLE tasks ADD COLUMN metadata TEXT;
ALTER TABLE projects ADD COLUMN metadata TEXT;
ALTER TABLE initiatives ADD COLUMN metadata TEXT;

--------------------------------------------------------------------------------
-- Recreate task event triggers to include metadata in payloads.
-- We use json(NEW.metadata) so the value is embedded as a JSON object
-- rather than a string, enabling nested template access like {metadata.key}.
--------------------------------------------------------------------------------

-- Task CRUD triggers (from 20260211000001_event_triggers.sql)

DROP TRIGGER IF EXISTS trg_task_created;
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
      'version', NEW.version,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_updated;
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
      'version', NEW.version,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_started;
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
      'old_status', OLD.status,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_completed;
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
      'old_status', OLD.status,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_blocked;
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
      'old_status', OLD.status,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_unblocked;
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
      'old_status', OLD.status,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_claimed;
CREATE TRIGGER trg_task_claimed
AFTER UPDATE ON tasks
WHEN NEW.claim_owner IS NOT NULL AND (OLD.claim_owner IS NULL OR OLD.claim_owner != NEW.claim_owner)
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.claimed', 'task', NEW.id, NEW.claim_owner, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'task_number', NEW.task_number, 'title', NEW.title,
      'description', NEW.description, 'status', NEW.status,
      'priority', NEW.priority, 'owner', NEW.owner,
      'claim_owner', NEW.claim_owner,
      'claim_claimed_at', NEW.claim_claimed_at,
      'claim_lease_expires_at', NEW.claim_lease_expires_at,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_released;
CREATE TRIGGER trg_task_released
AFTER UPDATE ON tasks
WHEN OLD.claim_owner IS NOT NULL AND NEW.claim_owner IS NULL
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  VALUES (
    'task.released', 'task', NEW.id, OLD.claim_owner, NULL,
    json_object(
      'id', NEW.id, 'project_id', NEW.project_id,
      'task_number', NEW.task_number, 'title', NEW.title,
      'description', NEW.description, 'status', NEW.status,
      'priority', NEW.priority, 'owner', NEW.owner,
      'claim_owner', NEW.claim_owner,
      'claim_claimed_at', NEW.claim_claimed_at,
      'claim_lease_expires_at', NEW.claim_lease_expires_at,
      'old_claim_owner', OLD.claim_owner,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

-- Task review trigger (from 20260216000005_review_mode_triggers.sql)
DROP TRIGGER IF EXISTS trg_task_review;
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
      'status', NEW.status, 'owner', NEW.owner,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

--------------------------------------------------------------------------------
-- Task.next triggers (from 20260216000003 + 20260216000004)
-- Add metadata to payload
--------------------------------------------------------------------------------

DROP TRIGGER IF EXISTS trg_task_next_on_project_dep_completed;
CREATE TRIGGER trg_task_next_on_project_dep_completed
AFTER UPDATE ON projects
WHEN NEW.status IN ('done', 'completed', 'archived') AND OLD.status NOT IN ('done', 'completed', 'archived')
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  SELECT 'task.next', 'task', t.id, NULL, NULL,
    json_object(
      'id', t.id, 'project_id', t.project_id,
      'title', t.title, 'priority', t.priority,
      'status', t.status, 'owner', t.owner,
      'metadata', json(t.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  FROM tasks t
  JOIN project_dependencies pd ON pd.project_id = t.project_id
  WHERE pd.depends_on_project_id = NEW.id
    AND t.status = 'todo'
    AND t.blocked_reason IS NULL
    AND (t.claim_owner IS NULL OR t.claim_lease_expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    AND NOT EXISTS (
      SELECT 1 FROM task_dependencies td
      JOIN tasks dep ON dep.id = td.depends_on_task_id
      WHERE td.task_id = t.id
        AND dep.status != 'done'
    )
    AND NOT EXISTS (
      SELECT 1 FROM project_dependencies pd2
      JOIN projects dep_p ON dep_p.id = pd2.depends_on_project_id
      WHERE pd2.project_id = t.project_id
        AND pd2.depends_on_project_id != NEW.id
        AND dep_p.status NOT IN ('done', 'completed', 'archived')
        AND EXISTS (
          SELECT 1 FROM tasks dep_t
          WHERE dep_t.project_id = pd2.depends_on_project_id
            AND dep_t.status != 'done'
        )
    )
    AND EXISTS (
      SELECT 1 FROM projects p WHERE p.id = t.project_id AND p.status = 'active'
    );
END;

DROP TRIGGER IF EXISTS trg_task_next_on_insert_todo;
CREATE TRIGGER trg_task_next_on_insert_todo
AFTER INSERT ON tasks
WHEN NEW.status = 'todo'
  AND NEW.blocked_reason IS NULL
  AND NOT EXISTS (
    SELECT 1 FROM task_dependencies td
    JOIN tasks dep ON dep.id = td.depends_on_task_id
    WHERE td.task_id = NEW.id AND dep.status != 'done'
  )
  AND NOT EXISTS (
    SELECT 1 FROM project_dependencies pd
    JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
    WHERE pd.project_id = NEW.project_id
      AND dep_p.status NOT IN ('done', 'completed', 'archived')
      AND EXISTS (
        SELECT 1 FROM tasks dep_t
        WHERE dep_t.project_id = pd.depends_on_project_id
          AND dep_t.status != 'done'
      )
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
      'status', NEW.status, 'owner', NEW.owner,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_next_on_status_todo;
CREATE TRIGGER trg_task_next_on_status_todo
AFTER UPDATE ON tasks
WHEN NEW.status = 'todo'
  AND OLD.status != 'todo'
  AND NEW.blocked_reason IS NULL
  AND (NEW.claim_owner IS NULL OR NEW.claim_lease_expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
  AND NOT EXISTS (
    SELECT 1 FROM task_dependencies td
    JOIN tasks dep ON dep.id = td.depends_on_task_id
    WHERE td.task_id = NEW.id AND dep.status != 'done'
  )
  AND NOT EXISTS (
    SELECT 1 FROM project_dependencies pd
    JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
    WHERE pd.project_id = NEW.project_id
      AND dep_p.status NOT IN ('done', 'completed', 'archived')
      AND EXISTS (
        SELECT 1 FROM tasks dep_t
        WHERE dep_t.project_id = pd.depends_on_project_id
          AND dep_t.status != 'done'
      )
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
      'status', NEW.status, 'owner', NEW.owner,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_next_on_dep_completed;
CREATE TRIGGER trg_task_next_on_dep_completed
AFTER UPDATE ON tasks
WHEN OLD.status != 'done' AND NEW.status = 'done'
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  SELECT 'task.next', 'task', t.id, NULL, NULL,
    json_object(
      'id', t.id, 'project_id', t.project_id,
      'title', t.title, 'priority', t.priority,
      'status', t.status, 'owner', t.owner,
      'metadata', json(t.metadata)
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
    AND NOT EXISTS (
      SELECT 1 FROM project_dependencies pd
      JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
      WHERE pd.project_id = t.project_id
        AND dep_p.status NOT IN ('done', 'completed', 'archived')
        AND EXISTS (
          SELECT 1 FROM tasks dep_t
          WHERE dep_t.project_id = pd.depends_on_project_id
            AND dep_t.status != 'done'
        )
    )
    AND EXISTS (
      SELECT 1 FROM projects p WHERE p.id = t.project_id AND p.status = 'active'
    );
END;

DROP TRIGGER IF EXISTS trg_task_next_on_unblocked;
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
  AND NOT EXISTS (
    SELECT 1 FROM project_dependencies pd
    JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
    WHERE pd.project_id = NEW.project_id
      AND dep_p.status NOT IN ('done', 'completed', 'archived')
      AND EXISTS (
        SELECT 1 FROM tasks dep_t
        WHERE dep_t.project_id = pd.depends_on_project_id
          AND dep_t.status != 'done'
      )
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
      'status', NEW.status, 'owner', NEW.owner,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_next_on_released;
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
  AND NOT EXISTS (
    SELECT 1 FROM project_dependencies pd
    JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
    WHERE pd.project_id = NEW.project_id
      AND dep_p.status NOT IN ('done', 'completed', 'archived')
      AND EXISTS (
        SELECT 1 FROM tasks dep_t
        WHERE dep_t.project_id = pd.depends_on_project_id
          AND dep_t.status != 'done'
      )
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
      'status', NEW.status, 'owner', NEW.owner,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_task_next_on_dep_removed;
CREATE TRIGGER trg_task_next_on_dep_removed
AFTER DELETE ON task_dependencies
BEGIN
  INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
  SELECT 'task.next', 'task', t.id, NULL, NULL,
    json_object(
      'id', t.id, 'project_id', t.project_id,
      'title', t.title, 'priority', t.priority,
      'status', t.status, 'owner', t.owner,
      'metadata', json(t.metadata)
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
    AND NOT EXISTS (
      SELECT 1 FROM project_dependencies pd
      JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
      WHERE pd.project_id = t.project_id
        AND dep_p.status NOT IN ('done', 'completed', 'archived')
        AND EXISTS (
          SELECT 1 FROM tasks dep_t
          WHERE dep_t.project_id = pd.depends_on_project_id
            AND dep_t.status != 'done'
        )
    )
    AND EXISTS (
      SELECT 1 FROM projects p WHERE p.id = t.project_id AND p.status = 'active'
    );
END;

--------------------------------------------------------------------------------
-- Project event triggers (from 20260211000001_event_triggers.sql + 20260216000005)
-- Add metadata to payload
--------------------------------------------------------------------------------

DROP TRIGGER IF EXISTS trg_project_created;
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
      'version', NEW.version,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_project_updated;
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
      'version', NEW.version,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_project_completed;
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
      'old_status', OLD.status,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_project_archived;
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
      'old_status', OLD.status,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

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
      'old_status', OLD.status,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

DROP TRIGGER IF EXISTS trg_project_review;
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
      'status', NEW.status, 'owner', NEW.owner,
      'metadata', json(NEW.metadata)
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;
