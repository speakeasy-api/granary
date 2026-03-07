-- Review mode triggers
-- Adds task.review and project.review event triggers,
-- updates trg_project_auto_complete to be config-aware,
-- and updates trg_project_unarchived to include in_review -> active transitions.

-- 1. Task review trigger: emit task.review when task transitions to in_review
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

-- 2. Project review trigger: emit project.review when project transitions to in_review
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

-- 3. Update project auto-complete trigger to be config-aware
-- When workflow.review_mode = 'project', set project to in_review instead of completed
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

-- 4. Update project unarchived trigger to include in_review -> active
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
