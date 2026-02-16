-- FTS5 full-text search index: shared virtual table across projects, tasks,
-- and initiatives with sync triggers and backfill.

--------------------------------------------------------------------------------
-- 1. FTS5 virtual table
--------------------------------------------------------------------------------

CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
    entity_type,
    entity_id,
    title,
    body,
    tags,
    tokenize='unicode61 remove_diacritics 2'
);

--------------------------------------------------------------------------------
-- 2. Project triggers
--------------------------------------------------------------------------------

CREATE TRIGGER IF NOT EXISTS search_index_projects_insert AFTER INSERT ON projects BEGIN
    INSERT INTO search_index(entity_type, entity_id, title, body, tags)
    VALUES ('project', NEW.id, NEW.name, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS search_index_projects_update AFTER UPDATE ON projects BEGIN
    DELETE FROM search_index WHERE entity_type = 'project' AND entity_id = OLD.id;
    INSERT INTO search_index(entity_type, entity_id, title, body, tags)
    VALUES ('project', NEW.id, NEW.name, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS search_index_projects_delete AFTER DELETE ON projects BEGIN
    DELETE FROM search_index WHERE entity_type = 'project' AND entity_id = OLD.id;
END;

--------------------------------------------------------------------------------
-- 3. Task triggers
--------------------------------------------------------------------------------

CREATE TRIGGER IF NOT EXISTS search_index_tasks_insert AFTER INSERT ON tasks BEGIN
    INSERT INTO search_index(entity_type, entity_id, title, body, tags)
    VALUES ('task', NEW.id, NEW.title, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS search_index_tasks_update AFTER UPDATE ON tasks BEGIN
    DELETE FROM search_index WHERE entity_type = 'task' AND entity_id = OLD.id;
    INSERT INTO search_index(entity_type, entity_id, title, body, tags)
    VALUES ('task', NEW.id, NEW.title, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS search_index_tasks_delete AFTER DELETE ON tasks BEGIN
    DELETE FROM search_index WHERE entity_type = 'task' AND entity_id = OLD.id;
END;

--------------------------------------------------------------------------------
-- 4. Initiative triggers
--------------------------------------------------------------------------------

CREATE TRIGGER IF NOT EXISTS search_index_initiatives_insert AFTER INSERT ON initiatives BEGIN
    INSERT INTO search_index(entity_type, entity_id, title, body, tags)
    VALUES ('initiative', NEW.id, NEW.name, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS search_index_initiatives_update AFTER UPDATE ON initiatives BEGIN
    DELETE FROM search_index WHERE entity_type = 'initiative' AND entity_id = OLD.id;
    INSERT INTO search_index(entity_type, entity_id, title, body, tags)
    VALUES ('initiative', NEW.id, NEW.name, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS search_index_initiatives_delete AFTER DELETE ON initiatives BEGIN
    DELETE FROM search_index WHERE entity_type = 'initiative' AND entity_id = OLD.id;
END;

--------------------------------------------------------------------------------
-- 5. Backfill existing rows
--------------------------------------------------------------------------------

INSERT INTO search_index(entity_type, entity_id, title, body, tags)
SELECT 'project', id, name, description, tags FROM projects;

INSERT INTO search_index(entity_type, entity_id, title, body, tags)
SELECT 'task', id, title, description, tags FROM tasks;

INSERT INTO search_index(entity_type, entity_id, title, body, tags)
SELECT 'initiative', id, name, description, tags FROM initiatives;
