use granary_types::{CreateProject, Project, ProjectStatus, UpdateProject};
use sqlx::SqlitePool;

use crate::db;
use crate::error::{GranaryError, Result};
use crate::models::*;

/// Create a new project
pub async fn create_project(pool: &SqlitePool, input: CreateProject) -> Result<Project> {
    let id = generate_project_id(&input.name);
    let slug = normalize_slug(&input.name);
    let now = chrono::Utc::now().to_rfc3339();

    let tags = if input.tags.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&input.tags)?)
    };

    let steering_refs = if input.steering_refs.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&input.steering_refs)?)
    };

    let default_session_policy = input
        .default_session_policy
        .map(|p| serde_json::to_string(&p))
        .transpose()?;

    let project = Project {
        id: id.clone(),
        slug,
        name: input.name,
        description: input.description,
        owner: input.owner,
        status: ProjectStatus::Active.as_str().to_string(),
        tags,
        default_session_policy,
        steering_refs,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };

    db::projects::create(pool, &project).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::ProjectCreated,
            entity_type: EntityType::Project,
            entity_id: project.id.clone(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({
                "name": project.name,
            }),
        },
    )
    .await?;

    Ok(project)
}

/// Get a project by ID
pub async fn get_project(pool: &SqlitePool, id: &str) -> Result<Project> {
    db::projects::get(pool, id)
        .await?
        .ok_or_else(|| GranaryError::ProjectNotFound(id.to_string()))
}

/// List all projects
pub async fn list_projects(pool: &SqlitePool, include_archived: bool) -> Result<Vec<Project>> {
    db::projects::list(pool, include_archived).await
}

/// Update a project
pub async fn update_project(
    pool: &SqlitePool,
    id: &str,
    updates: UpdateProject,
) -> Result<Project> {
    let mut project = get_project(pool, id).await?;

    if let Some(name) = updates.name {
        project.name = name;
    }
    if let Some(description) = updates.description {
        project.description = Some(description);
    }
    if let Some(owner) = updates.owner {
        project.owner = Some(owner);
    }
    if let Some(status) = updates.status {
        project.status = status.as_str().to_string();
    }
    if let Some(tags) = updates.tags {
        project.tags = Some(serde_json::to_string(&tags)?);
    }
    if let Some(policy) = updates.default_session_policy {
        project.default_session_policy = Some(serde_json::to_string(&policy)?);
    }
    if let Some(refs) = updates.steering_refs {
        project.steering_refs = Some(serde_json::to_string(&refs)?);
    }

    let updated = db::projects::update(pool, &project).await?;
    if !updated {
        return Err(GranaryError::VersionMismatch {
            expected: project.version,
            found: project.version + 1,
        });
    }

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::ProjectUpdated,
            entity_type: EntityType::Project,
            entity_id: project.id.clone(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({}),
        },
    )
    .await?;

    // Refetch to get updated version
    get_project(pool, id).await
}

/// Archive a project
pub async fn archive_project(pool: &SqlitePool, id: &str) -> Result<Project> {
    let project = get_project(pool, id).await?;

    if project.status == ProjectStatus::Archived.as_str() {
        return Err(GranaryError::Conflict(format!(
            "Project {} is already archived",
            id
        )));
    }

    db::projects::archive(pool, id).await?;

    // Log event
    db::events::create(
        pool,
        &CreateEvent {
            event_type: EventType::ProjectArchived,
            entity_type: EntityType::Project,
            entity_id: id.to_string(),
            actor: None,
            session_id: None,
            payload: serde_json::json!({}),
        },
    )
    .await?;

    get_project(pool, id).await
}
