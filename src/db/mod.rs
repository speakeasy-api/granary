pub mod connection;

use sqlx::SqlitePool;

use crate::error::Result;
use crate::models::*;

/// Database operations for projects
pub mod projects {
    use granary_types::Project;

    use super::*;

    pub async fn create(pool: &SqlitePool, project: &Project) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO projects (id, slug, name, description, owner, status, tags,
                default_session_policy, steering_refs, created_at, updated_at, version, last_edited_by)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(&project.last_edited_by)
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
                default_session_policy = ?, steering_refs = ?, updated_at = ?, version = version + 1,
                last_edited_by = ?
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
        .bind(&project.last_edited_by)
        .bind(&project.id)
        .bind(project.version)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn archive(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE projects SET status = 'archived', updated_at = ?, last_edited_by = NULL WHERE id = ?",
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn complete(pool: &SqlitePool, id: &str, complete_tasks: bool) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut tx = pool.begin().await?;

        if complete_tasks {
            sqlx::query(
                "UPDATE tasks SET status = 'done', completed_at = ?, updated_at = ?, last_edited_by = NULL, version = version + 1 WHERE project_id = ? AND status != 'done'",
            )
            .bind(&now)
            .bind(&now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }

        let result = sqlx::query(
            "UPDATE projects SET status = 'completed', updated_at = ?, last_edited_by = NULL WHERE id = ?",
        )
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn unarchive(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE projects SET status = 'active', updated_at = ?, last_edited_by = NULL WHERE id = ?",
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Database operations for initiatives
pub mod initiatives {
    use super::*;
    use crate::models::ids;
    use crate::models::{CreateInitiative, Initiative, UpdateInitiative};

    pub async fn create(pool: &SqlitePool, input: &CreateInitiative) -> Result<Initiative> {
        let id = ids::generate_initiative_id(&input.name);
        let slug = ids::normalize_slug(&input.name);
        let now = chrono::Utc::now().to_rfc3339();
        let tags_json = if input.tags.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&input.tags)?)
        };

        sqlx::query(
            r#"
            INSERT INTO initiatives (id, slug, name, description, owner, status, tags, created_at, updated_at, version)
            VALUES (?, ?, ?, ?, ?, 'active', ?, ?, ?, 1)
            "#,
        )
        .bind(&id)
        .bind(&slug)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.owner)
        .bind(&tags_json)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        // Fetch the created initiative
        get(pool, &id).await?.ok_or_else(|| {
            crate::error::GranaryError::Conflict(
                "Failed to create initiative: could not retrieve after insert".to_string(),
            )
        })
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Initiative>> {
        let initiative = sqlx::query_as::<_, Initiative>("SELECT * FROM initiatives WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(initiative)
    }

    pub async fn list(pool: &SqlitePool, include_archived: bool) -> Result<Vec<Initiative>> {
        let initiatives = if include_archived {
            sqlx::query_as::<_, Initiative>("SELECT * FROM initiatives ORDER BY created_at DESC")
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as::<_, Initiative>(
                "SELECT * FROM initiatives WHERE status = 'active' ORDER BY created_at DESC",
            )
            .fetch_all(pool)
            .await?
        };
        Ok(initiatives)
    }

    pub async fn update(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateInitiative,
        expected_version: i64,
    ) -> Result<Option<Initiative>> {
        // Build dynamic update - we need to update only the fields that are Some
        // Using optimistic locking with version check
        let now = chrono::Utc::now().to_rfc3339();

        // First get the current initiative to merge updates
        let current = match get(pool, id).await? {
            Some(i) => i,
            None => return Ok(None),
        };

        // Check version for optimistic locking
        if current.version != expected_version {
            return Err(crate::error::GranaryError::VersionMismatch {
                expected: expected_version,
                found: current.version,
            });
        }

        // Merge updates with current values
        let name = update.name.as_ref().unwrap_or(&current.name);
        let description = update.description.clone().or(current.description);
        let owner = update.owner.clone().or(current.owner);
        let status = update
            .status
            .as_ref()
            .map(|s| s.as_str().to_string())
            .unwrap_or(current.status);
        let tags_json = update
            .tags
            .as_ref()
            .map(|t| serde_json::to_string(t).ok())
            .unwrap_or(current.tags);

        let result = sqlx::query(
            r#"
            UPDATE initiatives
            SET name = ?, description = ?, owner = ?, status = ?, tags = ?,
                updated_at = ?, version = version + 1
            WHERE id = ? AND version = ?
            "#,
        )
        .bind(name)
        .bind(&description)
        .bind(&owner)
        .bind(&status)
        .bind(&tags_json)
        .bind(&now)
        .bind(id)
        .bind(expected_version)
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            // Version conflict (race condition)
            return Err(crate::error::GranaryError::VersionMismatch {
                expected: expected_version,
                found: current.version,
            });
        }

        get(pool, id).await
    }

    pub async fn archive(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result =
            sqlx::query("UPDATE initiatives SET status = 'archived', updated_at = ? WHERE id = ?")
                .bind(chrono::Utc::now().to_rfc3339())
                .bind(id)
                .execute(pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM initiatives WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Database operations for initiative-project relationships
pub mod initiative_projects {
    use granary_types::{Project, ProjectDependency};

    use super::*;
    use crate::models::Initiative;

    /// Add a project to an initiative
    pub async fn add(pool: &SqlitePool, initiative_id: &str, project_id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR IGNORE INTO initiative_projects (initiative_id, project_id, added_at) VALUES (?, ?, ?)",
        )
        .bind(initiative_id)
        .bind(project_id)
        .bind(&now)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove a project from an initiative
    pub async fn remove(pool: &SqlitePool, initiative_id: &str, project_id: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM initiative_projects WHERE initiative_id = ? AND project_id = ?",
        )
        .bind(initiative_id)
        .bind(project_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all projects in an initiative
    pub async fn list_projects(pool: &SqlitePool, initiative_id: &str) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.* FROM projects p
            JOIN initiative_projects ip ON p.id = ip.project_id
            WHERE ip.initiative_id = ?
            ORDER BY p.name ASC
            "#,
        )
        .bind(initiative_id)
        .fetch_all(pool)
        .await?;
        Ok(projects)
    }

    /// List all initiatives that contain a project
    pub async fn list_initiatives(pool: &SqlitePool, project_id: &str) -> Result<Vec<Initiative>> {
        let initiatives = sqlx::query_as::<_, Initiative>(
            r#"
            SELECT i.* FROM initiatives i
            JOIN initiative_projects ip ON i.id = ip.initiative_id
            WHERE ip.project_id = ?
            ORDER BY i.name ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(initiatives)
    }

    /// Get all project dependencies within an initiative
    /// Returns dependencies where both the source and target projects are in the initiative
    pub async fn list_internal_dependencies(
        pool: &SqlitePool,
        initiative_id: &str,
    ) -> Result<Vec<ProjectDependency>> {
        let dependencies = sqlx::query_as::<_, ProjectDependency>(
            r#"
            SELECT pd.* FROM project_dependencies pd
            JOIN initiative_projects ip1 ON pd.project_id = ip1.project_id
            JOIN initiative_projects ip2 ON pd.depends_on_project_id = ip2.project_id
            WHERE ip1.initiative_id = ? AND ip2.initiative_id = ?
            ORDER BY pd.project_id, pd.depends_on_project_id
            "#,
        )
        .bind(initiative_id)
        .bind(initiative_id)
        .fetch_all(pool)
        .await?;
        Ok(dependencies)
    }
}

/// Database operations for tasks
pub mod tasks {
    use granary_types::Task;

    use super::*;

    pub async fn create(pool: &SqlitePool, task: &Task) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tasks (id, project_id, task_number, parent_task_id, title, description,
                status, priority, owner, tags, worker_ids, run_ids, blocked_reason,
                started_at, completed_at, due_at,
                claim_owner, claim_claimed_at, claim_lease_expires_at, pinned, focus_weight,
                created_at, updated_at, version, last_edited_by)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(&task.worker_ids)
        .bind(&task.run_ids)
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
        .bind(&task.last_edited_by)
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
                worker_ids = ?, run_ids = ?,
                blocked_reason = ?, started_at = ?, completed_at = ?, due_at = ?,
                claim_owner = ?, claim_claimed_at = ?, claim_lease_expires_at = ?,
                pinned = ?, focus_weight = ?, updated_at = ?, version = version + 1,
                last_edited_by = ?
            WHERE id = ? AND version = ?
            "#,
        )
        .bind(&task.title)
        .bind(&task.description)
        .bind(&task.status)
        .bind(&task.priority)
        .bind(&task.owner)
        .bind(&task.tags)
        .bind(&task.worker_ids)
        .bind(&task.run_ids)
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
        .bind(&task.last_edited_by)
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
        // 1. Status is todo
        // 2. Not blocked
        // 3. All dependencies are done
        // 4. Project is active (not archived)
        // 5. Order by priority, due_at, created_at

        let base_query = r#"
            SELECT t.*
            FROM tasks t
            JOIN projects p ON p.id = t.project_id
            WHERE t.status IS 'todo'
              AND t.blocked_reason IS NULL
              AND p.status = 'active'
              AND NOT EXISTS (
                  SELECT 1 FROM task_dependencies td
                  JOIN tasks dep ON dep.id = td.depends_on_task_id
                  WHERE td.task_id = t.id
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

    /// Get all actionable tasks (next tasks without limit)
    /// Draft tasks are excluded from actionable tasks
    /// Archived projects are excluded
    pub async fn get_all_next(
        pool: &SqlitePool,
        project_ids: Option<&[String]>,
    ) -> Result<Vec<Task>> {
        // Same query as get_next but without LIMIT 1
        // Draft tasks are excluded (only 'todo' is actionable)
        // Archived projects are excluded
        let base_query = r#"
            SELECT t.*
            FROM tasks t
            JOIN projects p ON p.id = t.project_id
            WHERE t.status IS 'todo'
              AND t.blocked_reason IS NULL
              AND p.status = 'active'
              AND NOT EXISTS (
                  SELECT 1 FROM task_dependencies td
                  JOIN tasks dep ON dep.id = td.depends_on_task_id
                  WHERE td.task_id = t.id
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
        "#;

        let mut query = base_query.to_string();

        if project_ids.is_some() {
            query.push_str(" AND t.project_id IN (");
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
            "#,
        );

        let tasks = if let Some(ids) = project_ids {
            let json_ids = serde_json::to_string(ids)?;
            sqlx::query_as::<_, Task>(&query)
                .bind(json_ids)
                .fetch_all(pool)
                .await?
        } else {
            sqlx::query_as::<_, Task>(&query).fetch_all(pool).await?
        };

        Ok(tasks)
    }

    /// Update all draft tasks in a project to todo status
    /// Returns the number of tasks updated
    pub async fn set_draft_tasks_to_todo(pool: &SqlitePool, project_id: &str) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET status = 'todo', updated_at = ?, last_edited_by = NULL, version = version + 1
            WHERE project_id = ? AND status = 'draft'
            "#,
        )
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(project_id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

/// Database operations for task dependencies
pub mod dependencies {
    use granary_types::{Task, TaskDependency};

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

/// Database operations for project dependencies
pub mod project_dependencies {
    use granary_types::{Project, ProjectDependency};

    use super::*;
    use crate::error::GranaryError;

    /// Add a dependency from one project to another
    pub async fn add(pool: &SqlitePool, project_id: &str, depends_on: &str) -> Result<()> {
        // Check for cycle first
        if would_create_cycle(pool, project_id, depends_on).await? {
            return Err(GranaryError::DependencyCycle(format!(
                "Adding dependency {} -> {} would create a cycle",
                project_id, depends_on
            )));
        }

        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR IGNORE INTO project_dependencies (project_id, depends_on_project_id, created_at) VALUES (?, ?, ?)",
        )
        .bind(project_id)
        .bind(depends_on)
        .bind(&now)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Remove a dependency between projects
    pub async fn remove(pool: &SqlitePool, project_id: &str, depends_on: &str) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM project_dependencies WHERE project_id = ? AND depends_on_project_id = ?",
        )
        .bind(project_id)
        .bind(depends_on)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all projects that this project depends on
    pub async fn list(pool: &SqlitePool, project_id: &str) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.* FROM projects p
            JOIN project_dependencies pd ON p.id = pd.depends_on_project_id
            WHERE pd.project_id = ?
            ORDER BY p.name ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(projects)
    }

    /// List all projects that depend on this project
    pub async fn list_dependents(pool: &SqlitePool, project_id: &str) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT p.* FROM projects p
            JOIN project_dependencies pd ON p.id = pd.project_id
            WHERE pd.depends_on_project_id = ?
            ORDER BY p.name ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(projects)
    }

    /// Check if adding a dependency would create a cycle
    pub async fn would_create_cycle(
        pool: &SqlitePool,
        project_id: &str,
        depends_on: &str,
    ) -> Result<bool> {
        // If project_id == depends_on, it's a self-loop (already prevented by CHECK constraint)
        if project_id == depends_on {
            return Ok(true);
        }

        // Check if depends_on transitively depends on project_id
        // This would create a cycle: project_id -> depends_on -> ... -> project_id
        let result = sqlx::query_scalar::<_, i64>(
            r#"
            WITH RECURSIVE dep_chain(id) AS (
                SELECT depends_on_project_id FROM project_dependencies WHERE project_id = ?
                UNION
                SELECT pd.depends_on_project_id
                FROM project_dependencies pd
                JOIN dep_chain dc ON pd.project_id = dc.id
            )
            SELECT COUNT(*) FROM dep_chain WHERE id = ?
            "#,
        )
        .bind(depends_on)
        .bind(project_id)
        .fetch_one(pool)
        .await?;

        Ok(result > 0)
    }

    /// Get all unmet dependencies for a project
    /// A project dependency is "unmet" if the dependent project has any incomplete tasks
    pub async fn get_unmet(pool: &SqlitePool, project_id: &str) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(
            r#"
            SELECT DISTINCT p.* FROM projects p
            JOIN project_dependencies pd ON p.id = pd.depends_on_project_id
            WHERE pd.project_id = ?
            AND EXISTS (
                SELECT 1 FROM tasks t
                WHERE t.project_id = p.id AND t.status != 'done'
            )
            ORDER BY p.name ASC
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(projects)
    }

    /// Get raw dependency records for a project
    pub async fn list_raw(pool: &SqlitePool, project_id: &str) -> Result<Vec<ProjectDependency>> {
        let deps = sqlx::query_as::<_, ProjectDependency>(
            "SELECT * FROM project_dependencies WHERE project_id = ?",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(deps)
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
                created_at, updated_at, closed_at, last_edited_by)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(&session.last_edited_by)
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
                updated_at = ?, closed_at = ?, last_edited_by = ?
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
        .bind(&session.last_edited_by)
        .bind(&session.id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn close(pool: &SqlitePool, id: &str) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE sessions SET closed_at = ?, updated_at = ?, last_edited_by = NULL WHERE id = ? AND closed_at IS NULL",
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

    pub async fn get_by_id(pool: &SqlitePool, event_id: i64) -> Result<Option<Event>> {
        let event = sqlx::query_as::<_, Event>("SELECT * FROM events WHERE id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await?;
        Ok(event)
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

    /// List events since a specific event ID (exclusive)
    ///
    /// This is used for cursor-based event polling. Returns events with IDs
    /// greater than the specified ID, ordered by ID ascending.
    pub async fn list_since_id(pool: &SqlitePool, since_id: i64) -> Result<Vec<Event>> {
        let events =
            sqlx::query_as::<_, Event>("SELECT * FROM events WHERE id > ? ORDER BY id ASC")
                .bind(since_id)
                .fetch_all(pool)
                .await?;
        Ok(events)
    }

    /// List events since a specific event ID, filtered by event type
    ///
    /// This is more efficient than fetching all events and filtering in memory.
    pub async fn list_since_id_by_type(
        pool: &SqlitePool,
        since_id: i64,
        event_type: &str,
    ) -> Result<Vec<Event>> {
        let events = sqlx::query_as::<_, Event>(
            "SELECT * FROM events WHERE id > ? AND event_type = ? ORDER BY id ASC",
        )
        .bind(since_id)
        .bind(event_type)
        .fetch_all(pool)
        .await?;
        Ok(events)
    }

    /// List events with optional filters for CLI display
    pub async fn list_filtered(
        pool: &SqlitePool,
        event_type: Option<&str>,
        entity_type: Option<&str>,
        since: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Event>> {
        let mut query = String::from("SELECT * FROM events WHERE 1=1");
        if event_type.is_some() {
            query.push_str(" AND event_type = ?");
        }
        if entity_type.is_some() {
            query.push_str(" AND entity_type = ?");
        }
        if since.is_some() {
            query.push_str(" AND created_at >= ?");
        }
        query.push_str(" ORDER BY id DESC LIMIT ?");

        let mut q = sqlx::query_as::<_, Event>(&query);
        if let Some(et) = event_type {
            q = q.bind(et);
        }
        if let Some(ent) = entity_type {
            q = q.bind(ent);
        }
        if let Some(s) = since {
            q = q.bind(s);
        }
        q = q.bind(limit);

        let events = q.fetch_all(pool).await?;
        Ok(events)
    }

    /// Delete events created before a given timestamp
    pub async fn delete_before(pool: &SqlitePool, before_timestamp: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM events WHERE created_at < ?")
            .bind(before_timestamp)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Get the maximum event ID (useful for drain operations)
    pub async fn max_id_before(pool: &SqlitePool, before_timestamp: &str) -> Result<i64> {
        let id = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(id), 0) FROM events WHERE created_at < ?",
        )
        .bind(before_timestamp)
        .fetch_one(pool)
        .await?;
        Ok(id)
    }
}

/// Database operations for event consumers
pub mod event_consumers {
    use super::*;
    use crate::models::EventConsumer;

    pub async fn register(
        pool: &SqlitePool,
        id: &str,
        event_type: &str,
        started_at: &str,
        last_seen_id: i64,
    ) -> Result<EventConsumer> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO event_consumers (id, event_type, started_at, last_seen_id, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET updated_at = ?
            "#,
        )
        .bind(id)
        .bind(event_type)
        .bind(started_at)
        .bind(last_seen_id)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        get(pool, id).await?.ok_or_else(|| {
            crate::error::GranaryError::Conflict("Failed to register event consumer".to_string())
        })
    }

    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<EventConsumer>> {
        let consumer =
            sqlx::query_as::<_, EventConsumer>("SELECT * FROM event_consumers WHERE id = ?")
                .bind(id)
                .fetch_optional(pool)
                .await?;
        Ok(consumer)
    }

    pub async fn update_last_seen(pool: &SqlitePool, id: &str, last_seen_id: i64) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE event_consumers SET last_seen_id = ?, updated_at = ? WHERE id = ?")
            .bind(last_seen_id)
            .bind(&now)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM event_consumers WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn list(pool: &SqlitePool) -> Result<Vec<EventConsumer>> {
        let consumers = sqlx::query_as::<_, EventConsumer>(
            "SELECT * FROM event_consumers ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(consumers)
    }
}

/// Database operations for event consumptions (claim-based)
pub mod event_consumptions {
    use super::*;
    use crate::models::Event;

    /// Atomically claim the next unclaimed event for a consumer.
    /// Returns None if no matching events are available.
    ///
    /// The `filter_clauses` parameter contains tuples of (sql_fragment, json_path, value)
    /// from Filter::to_sql(). These are appended as AND conditions.
    pub async fn try_claim_next(
        pool: &SqlitePool,
        consumer_id: &str,
        event_type: &str,
        last_seen_id: i64,
        started_at: &str,
        filter_clauses: &[(String, String, String)],
    ) -> Result<Option<Event>> {
        let now = chrono::Utc::now().to_rfc3339();

        // Step 1: Find the next unclaimed event
        let mut select_query = String::from(
            r#"
            SELECT e.* FROM events e
            WHERE e.id > ?
              AND e.event_type = ?
              AND e.created_at >= ?
              AND NOT EXISTS (
                SELECT 1 FROM event_consumptions ec
                WHERE ec.consumer_id = ? AND ec.event_id = e.id
              )
            "#,
        );

        // Append dynamic filter clauses
        for (sql_frag, _, _) in filter_clauses {
            select_query.push_str("  AND ");
            select_query.push_str(sql_frag);
            select_query.push('\n');
        }

        select_query.push_str("ORDER BY e.id ASC LIMIT 1");

        let mut q = sqlx::query_as::<_, Event>(&select_query)
            .bind(last_seen_id)
            .bind(event_type)
            .bind(started_at)
            .bind(consumer_id);

        // Bind filter parameters (each filter has json_path and value)
        for (_, json_path, value) in filter_clauses {
            q = q.bind(json_path).bind(value);
        }

        let event = match q.fetch_optional(pool).await? {
            Some(e) => e,
            None => return Ok(None),
        };

        // Step 2: Try to claim it (INSERT OR IGNORE to handle races)
        let claimed = sqlx::query(
            "INSERT OR IGNORE INTO event_consumptions (consumer_id, event_id, consumed_at) VALUES (?, ?, ?)",
        )
        .bind(consumer_id)
        .bind(event.id)
        .bind(&now)
        .execute(pool)
        .await?;

        if claimed.rows_affected() == 0 {
            // Another consumer claimed it first; skip
            return Ok(None);
        }

        Ok(Some(event))
    }

    /// Delete consumption records for events before a given event ID
    pub async fn delete_before(pool: &SqlitePool, before_event_id: i64) -> Result<u64> {
        let result = sqlx::query("DELETE FROM event_consumptions WHERE event_id < ?")
            .bind(before_event_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
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

/// Database operations for steering files
pub mod steering {
    use super::*;

    #[derive(Debug, Clone, serde::Serialize)]
    pub struct SteeringFile {
        pub id: i64,
        pub path: String,
        pub mode: String,
        pub scope_type: Option<String>,
        pub scope_id: Option<String>,
        pub created_at: String,
    }

    impl SteeringFile {
        /// Format the scope for display
        pub fn scope_display(&self) -> String {
            match (&self.scope_type, &self.scope_id) {
                (None, _) => "global".to_string(),
                (Some(t), Some(id)) => format!("{}: {}", t, id),
                (Some(t), None) => t.clone(),
            }
        }
    }

    /// List all steering files
    pub async fn list(pool: &SqlitePool) -> Result<Vec<SteeringFile>> {
        let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>, String)>(
            "SELECT id, path, mode, scope_type, scope_id, created_at FROM steering ORDER BY scope_type NULLS FIRST, path",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, path, mode, scope_type, scope_id, created_at)| SteeringFile {
                    id,
                    path,
                    mode,
                    scope_type,
                    scope_id,
                    created_at,
                },
            )
            .collect())
    }

    /// List global (unscoped) steering files
    pub async fn list_global(pool: &SqlitePool) -> Result<Vec<SteeringFile>> {
        let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>, String)>(
            "SELECT id, path, mode, scope_type, scope_id, created_at FROM steering WHERE scope_type IS NULL ORDER BY path",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, path, mode, scope_type, scope_id, created_at)| SteeringFile {
                    id,
                    path,
                    mode,
                    scope_type,
                    scope_id,
                    created_at,
                },
            )
            .collect())
    }

    /// List steering files attached to a specific scope
    pub async fn list_by_scope(
        pool: &SqlitePool,
        scope_type: &str,
        scope_id: &str,
    ) -> Result<Vec<SteeringFile>> {
        let rows = sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>, String)>(
            "SELECT id, path, mode, scope_type, scope_id, created_at FROM steering WHERE scope_type = ? AND scope_id = ? ORDER BY path",
        )
        .bind(scope_type)
        .bind(scope_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, path, mode, scope_type, scope_id, created_at)| SteeringFile {
                    id,
                    path,
                    mode,
                    scope_type,
                    scope_id,
                    created_at,
                },
            )
            .collect())
    }

    /// List steering files for multiple scopes (e.g., all projects in session)
    pub async fn list_by_scope_ids(
        pool: &SqlitePool,
        scope_type: &str,
        scope_ids: &[String],
    ) -> Result<Vec<SteeringFile>> {
        if scope_ids.is_empty() {
            return Ok(Vec::new());
        }

        let json_ids = serde_json::to_string(scope_ids)?;
        let rows =
            sqlx::query_as::<_, (i64, String, String, Option<String>, Option<String>, String)>(
                "SELECT id, path, mode, scope_type, scope_id, created_at FROM steering
             WHERE scope_type = ? AND scope_id IN (SELECT value FROM json_each(?))
             ORDER BY path",
            )
            .bind(scope_type)
            .bind(json_ids)
            .fetch_all(pool)
            .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, path, mode, scope_type, scope_id, created_at)| SteeringFile {
                    id,
                    path,
                    mode,
                    scope_type,
                    scope_id,
                    created_at,
                },
            )
            .collect())
    }

    /// Add a steering file
    pub async fn add(
        pool: &SqlitePool,
        path: &str,
        mode: &str,
        scope_type: Option<&str>,
        scope_id: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO steering (path, mode, scope_type, scope_id, created_at) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(path, scope_type, scope_id) DO UPDATE SET mode = ?",
        )
        .bind(path)
        .bind(mode)
        .bind(scope_type)
        .bind(scope_id)
        .bind(&now)
        .bind(mode)
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Remove a steering file by path and scope
    pub async fn remove(
        pool: &SqlitePool,
        path: &str,
        scope_type: Option<&str>,
        scope_id: Option<&str>,
    ) -> Result<bool> {
        let result = match (scope_type, scope_id) {
            (None, _) => {
                sqlx::query("DELETE FROM steering WHERE path = ? AND scope_type IS NULL")
                    .bind(path)
                    .execute(pool)
                    .await?
            }
            (Some(st), Some(sid)) => {
                sqlx::query(
                    "DELETE FROM steering WHERE path = ? AND scope_type = ? AND scope_id = ?",
                )
                .bind(path)
                .bind(st)
                .bind(sid)
                .execute(pool)
                .await?
            }
            _ => return Ok(false),
        };
        Ok(result.rows_affected() > 0)
    }

    /// Delete all steering files attached to a session (for cleanup on session close)
    pub async fn delete_by_session(pool: &SqlitePool, session_id: &str) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM steering WHERE scope_type = 'session' AND scope_id = ?")
                .bind(session_id)
                .execute(pool)
                .await?;
        Ok(result.rows_affected())
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

/// Database operations for search (FTS5-backed)
pub mod search {
    use granary_types::{Project, Task};

    use super::*;

    #[derive(Debug, sqlx::FromRow)]
    pub struct FtsMatch {
        pub entity_type: String,
        pub entity_id: String,
        pub rank: f64,
    }

    /// Search all entity types using FTS5 full-text search with BM25 ranking
    pub async fn search_all(pool: &SqlitePool, query: &str, limit: i32) -> Result<Vec<FtsMatch>> {
        let rows = sqlx::query_as::<_, FtsMatch>(
            r#"
            SELECT entity_type, entity_id, rank
            FROM search_index
            WHERE search_index MATCH ?
            ORDER BY bm25(search_index, 0.0, 0.0, 10.0, 1.0, 2.0)
            LIMIT ?
            "#,
        )
        .bind(query)
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(rows)
    }

    /// Search projects using FTS5 full-text search
    pub async fn search_projects(pool: &SqlitePool, query: &str) -> Result<Vec<Project>> {
        let ids = sqlx::query_scalar::<_, String>(
            r#"
            SELECT entity_id
            FROM search_index
            WHERE search_index MATCH ? AND entity_type = 'project'
            ORDER BY bm25(search_index, 0.0, 0.0, 10.0, 1.0, 2.0)
            LIMIT 50
            "#,
        )
        .bind(query)
        .fetch_all(pool)
        .await?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let json_ids = serde_json::to_string(&ids)?;
        let projects = sqlx::query_as::<_, Project>(
            "SELECT * FROM projects WHERE id IN (SELECT value FROM json_each(?))",
        )
        .bind(json_ids)
        .fetch_all(pool)
        .await?;
        Ok(projects)
    }

    /// Search tasks using FTS5 full-text search
    pub async fn search_tasks(pool: &SqlitePool, query: &str) -> Result<Vec<Task>> {
        let ids = sqlx::query_scalar::<_, String>(
            r#"
            SELECT entity_id
            FROM search_index
            WHERE search_index MATCH ? AND entity_type = 'task'
            ORDER BY bm25(search_index, 0.0, 0.0, 10.0, 1.0, 2.0)
            LIMIT 50
            "#,
        )
        .bind(query)
        .fetch_all(pool)
        .await?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let json_ids = serde_json::to_string(&ids)?;
        let tasks = sqlx::query_as::<_, Task>(
            "SELECT * FROM tasks WHERE id IN (SELECT value FROM json_each(?))",
        )
        .bind(json_ids)
        .fetch_all(pool)
        .await?;
        Ok(tasks)
    }

    /// Search initiatives using FTS5 full-text search
    pub async fn search_initiatives(
        pool: &SqlitePool,
        query: &str,
    ) -> Result<Vec<crate::models::Initiative>> {
        let ids = sqlx::query_scalar::<_, String>(
            r#"
            SELECT entity_id
            FROM search_index
            WHERE search_index MATCH ? AND entity_type = 'initiative'
            ORDER BY bm25(search_index, 0.0, 0.0, 10.0, 1.0, 2.0)
            LIMIT 50
            "#,
        )
        .bind(query)
        .fetch_all(pool)
        .await?;

        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let json_ids = serde_json::to_string(&ids)?;
        let initiatives = sqlx::query_as::<_, crate::models::Initiative>(
            "SELECT * FROM initiatives WHERE id IN (SELECT value FROM json_each(?))",
        )
        .bind(json_ids)
        .fetch_all(pool)
        .await?;
        Ok(initiatives)
    }
}

/// Database operations for getting next tasks across an initiative
/// This respects both project-to-project dependencies and task-to-task dependencies
pub mod initiative_tasks {
    use granary_types::Task;

    use super::*;

    /// Get all unblocked tasks across an initiative.
    ///
    /// A task is actionable only if:
    /// 1. Its project has no unmet project dependencies (all dependency projects have all tasks done)
    /// 2. The task itself has no unmet task dependencies (all dependency tasks are done)
    /// 3. The task is not blocked (status != blocked, no blocked_reason)
    /// 4. The task is todo or in_progress
    /// 5. The project is active (not archived)
    ///
    /// Results are sorted by priority (P0 first), due_at (earliest first), created_at, and id (for determinism).
    pub async fn get_next(pool: &SqlitePool, initiative_id: &str, all: bool) -> Result<Vec<Task>> {
        // Step 1: Get all active (non-archived) project IDs in the initiative
        let project_rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT p.id FROM projects p
            JOIN initiative_projects ip ON p.id = ip.project_id
            WHERE ip.initiative_id = ?
              AND p.status = 'active'
            "#,
        )
        .bind(initiative_id)
        .fetch_all(pool)
        .await?;

        if project_rows.is_empty() {
            return Ok(Vec::new());
        }

        let all_project_ids: Vec<String> = project_rows.into_iter().map(|(id,)| id).collect();

        // Step 2: Find projects with unmet dependencies
        // A project has unmet dependencies if any of its dependencies has incomplete tasks
        let mut blocked_project_ids: Vec<String> = Vec::new();

        for project_id in &all_project_ids {
            // Check if this project has any unmet project dependencies
            let unmet_count: (i64,) = sqlx::query_as(
                r#"
                SELECT COUNT(*) FROM project_dependencies pd
                JOIN projects dep_p ON dep_p.id = pd.depends_on_project_id
                WHERE pd.project_id = ?
                AND dep_p.status NOT IN ('done', 'archived')
                AND EXISTS (
                    SELECT 1 FROM tasks t
                    WHERE t.project_id = pd.depends_on_project_id
                    AND t.status != 'done'
                )
                "#,
            )
            .bind(project_id)
            .fetch_one(pool)
            .await?;

            if unmet_count.0 > 0 {
                blocked_project_ids.push(project_id.clone());
            }
        }

        // Step 3: Get unblocked project IDs
        let unblocked_project_ids: Vec<&String> = all_project_ids
            .iter()
            .filter(|id| !blocked_project_ids.contains(id))
            .collect();

        if unblocked_project_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Step 4: Query for unblocked tasks in unblocked projects
        let json_project_ids = serde_json::to_string(&unblocked_project_ids)?;

        let limit = if all { 1000i32 } else { 1i32 };

        let tasks = sqlx::query_as::<_, Task>(
            r#"
            SELECT t.*
            FROM tasks t
            WHERE t.project_id IN (SELECT value FROM json_each(?))
              AND t.status IN ('todo', 'in_progress')
              AND t.blocked_reason IS NULL
              AND NOT EXISTS (
                  SELECT 1 FROM task_dependencies td
                  JOIN tasks dep ON dep.id = td.depends_on_task_id
                  WHERE td.task_id = t.id
                    AND dep.status != 'done'
              )
            ORDER BY
                CASE t.priority
                    WHEN 'P0' THEN 0
                    WHEN 'P1' THEN 1
                    WHEN 'P2' THEN 2
                    WHEN 'P3' THEN 3
                    WHEN 'P4' THEN 4
                END,
                t.due_at NULLS LAST,
                t.created_at,
                t.id
            LIMIT ?
            "#,
        )
        .bind(&json_project_ids)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(tasks)
    }
}

/// Database operations for workers
/// Workers are stored in a GLOBAL database (~/.granary/workers.db)
pub mod workers {
    use super::*;
    use crate::models::ids::generate_worker_id;
    use crate::models::{CreateWorker, UpdateWorkerStatus, Worker, WorkerStatus};

    /// Create a new worker record
    pub async fn create(pool: &SqlitePool, input: &CreateWorker) -> Result<Worker> {
        let id = generate_worker_id();
        let now = chrono::Utc::now().to_rfc3339();
        let args_json = serde_json::to_string(&input.args)?;
        let filters_json = serde_json::to_string(&input.filters)?;
        let env_json = serde_json::to_string(&input.env)?;

        sqlx::query(
            r#"
            INSERT INTO workers (id, runner_name, command, args, event_type, filters,
                concurrency, instance_path, status, detached, created_at, updated_at, env, pipeline_steps)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.runner_name)
        .bind(&input.command)
        .bind(&args_json)
        .bind(&input.event_type)
        .bind(&filters_json)
        .bind(input.concurrency)
        .bind(&input.instance_path)
        .bind(input.detached)
        .bind(&now)
        .bind(&now)
        .bind(&env_json)
        .bind(&input.pipeline_steps)
        .execute(pool)
        .await?;

        // Fetch and return the created worker
        get(pool, &id).await?.ok_or_else(|| {
            crate::error::GranaryError::Conflict(
                "Failed to create worker: could not retrieve after insert".to_string(),
            )
        })
    }

    /// Column list for Worker queries (must match Worker struct field order)
    const WORKER_COLUMNS: &str = r#"
        id, runner_name, command, args, event_type, filters, concurrency,
        instance_path, status, error_message, pid, detached, created_at,
        updated_at, stopped_at, last_event_id, env, pipeline_steps
    "#;

    /// Get a worker by ID
    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Worker>> {
        let query = format!("SELECT {} FROM workers WHERE id = ?", WORKER_COLUMNS);
        let worker = sqlx::query_as::<_, Worker>(&query)
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(worker)
    }

    /// List all workers (global registry)
    pub async fn list(pool: &SqlitePool) -> Result<Vec<Worker>> {
        let query = format!(
            "SELECT {} FROM workers ORDER BY created_at DESC",
            WORKER_COLUMNS
        );
        let workers = sqlx::query_as::<_, Worker>(&query).fetch_all(pool).await?;
        Ok(workers)
    }

    /// List workers by status
    pub async fn list_by_status(pool: &SqlitePool, status: WorkerStatus) -> Result<Vec<Worker>> {
        let query = format!(
            "SELECT {} FROM workers WHERE status = ? ORDER BY created_at DESC",
            WORKER_COLUMNS
        );
        let workers = sqlx::query_as::<_, Worker>(&query)
            .bind(status.as_str())
            .fetch_all(pool)
            .await?;
        Ok(workers)
    }

    /// List workers for a specific workspace/instance
    pub async fn list_by_instance(pool: &SqlitePool, instance_path: &str) -> Result<Vec<Worker>> {
        let query = format!(
            "SELECT {} FROM workers WHERE instance_path = ? ORDER BY created_at DESC",
            WORKER_COLUMNS
        );
        let workers = sqlx::query_as::<_, Worker>(&query)
            .bind(instance_path)
            .fetch_all(pool)
            .await?;
        Ok(workers)
    }

    /// List workers by event type
    pub async fn list_by_event_type(pool: &SqlitePool, event_type: &str) -> Result<Vec<Worker>> {
        let query = format!(
            "SELECT {} FROM workers WHERE event_type = ? ORDER BY created_at DESC",
            WORKER_COLUMNS
        );
        let workers = sqlx::query_as::<_, Worker>(&query)
            .bind(event_type)
            .fetch_all(pool)
            .await?;
        Ok(workers)
    }

    /// Update worker status (and optionally error message and pid)
    pub async fn update_status(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateWorkerStatus,
    ) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let stopped_at = if matches!(update.status, WorkerStatus::Stopped | WorkerStatus::Error) {
            Some(now.clone())
        } else {
            None
        };

        let result = sqlx::query(
            r#"
            UPDATE workers
            SET status = ?, error_message = ?, pid = ?, stopped_at = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(update.status.as_str())
        .bind(&update.error_message)
        .bind(update.pid)
        .bind(&stopped_at)
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Update worker PID (when worker starts running)
    pub async fn update_pid(pool: &SqlitePool, id: &str, pid: i64) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE workers SET pid = ?, status = 'running', updated_at = ? WHERE id = ?",
        )
        .bind(pid)
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete a worker record
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM workers WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete all workers for a specific workspace/instance
    pub async fn delete_by_instance(pool: &SqlitePool, instance_path: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM workers WHERE instance_path = ?")
            .bind(instance_path)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Get running workers (for health checks)
    pub async fn get_running(pool: &SqlitePool) -> Result<Vec<Worker>> {
        let query = format!(
            "SELECT {} FROM workers WHERE status = 'running' ORDER BY created_at DESC",
            WORKER_COLUMNS
        );
        let workers = sqlx::query_as::<_, Worker>(&query).fetch_all(pool).await?;
        Ok(workers)
    }

    /// List active workers (running or pending)
    ///
    /// This is used by WorkerManager to filter out stopped/errored workers.
    pub async fn list_active(pool: &SqlitePool) -> Result<Vec<Worker>> {
        let query = format!(
            "SELECT {} FROM workers WHERE status IN ('running', 'pending') ORDER BY created_at DESC",
            WORKER_COLUMNS
        );
        let workers = sqlx::query_as::<_, Worker>(&query).fetch_all(pool).await?;
        Ok(workers)
    }

    /// Count workers by status
    pub async fn count_by_status(pool: &SqlitePool, status: WorkerStatus) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM workers WHERE status = ?")
            .bind(status.as_str())
            .fetch_one(pool)
            .await?;
        Ok(count)
    }

    /// Update the last_event_id cursor for a worker
    ///
    /// This is used for event polling to track which events have been processed.
    pub async fn update_cursor(pool: &SqlitePool, id: &str, last_event_id: i64) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();
        let result =
            sqlx::query("UPDATE workers SET last_event_id = ?, updated_at = ? WHERE id = ?")
                .bind(last_event_id)
                .bind(&now)
                .bind(id)
                .execute(pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// Database operations for runs
/// Runs are stored in the same GLOBAL database as workers (~/.granary/workers.db)
pub mod runs {
    use super::*;
    use crate::models::ids::generate_run_id;
    use crate::models::run::{CreateRun, Run, RunStatus, ScheduleRetry, UpdateRunStatus};

    /// Create a new run record
    pub async fn create(pool: &SqlitePool, input: &CreateRun) -> Result<Run> {
        let id = generate_run_id();
        let now = chrono::Utc::now().to_rfc3339();
        let args_json = serde_json::to_string(&input.args)?;

        sqlx::query(
            r#"
            INSERT INTO runs (id, worker_id, event_id, event_type, entity_id, command, args,
                status, attempt, max_attempts, log_path, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', 1, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.worker_id)
        .bind(input.event_id)
        .bind(&input.event_type)
        .bind(&input.entity_id)
        .bind(&input.command)
        .bind(&args_json)
        .bind(input.max_attempts)
        .bind(&input.log_path)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;

        // Fetch and return the created run
        get(pool, &id).await?.ok_or_else(|| {
            crate::error::GranaryError::Conflict(
                "Failed to create run: could not retrieve after insert".to_string(),
            )
        })
    }

    /// Get a run by ID
    pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<Run>> {
        let run = sqlx::query_as::<_, Run>("SELECT * FROM runs WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(run)
    }

    /// List all runs for a worker
    pub async fn list_by_worker(pool: &SqlitePool, worker_id: &str) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>(
            "SELECT * FROM runs WHERE worker_id = ? ORDER BY created_at DESC",
        )
        .bind(worker_id)
        .fetch_all(pool)
        .await?;
        Ok(runs)
    }

    /// List all runs (global list)
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>("SELECT * FROM runs ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?;
        Ok(runs)
    }

    /// List runs pending retry (where next_retry_at is before the given time)
    pub async fn list_pending_retries(pool: &SqlitePool, before_time: &str) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>(
            r#"
            SELECT * FROM runs
            WHERE status = 'pending'
              AND attempt > 1
              AND next_retry_at IS NOT NULL
              AND next_retry_at <= ?
            ORDER BY next_retry_at ASC
            "#,
        )
        .bind(before_time)
        .fetch_all(pool)
        .await?;
        Ok(runs)
    }

    /// Count running runs for a worker (for concurrency check)
    pub async fn count_running_by_worker(pool: &SqlitePool, worker_id: &str) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM runs WHERE worker_id = ? AND status = 'running'",
        )
        .bind(worker_id)
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    /// Update run status (and optionally exit_code, error_message, pid)
    pub async fn update_status(
        pool: &SqlitePool,
        id: &str,
        update: &UpdateRunStatus,
    ) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();

        // Determine started_at and completed_at based on status
        let (started_at, completed_at) = match update.status {
            RunStatus::Running => (Some(now.clone()), None),
            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled => {
                (None, Some(now.clone()))
            }
            _ => (None, None),
        };

        let result = sqlx::query(
            r#"
            UPDATE runs
            SET status = ?, exit_code = ?, error_message = ?, pid = ?,
                started_at = COALESCE(?, started_at),
                completed_at = COALESCE(?, completed_at),
                updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(update.status.as_str())
        .bind(update.exit_code)
        .bind(&update.error_message)
        .bind(update.pid)
        .bind(&started_at)
        .bind(&completed_at)
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Schedule a retry for a run
    pub async fn update_for_retry(
        pool: &SqlitePool,
        id: &str,
        retry: &ScheduleRetry,
    ) -> Result<bool> {
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE runs
            SET status = 'pending', next_retry_at = ?, attempt = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(&retry.next_retry_at)
        .bind(retry.attempt)
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Cancel all runs for a worker
    pub async fn cancel_by_worker(pool: &SqlitePool, worker_id: &str) -> Result<u64> {
        let now = chrono::Utc::now().to_rfc3339();

        let result = sqlx::query(
            r#"
            UPDATE runs
            SET status = 'cancelled', completed_at = ?, updated_at = ?
            WHERE worker_id = ? AND status IN ('pending', 'running', 'paused')
            "#,
        )
        .bind(&now)
        .bind(&now)
        .bind(worker_id)
        .execute(pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// List runs by status
    pub async fn list_by_status(pool: &SqlitePool, status: RunStatus) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>(
            "SELECT * FROM runs WHERE status = ? ORDER BY created_at DESC",
        )
        .bind(status.as_str())
        .fetch_all(pool)
        .await?;
        Ok(runs)
    }

    /// Delete a run record
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM runs WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Delete all runs for a worker
    pub async fn delete_by_worker(pool: &SqlitePool, worker_id: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM runs WHERE worker_id = ?")
            .bind(worker_id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Get pending runs (not yet started, not retries)
    pub async fn get_pending(pool: &SqlitePool, worker_id: &str) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>(
            r#"
            SELECT * FROM runs
            WHERE worker_id = ? AND status = 'pending' AND attempt = 1
            ORDER BY created_at ASC
            "#,
        )
        .bind(worker_id)
        .fetch_all(pool)
        .await?;
        Ok(runs)
    }

    /// Count runs by status for a worker
    pub async fn count_by_status_for_worker(
        pool: &SqlitePool,
        worker_id: &str,
        status: RunStatus,
    ) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM runs WHERE worker_id = ? AND status = ?",
        )
        .bind(worker_id)
        .bind(status.as_str())
        .fetch_one(pool)
        .await?;
        Ok(count)
    }

    /// List active runs (pending, running, paused)
    ///
    /// This is used by WorkerManager to get runs that are still in progress.
    pub async fn list_active(pool: &SqlitePool) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>(
            "SELECT * FROM runs WHERE status IN ('pending', 'running', 'paused') ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(runs)
    }

    /// List runs for a specific worker filtered by status
    pub async fn list_by_worker_and_status(
        pool: &SqlitePool,
        worker_id: &str,
        status: RunStatus,
    ) -> Result<Vec<Run>> {
        let runs = sqlx::query_as::<_, Run>(
            "SELECT * FROM runs WHERE worker_id = ? AND status = ? ORDER BY created_at DESC",
        )
        .bind(worker_id)
        .bind(status.as_str())
        .fetch_all(pool)
        .await?;
        Ok(runs)
    }
}
