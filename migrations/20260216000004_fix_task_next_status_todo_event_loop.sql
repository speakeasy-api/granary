-- Fix: trg_task_next_on_status_todo fires on ANY update to a task with
-- status='todo', not just on transitions TO 'todo'. This causes an event loop
-- when workers update metadata (owner, worker_ids) on todo tasks.
--
-- Add OLD.status != 'todo' guard so it only fires on actual status transitions.

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
      'status', NEW.status, 'owner', NEW.owner
    ),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
  );
END;
