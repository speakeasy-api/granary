-- Fix: task.next and project.next must respect project dependencies
-- Previously only task dependencies were checked, so tasks in projects blocked
-- by project dependencies would incorrectly appear as actionable.

--------------------------------------------------------------------------------
-- Drop existing task.next triggers to recreate with project dependency checks
--------------------------------------------------------------------------------

DROP TRIGGER IF EXISTS trg_task_next_on_insert_todo;
DROP TRIGGER IF EXISTS trg_task_next_on_status_todo;
DROP TRIGGER IF EXISTS trg_task_next_on_dep_completed;
DROP TRIGGER IF EXISTS trg_task_next_on_unblocked;
DROP TRIGGER IF EXISTS trg_task_next_on_released;
DROP TRIGGER IF EXISTS trg_task_next_on_dep_removed;

--------------------------------------------------------------------------------
-- Recreate task.next triggers with project dependency checks
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
  AND NOT EXISTS (
    SELECT 1 FROM project_dependencies pd
    JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
    WHERE pd.project_id = NEW.project_id
      AND dep_p.status NOT IN ('done', 'archived')
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
  AND NOT EXISTS (
    SELECT 1 FROM project_dependencies pd
    JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
    WHERE pd.project_id = NEW.project_id
      AND dep_p.status NOT IN ('done', 'archived')
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
      'status', NEW.status, 'owner', NEW.owner
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;

-- 7c. Task dependency completed → unblocks dependents
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
    AND NOT EXISTS (
      SELECT 1 FROM project_dependencies pd
      JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
      WHERE pd.project_id = t.project_id
        AND dep_p.status NOT IN ('done', 'archived')
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
  AND NOT EXISTS (
    SELECT 1 FROM project_dependencies pd
    JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
    WHERE pd.project_id = NEW.project_id
      AND dep_p.status NOT IN ('done', 'archived')
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
  AND NOT EXISTS (
    SELECT 1 FROM project_dependencies pd
    JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
    WHERE pd.project_id = NEW.project_id
      AND dep_p.status NOT IN ('done', 'archived')
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
    AND NOT EXISTS (
      SELECT 1 FROM project_dependencies pd
      JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
      WHERE pd.project_id = t.project_id
        AND dep_p.status NOT IN ('done', 'archived')
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
-- New: Cascade trigger when a project dependency is completed (done/archived)
-- Emits task.next for newly-unblocked tasks in dependent projects
--------------------------------------------------------------------------------

CREATE TRIGGER trg_task_next_on_project_dep_completed
AFTER UPDATE ON projects
WHEN NEW.status IN ('done', 'archived') AND OLD.status NOT IN ('done', 'archived')
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
  -- Find tasks in projects that depend on the just-completed project
  JOIN project_dependencies pd ON pd.project_id = t.project_id
  WHERE pd.depends_on_project_id = NEW.id
    AND t.status = 'todo'
    AND t.blocked_reason IS NULL
    AND (t.claim_owner IS NULL OR t.claim_lease_expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
    -- No unmet task dependencies
    AND NOT EXISTS (
      SELECT 1 FROM task_dependencies td
      JOIN tasks dep ON dep.id = td.depends_on_task_id
      WHERE td.task_id = t.id
        AND dep.status != 'done'
    )
    -- No other unmet project dependencies
    AND NOT EXISTS (
      SELECT 1 FROM project_dependencies pd2
      JOIN projects dep_p ON dep_p.id = pd2.depends_on_project_id
      WHERE pd2.project_id = t.project_id
        AND pd2.depends_on_project_id != NEW.id
        AND dep_p.status NOT IN ('done', 'archived')
        AND EXISTS (
          SELECT 1 FROM tasks dep_t
          WHERE dep_t.project_id = pd2.depends_on_project_id
            AND dep_t.status != 'done'
        )
    )
    -- Task's own project must be active
    AND EXISTS (
      SELECT 1 FROM projects p WHERE p.id = t.project_id AND p.status = 'active'
    );
END;
