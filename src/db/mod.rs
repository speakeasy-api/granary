pub mod connection;

use sqlx::SqlitePool;

use crate::error::Result;
use crate::models::*;

/// Database operations for projects
pub mod projects {
    use super::*;

    pub async fn create(pool: &SqlitePool, project: &Project) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO projects (id, slug, name, description, owner, status, tags,
                default_session_policy, steering_refs, created_at, updated_at, version)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&project.id)
        .bind(&project.slug)
        .bind(&project.name)
        .bind(&project.description)
        .bind(&project.owner)
        .bind(&project.status)
        .bind(&project.tags)
        .bind(&project.default_session_policy)
        .bind(&project.steering_refs)
        .bind(&project.created_at)
        .bind(&project.updated_at)
        .bind(project.version)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(project)
    }

    pub async fn list(pool: &SqlitePool, include_archived: bool) -> Result<Vec<Project>> {
        let projects = if include_archived {
            sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at DESC")
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as::<_, Project>(
                "SELECT * FROM projects WHERE status = 'active' ORDER BY created_at DESC",
            )
            .fetch_all(pool)
            .await?
        };
        Ok(projects)
    }

    pub async fn update(pool: &SqlitePool, project: &Project) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE projects
            SET name = ?, description = ?, owner = ?, status = ?, tags = ?,
                default_session_policy = ?, steering_refs = ?, updated_at = ?, version = version + 1
            WHERE id = ? AND version = ?
            "#,
        )
        .bind(&project.name)
        .bind(&project.description)
        .bind(&project.owner)
        .bind(&project.status)
        .bind(&project.tags)
        .bind(&project.default_session_policy)
        .bind(&project.steering_refs)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&project.id)
        .bind(project.version)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn archive(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result =
            sqlx::query("UPDATE projects SET status = 'archived', updated_at = ? WHERE id = ?")
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(id)
                .execute(pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Database operations for tasks
pub mod tasks {
    use super::*;

    pub async fn create(pool: &SqlitePool, task: &Task) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tasks (id, project_id, task_number, parent_task_id, title, description,
                status, priority, owner, tags, blocked_reason, started_at, completed_at, due_at,
                claim_owner, claim_claimed_at, claim_lease_expires_at, pinned, focus_weight,
                created_at, updated_at, version)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&task.id)
        .bind(&task.project_id)
        .bind(task.task_number)
        .bind(&task.parent_task_id)
        .bind(&task.title)
        .bind(&task.description)
        .bind(&task.status)
        .bind(&task.priority)
        .bind(&task.owner)
        .bind(&task.tags)
        .bind(&task.blocked_reason)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .bind(&task.due_at)
        .bind(&task.claim_owner)
        .bind(&task.claim_claimed_at)
        .bind(&task.claim_lease_expires_at)
        .bind(task.pinned)
        .bind(task.focus_weight)
        .bind(&task.created_at)
        .bind(&task.updated_at)
        .bind(task.version)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Task>> {
        let task = sqlx::query_as::<_, Task>("SELECT * FROM tasks WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(task)
    }

    pub async fn list_by_project(pool: &SqlitePool, project_id: &str) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            "SELECT * FROM tasks WHERE project_id = ? ORDER BY task_number ASC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(tasks)
    }

    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>("SELECT * FROM tasks ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?;
        Ok(tasks)
    }

    pub async fn list_filtered(
        pool: &SqlitePool,
        status: Option<&str>,
        priority: Option<&str>,
        owner: Option<&str>,
    ) -> Result<Vec<Task>> {
        let mut query = String::from("SELECT * FROM tasks WHERE 1=1");

        if status.is_some() {
            query.push_str(" AND status = ?");
        }
        if priority.is_some() {
            query.push_str(" AND priority = ?");
        }
        if owner.is_some() {
            query.push_str(" AND owner = ?");
        }
        query.push_str(" ORDER BY created_at DESC");

        let mut q = sqlx::query_as::<_, Task>(&query);

        if let Some(s) = status {
            q = q.bind(s);
        }
        if let Some(p) = priority {
            q = q.bind(p);
        }
        if let Some(o) = owner {
            q = q.bind(o);
        }

        let tasks = q.fetch_all(pool).await?;
        Ok(tasks)
    }

    pub async fn list_subtasks(pool: &SqlitePool, parent_task_id: &str) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            "SELECT * FROM tasks WHERE parent_task_id = ? ORDER BY task_number ASC",
        )
        .bind(parent_task_id)
        .fetch_all(pool)
        .await?;
        Ok(tasks)
    }

    pub async fn update(pool: &SqlitePool, task: &Task) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET title = ?, description = ?, status = ?, priority = ?, owner = ?, tags = ?,
                blocked_reason = ?, started_at = ?, completed_at = ?, due_at = ?,
                claim_owner = ?, claim_claimed_at = ?, claim_lease_expires_at = ?,
                pinned = ?, focus_weight = ?, updated_at = ?, version = version + 1
            WHERE id = ? AND version = ?
            "#,
        )
        .bind(&task.title)
        .bind(&task.description)
        .bind(&task.status)
        .bind(&task.priority)
        .bind(&task.owner)
        .bind(&task.tags)
        .bind(&task.blocked_reason)
        .bind(&task.started_at)
        .bind(&task.completed_at)
        .bind(&task.due_at)
        .bind(&task.claim_owner)
        .bind(&task.claim_claimed_at)
        .bind(&task.claim_lease_expires_at)
        .bind(task.pinned)
        .bind(task.focus_weight)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&task.id)
        .bind(task.version)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get the next actionable task based on the spec algorithm
    pub async fn get_next(
        pool: &SqlitePool,
        project_ids: Option<&[String]>,
    ) -> Result<Option<Task>> {
        // Build query for next task:
        // 1. Status is todo or in_progress
        // 2. Not blocked
        // 3. All dependencies are done
        // 4. Order by priority, due_at, created_at

        let base_query = r#"
            SELECT t.*
            FROM tasks t
            WHERE t.status IN ('todo', 'in_progress')
              AND t.blocked_reason IS NULL
              AND NOT EXISTS (
                  SELECT 1 FROM task_dependencies td
                  JOIN tasks dep ON dep.id = td.depends_on_task_id
                  WHERE td.task_id = t.id
                    AND dep.status != 'done'
              )
        "#;

        let mut query = base_query.to_string();

        if project_ids.is_some() {
            query.push_str(" AND t.project_id IN (");
            // SQLite doesn't have array support, so we build the IN clause
            // This is safe since project_ids come from internal sources
            query.push_str("SELECT value FROM json_each(?)");
            query.push(')');
        }

        query.push_str(
            r#"
            ORDER BY
                CASE t.priority
                    WHEN 'P0' THEN 0
                    WHEN 'P1' THEN 1
                    WHEN 'P2' THEN 2
                    WHEN 'P3' THEN 3
                    WHEN 'P4' THEN 4
                END,
                t.due_at NULLS LAST,
                t.created_at
            LIMIT 1
            "#,
        );

        let task = if let Some(ids) = project_ids {
            let json_ids = serde_json::to_string(ids)?;
            sqlx::query_as::<_, Task>(&query)
                .bind(json_ids)
                .fetch_optional(pool)
                .await?
        } else {
            sqlx::query_as::<_, Task>(&query)
                .fetch_optional(pool)
                .await?
        };

        Ok(task)
    }
}

/// Database operations for task dependencies
pub mod dependencies {
    use super::*;

    pub async fn add(pool: &SqlitePool, task_id: &str, depends_on: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR IGNORE INTO task_dependencies (task_id, depends_on_task_id, created_at) VALUES (?, ?, ?)",
        )
        .bind(task_id)
        .bind(depends_on)
        .bind(&now)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn remove(pool: &SqlitePool, task_id: &str, depends_on: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM task_dependencies WHERE task_id = ? AND depends_on_task_id = ?",
        )
        .bind(task_id)
        .bind(depends_on)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list(pool: &SqlitePool, task_id: &str) -> Result<Vec<TaskDependency>> {
        let deps = sqlx::query_as::<_, TaskDependency>(
            "SELECT * FROM task_dependencies WHERE task_id = ?",
        )
        .bind(task_id)
        .fetch_all(pool)
        .await?;
        Ok(deps)
    }

    pub async fn list_dependents(pool: &SqlitePool, task_id: &str) -> Result<Vec<TaskDependency>> {
        let deps = sqlx::query_as::<_, TaskDependency>(
            "SELECT * FROM task_dependencies WHERE depends_on_task_id = ?",
        )
        .bind(task_id)
        .fetch_all(pool)
        .await?;
        Ok(deps)
    }

    /// Check if adding a dependency would create a cycle
    pub async fn would_create_cycle(
        pool: &SqlitePool,
        task_id: &str,
        depends_on: &str,
    ) -> Result<bool> {
        // Check if depends_on transitively depends on task_id
        // This would create a cycle
        let result = sqlx::query_scalar::<_, i64>(
            r#"
            WITH RECURSIVE dep_chain(id) AS (
                SELECT depends_on_task_id FROM task_dependencies WHERE task_id = ?
                UNION
                SELECT td.depends_on_task_id
                FROM task_dependencies td
                JOIN dep_chain dc ON td.task_id = dc.id
            )
            SELECT COUNT(*) FROM dep_chain WHERE id = ?
            "#,
        )
        .bind(depends_on)
        .bind(task_id)
        .fetch_one(pool)
        .await?;

        Ok(result > 0)
    }

    /// Get all unmet dependencies for a task (dependencies that aren't done)
    pub async fn get_unmet(pool: &SqlitePool, task_id: &str) -> Result<Vec<Task>> {
        let tasks = sqlx::query_as::<_, Task>(
            r#"
            SELECT t.* FROM tasks t
            JOIN task_dependencies td ON t.id = td.depends_on_task_id
            WHERE td.task_id = ? AND t.status != 'done'
            "#,
        )
        .bind(task_id)
        .fetch_all(pool)
        .await?;
        Ok(tasks)
    }
}

/// Database operations for comments
pub mod comments {
    use super::*;

    pub async fn create(pool: &SqlitePool, comment: &Comment) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO comments (id, parent_type, parent_id, comment_number, kind, content,
                author, meta, created_at, updated_at, version)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&comment.id)
        .bind(&comment.parent_type)
        .bind(&comment.parent_id)
        .bind(comment.comment_number)
        .bind(&comment.kind)
        .bind(&comment.content)
        .bind(&comment.author)
        .bind(&comment.meta)
        .bind(&comment.created_at)
        .bind(&comment.updated_at)
        .bind(comment.version)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Comment>> {
        let comment = sqlx::query_as::<_, Comment>("SELECT * FROM comments WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(comment)
    }

    pub async fn list_by_parent(pool: &SqlitePool, parent_id: &str) -> Result<Vec<Comment>> {
        let comments = sqlx::query_as::<_, Comment>(
            "SELECT * FROM comments WHERE parent_id = ? ORDER BY comment_number ASC",
        )
        .bind(parent_id)
        .fetch_all(pool)
        .await?;
        Ok(comments)
    }

    pub async fn list_by_kind(pool: &SqlitePool, kind: &str) -> Result<Vec<Comment>> {
        let comments = sqlx::query_as::<_, Comment>(
            "SELECT * FROM comments WHERE kind = ? ORDER BY created_at DESC",
        )
        .bind(kind)
        .fetch_all(pool)
        .await?;
        Ok(comments)
    }

    pub async fn update(pool: &SqlitePool, comment: &Comment) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE comments
            SET content = ?, kind = ?, meta = ?, updated_at = ?, version = version + 1
            WHERE id = ? AND version = ?
            "#,
        )
        .bind(&comment.content)
        .bind(&comment.kind)
        .bind(&comment.meta)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&comment.id)
        .bind(comment.version)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Database operations for sessions
pub mod sessions {
    use super::*;

    pub async fn create(pool: &SqlitePool, session: &Session) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sessions (id, name, owner, mode, focus_task_id, variables,
                created_at, updated_at, closed_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(&session.owner)
        .bind(&session.mode)
        .bind(&session.focus_task_id)
        .bind(&session.variables)
        .bind(&session.created_at)
        .bind(&session.updated_at)
        .bind(&session.closed_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(session)
    }

    pub async fn list(pool: &SqlitePool, include_closed: bool) -> Result<Vec<Session>> {
        let sessions = if include_closed {
            sqlx::query_as::<_, Session>("SELECT * FROM sessions ORDER BY created_at DESC")
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as::<_, Session>(
                "SELECT * FROM sessions WHERE closed_at IS NULL ORDER BY created_at DESC",
            )
            .fetch_all(pool)
            .await?
        };
        Ok(sessions)
    }

    pub async fn update(pool: &SqlitePool, session: &Session) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE sessions
            SET name = ?, owner = ?, mode = ?, focus_task_id = ?, variables = ?,
                updated_at = ?, closed_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&session.name)
        .bind(&session.owner)
        .bind(&session.mode)
        .bind(&session.focus_task_id)
        .bind(&session.variables)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&session.closed_at)
        .bind(&session.id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn close(pool: &SqlitePool, id: &str) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE sessions SET closed_at = ?, updated_at = ? WHERE id = ? AND closed_at IS NULL",
        )
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Session scope operations
    pub async fn add_scope(
        pool: &SqlitePool,
        session_id: &str,
        item_type: &str,
        item_id: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR IGNORE INTO session_scope (session_id, item_type, item_id, pinned_at) VALUES (?, ?, ?, ?)",
        )
        .bind(session_id)
        .bind(item_type)
        .bind(item_id)
        .bind(&now)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn remove_scope(
        pool: &SqlitePool,
        session_id: &str,
        item_type: &str,
        item_id: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM session_scope WHERE session_id = ? AND item_type = ? AND item_id = ?",
        )
        .bind(session_id)
        .bind(item_type)
        .bind(item_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn get_scope(pool: &SqlitePool, session_id: &str) -> Result<Vec<SessionScope>> {
        let scope = sqlx::query_as::<_, SessionScope>(
            "SELECT * FROM session_scope WHERE session_id = ? ORDER BY pinned_at DESC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;
        Ok(scope)
    }

    pub async fn get_scope_by_type(
        pool: &SqlitePool,
        session_id: &str,
        item_type: &str,
    ) -> Result<Vec<SessionScope>> {
        let scope = sqlx::query_as::<_, SessionScope>(
            "SELECT * FROM session_scope WHERE session_id = ? AND item_type = ?",
        )
        .bind(session_id)
        .bind(item_type)
        .fetch_all(pool)
        .await?;
        Ok(scope)
    }
}

/// Database operations for events
pub mod events {
    use super::*;

    pub async fn create(pool: &SqlitePool, event: &CreateEvent) -> Result<i64> {
        let now = chrono::Utc::now().to_rfc3339();
        let payload = serde_json::to_string(&event.payload)?;

        let id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO events (event_type, entity_type, entity_id, actor, session_id, payload, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            RETURNING id
            "#,
        )
        .bind(event.event_type.as_str())
        .bind(event.entity_type.as_str())
        .bind(&event.entity_id)
        .bind(&event.actor)
        .bind(&event.session_id)
        .bind(&payload)
        .bind(&now)
        .fetch_one(pool)
        .await?;

        Ok(id)
    }

    pub async fn list_by_entity(
        pool: &SqlitePool,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(
            "SELECT * FROM events WHERE entity_type = ? AND entity_id = ? ORDER BY created_at DESC",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(pool)
        .await?;
        Ok(events)
    }

    pub async fn list_by_session(pool: &SqlitePool, session_id: &str) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(
            "SELECT * FROM events WHERE session_id = ? ORDER BY created_at DESC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;
        Ok(events)
    }

    pub async fn list_since(pool: &SqlitePool, since: &str) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(
            "SELECT * FROM events WHERE created_at > ? ORDER BY created_at ASC",
        )
        .bind(since)
        .fetch_all(pool)
        .await?;
        Ok(events)
    }
}

/// Database operations for artifacts
pub mod artifacts {
    use super::*;

    pub async fn create(pool: &SqlitePool, artifact: &Artifact) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO artifacts (id, parent_type, parent_id, artifact_number, artifact_type,
                path_or_url, description, meta, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&artifact.id)
        .bind(&artifact.parent_type)
        .bind(&artifact.parent_id)
        .bind(artifact.artifact_number)
        .bind(&artifact.artifact_type)
        .bind(&artifact.path_or_url)
        .bind(&artifact.description)
        .bind(&artifact.meta)
        .bind(&artifact.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Artifact>> {
        let artifact = sqlx::query_as::<_, Artifact>("SELECT * FROM artifacts WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(artifact)
    }

    pub async fn list_by_parent(pool: &SqlitePool, parent_id: &str) -> Result<Vec<Artifact>> {
        let artifacts = sqlx::query_as::<_, Artifact>(
            "SELECT * FROM artifacts WHERE parent_id = ? ORDER BY artifact_number ASC",
        )
        .bind(parent_id)
        .fetch_all(pool)
        .await?;
        Ok(artifacts)
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM artifacts WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Database operations for checkpoints
pub mod checkpoints {
    use super::*;

    pub async fn create(pool: &SqlitePool, checkpoint: &Checkpoint) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO checkpoints (id, session_id, name, snapshot, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&checkpoint.id)
        .bind(&checkpoint.session_id)
        .bind(&checkpoint.name)
        .bind(&checkpoint.snapshot)
        .bind(&checkpoint.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Checkpoint>> {
        let checkpoint = sqlx::query_as::<_, Checkpoint>("SELECT * FROM checkpoints WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(checkpoint)
    }

    pub async fn get_by_name(
        pool: &SqlitePool,
        session_id: &str,
        name: &str,
    ) -> Result<Option<Checkpoint>> {
        let checkpoint = sqlx::query_as::<_, Checkpoint>(
            "SELECT * FROM checkpoints WHERE session_id = ? AND name = ?",
        )
        .bind(session_id)
        .bind(name)
        .fetch_optional(pool)
        .await?;
        Ok(checkpoint)
    }

    pub async fn list_by_session(pool: &SqlitePool, session_id: &str) -> Result<Vec<Checkpoint>> {
        let checkpoints = sqlx::query_as::<_, Checkpoint>(
            "SELECT * FROM checkpoints WHERE session_id = ? ORDER BY created_at DESC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;
        Ok(checkpoints)
    }
}

/// Database operations for counters (monotonic ID generation)
pub mod counters {
    use super::*;

    pub async fn next(pool: &SqlitePool, scope: &str) -> Result<i64> {
        let value = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO counters (scope, value) VALUES (?, 1)
            ON CONFLICT(scope) DO UPDATE SET value = value + 1
            RETURNING value
            "#,
        )
        .bind(scope)
        .fetch_one(pool)
        .await?;
        Ok(value)
    }

    pub async fn current(pool: &SqlitePool, scope: &str) -> Result<i64> {
        let value = sqlx::query_scalar::<_, i64>("SELECT value FROM counters WHERE scope = ?")
            .bind(scope)
            .fetch_optional(pool)
            .await?
            .unwrap_or(0);
        Ok(value)
    }
}

/// Database operations for config
pub mod config {
    use super::*;

    pub async fn get(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
        let value = sqlx::query_scalar::<_, String>("SELECT value FROM config WHERE key = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
        Ok(value)
    }

    pub async fn set(pool: &SqlitePool, key: &str, value: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO config (key, value, updated_at) VALUES (?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = ?
            "#,
        )
        .bind(key)
        .bind(value)
        .bind(&now)
        .bind(value)
        .bind(&now)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, key: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM config WHERE key = ?")
            .bind(key)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list(pool: &SqlitePool) -> Result<Vec<(String, String)>> {
        let rows =
            sqlx::query_as::<_, (String, String)>("SELECT key, value FROM config ORDER BY key")
                .fetch_all(pool)
                .await?;
        Ok(rows)
    }
}
