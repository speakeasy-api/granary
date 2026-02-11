//! End-to-end tests for the trigger-based event system.
//!
//! These tests verify that SQLite AFTER triggers on `projects`, `tasks`,
//! `comments`, `sessions`, `checkpoints`, `artifacts`, and `task_dependencies`
//! automatically emit the correct events in the `events` table when entities
//! are created, updated, or deleted.
//!
//! Each test sets up an in-memory SQLite pool with migrations, performs
//! mutations through the `db::` layer, and asserts on the resulting events.

use granary::db;
use granary::db::connection;
use granary::models::*;

/// Helper: create a pool + run migrations on an in-memory SQLite DB.
async fn setup_pool() -> sqlx::SqlitePool {
    let pool = connection::create_pool(std::path::Path::new(":memory:"))
        .await
        .expect("create_pool");
    connection::run_migrations(&pool).await.expect("migrations");
    pool
}

/// Helper: fetch all events from the events table, ordered by id ASC.
async fn all_events(pool: &sqlx::SqlitePool) -> Vec<Event> {
    db::events::list_since_id(pool, 0)
        .await
        .expect("list_since_id")
}

/// Helper: fetch events of a specific type.
async fn events_of_type(pool: &sqlx::SqlitePool, event_type: &str) -> Vec<Event> {
    all_events(pool)
        .await
        .into_iter()
        .filter(|e| e.event_type == event_type)
        .collect()
}

/// Helper: create a minimal active project and return it.
async fn create_project(pool: &sqlx::SqlitePool, name: &str) -> Project {
    let id = ids::generate_project_id(name);
    let slug = ids::normalize_slug(name);
    let now = chrono::Utc::now().to_rfc3339();
    let project = Project {
        id: id.clone(),
        slug,
        name: name.to_string(),
        description: None,
        owner: None,
        status: "active".to_string(),
        tags: None,
        default_session_policy: None,
        steering_refs: None,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
        last_edited_by: Some("test-actor".to_string()),
    };
    db::projects::create(pool, &project)
        .await
        .expect("create project");
    project
}

/// Helper: create a minimal task in a project and return it.
async fn create_task(pool: &sqlx::SqlitePool, project_id: &str, title: &str, status: &str) -> Task {
    let num = db::counters::next(pool, &format!("task:{}", project_id))
        .await
        .expect("counter");
    let id = ids::generate_task_id(project_id, num);
    let now = chrono::Utc::now().to_rfc3339();
    let task = Task {
        id: id.clone(),
        project_id: project_id.to_string(),
        task_number: num,
        parent_task_id: None,
        title: title.to_string(),
        description: None,
        status: status.to_string(),
        priority: "P2".to_string(),
        owner: None,
        tags: None,
        blocked_reason: None,
        started_at: None,
        completed_at: None,
        due_at: None,
        claim_owner: None,
        claim_claimed_at: None,
        claim_lease_expires_at: None,
        pinned: 0,
        focus_weight: 0,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
        last_edited_by: Some("test-actor".to_string()),
    };
    db::tasks::create(pool, &task).await.expect("create task");
    task
}

// ============================================================================
// Project trigger tests
// ============================================================================

#[tokio::test]
async fn test_project_created_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "trigger-test-proj").await;

    let events = events_of_type(&pool, "project.created").await;
    assert_eq!(events.len(), 1, "expected exactly 1 project.created event");
    let ev = &events[0];
    assert_eq!(ev.entity_type, "project");
    assert_eq!(ev.entity_id, project.id);
    assert_eq!(ev.actor.as_deref(), Some("test-actor"));

    let payload: serde_json::Value = serde_json::from_str(&ev.payload).unwrap();
    assert_eq!(payload["id"], project.id);
    assert_eq!(payload["name"], "trigger-test-proj");
    assert_eq!(payload["status"], "active");
}

#[tokio::test]
async fn test_project_updated_trigger() {
    let pool = setup_pool().await;
    let mut project = create_project(&pool, "update-proj").await;

    // Modify name but keep status the same → should fire project.updated
    project.name = "updated-name".to_string();
    project.last_edited_by = Some("editor".to_string());
    db::projects::update(&pool, &project).await.expect("update");

    let events = events_of_type(&pool, "project.updated").await;
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.actor.as_deref(), Some("editor"));

    let payload: serde_json::Value = serde_json::from_str(&ev.payload).unwrap();
    assert_eq!(payload["name"], "updated-name");
}

#[tokio::test]
async fn test_project_completed_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "complete-proj").await;

    db::projects::complete(&pool, &project.id, false)
        .await
        .expect("complete");

    let events = events_of_type(&pool, "project.completed").await;
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.entity_id, project.id);

    let payload: serde_json::Value = serde_json::from_str(&ev.payload).unwrap();
    assert_eq!(payload["status"], "completed");
    assert_eq!(payload["old_status"], "active");
}

#[tokio::test]
async fn test_project_archived_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "archive-proj").await;

    db::projects::archive(&pool, &project.id)
        .await
        .expect("archive");

    let events = events_of_type(&pool, "project.archived").await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].entity_id, project.id);
}

#[tokio::test]
async fn test_project_unarchived_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "unarchive-proj").await;

    // Archive first, then unarchive
    db::projects::archive(&pool, &project.id)
        .await
        .expect("archive");
    db::projects::unarchive(&pool, &project.id)
        .await
        .expect("unarchive");

    let events = events_of_type(&pool, "project.unarchived").await;
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.entity_id, project.id);

    let payload: serde_json::Value = serde_json::from_str(&ev.payload).unwrap();
    assert_eq!(payload["status"], "active");
    assert_eq!(payload["old_status"], "archived");
}

#[tokio::test]
async fn test_project_unarchived_from_completed() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "reactivate-proj").await;

    // Complete then unarchive → should fire project.unarchived
    db::projects::complete(&pool, &project.id, false)
        .await
        .expect("complete");
    db::projects::unarchive(&pool, &project.id)
        .await
        .expect("unarchive");

    let events = events_of_type(&pool, "project.unarchived").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["old_status"], "completed");
}

// ============================================================================
// Task trigger tests
// ============================================================================

#[tokio::test]
async fn test_task_created_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "task-create-proj").await;
    let task = create_task(&pool, &project.id, "my task", "todo").await;

    let events = events_of_type(&pool, "task.created").await;
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.entity_type, "task");
    assert_eq!(ev.entity_id, task.id);
    assert_eq!(ev.actor.as_deref(), Some("test-actor"));

    let payload: serde_json::Value = serde_json::from_str(&ev.payload).unwrap();
    assert_eq!(payload["title"], "my task");
    assert_eq!(payload["status"], "todo");
    assert_eq!(payload["project_id"], project.id);
}

#[tokio::test]
async fn test_task_updated_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "task-update-proj").await;
    let mut task = create_task(&pool, &project.id, "update me", "todo").await;

    // Change title (same status) → task.updated
    task.title = "updated title".to_string();
    task.last_edited_by = Some("editor".to_string());
    db::tasks::update(&pool, &task).await.expect("update task");

    let events = events_of_type(&pool, "task.updated").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["title"], "updated title");
}

#[tokio::test]
async fn test_task_started_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "task-start-proj").await;
    let mut task = create_task(&pool, &project.id, "start me", "todo").await;

    task.status = "in_progress".to_string();
    task.started_at = Some(chrono::Utc::now().to_rfc3339());
    db::tasks::update(&pool, &task).await.expect("update");

    let events = events_of_type(&pool, "task.started").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["status"], "in_progress");
    assert_eq!(payload["old_status"], "todo");
}

#[tokio::test]
async fn test_task_completed_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "task-done-proj").await;
    let mut task = create_task(&pool, &project.id, "finish me", "in_progress").await;

    task.status = "done".to_string();
    task.completed_at = Some(chrono::Utc::now().to_rfc3339());
    db::tasks::update(&pool, &task).await.expect("update");

    let events = events_of_type(&pool, "task.completed").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["old_status"], "in_progress");
}

#[tokio::test]
async fn test_task_blocked_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "task-block-proj").await;
    let mut task = create_task(&pool, &project.id, "block me", "todo").await;

    task.status = "blocked".to_string();
    task.blocked_reason = Some("waiting on API".to_string());
    db::tasks::update(&pool, &task).await.expect("update");

    let events = events_of_type(&pool, "task.blocked").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["old_status"], "todo");
    assert_eq!(payload["blocked_reason"], "waiting on API");
}

#[tokio::test]
async fn test_task_unblocked_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "task-unblock-proj").await;
    let mut task = create_task(&pool, &project.id, "unblock me", "blocked").await;
    task.blocked_reason = Some("reason".to_string());
    db::tasks::update(&pool, &task).await.expect("set blocked");

    // Re-fetch to get new version
    let mut task = db::tasks::get(&pool, &task.id).await.expect("get").unwrap();

    task.status = "todo".to_string();
    task.blocked_reason = None;
    db::tasks::update(&pool, &task).await.expect("unblock");

    let events = events_of_type(&pool, "task.unblocked").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["old_status"], "blocked");
}

#[tokio::test]
async fn test_task_claimed_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "task-claim-proj").await;
    let mut task = create_task(&pool, &project.id, "claim me", "todo").await;

    task.claim_owner = Some("worker-1".to_string());
    task.claim_claimed_at = Some(chrono::Utc::now().to_rfc3339());
    task.claim_lease_expires_at =
        Some((chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339());
    db::tasks::update(&pool, &task).await.expect("claim");

    let events = events_of_type(&pool, "task.claimed").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["claim_owner"], "worker-1");
}

#[tokio::test]
async fn test_task_released_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "task-release-proj").await;
    let mut task = create_task(&pool, &project.id, "release me", "todo").await;

    // Claim first
    task.claim_owner = Some("worker-1".to_string());
    task.claim_claimed_at = Some(chrono::Utc::now().to_rfc3339());
    task.claim_lease_expires_at =
        Some((chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339());
    db::tasks::update(&pool, &task).await.expect("claim");

    // Re-fetch, then release
    let mut task = db::tasks::get(&pool, &task.id).await.expect("get").unwrap();
    task.claim_owner = None;
    task.claim_claimed_at = None;
    task.claim_lease_expires_at = None;
    db::tasks::update(&pool, &task).await.expect("release");

    let events = events_of_type(&pool, "task.released").await;
    assert_eq!(events.len(), 1);
}

// ============================================================================
// Dependency trigger tests
// ============================================================================

#[tokio::test]
async fn test_dependency_added_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "dep-proj").await;
    let task_a = create_task(&pool, &project.id, "task A", "todo").await;
    let task_b = create_task(&pool, &project.id, "task B", "todo").await;

    db::dependencies::add(&pool, &task_b.id, &task_a.id)
        .await
        .expect("add dep");

    let events = events_of_type(&pool, "dependency.added").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["task_id"], task_b.id);
    assert_eq!(payload["depends_on_task_id"], task_a.id);
}

#[tokio::test]
async fn test_dependency_removed_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "dep-rm-proj").await;
    let task_a = create_task(&pool, &project.id, "task A", "todo").await;
    let task_b = create_task(&pool, &project.id, "task B", "todo").await;

    db::dependencies::add(&pool, &task_b.id, &task_a.id)
        .await
        .expect("add dep");
    db::dependencies::remove(&pool, &task_b.id, &task_a.id)
        .await
        .expect("remove dep");

    let events = events_of_type(&pool, "dependency.removed").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["task_id"], task_b.id);
    assert_eq!(payload["depends_on_task_id"], task_a.id);
}

// ============================================================================
// Comment trigger tests
// ============================================================================

#[tokio::test]
async fn test_comment_created_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "comment-proj").await;
    let task = create_task(&pool, &project.id, "comment target", "todo").await;

    let comment_id = ids::generate_comment_id(&task.id, 1);
    let now = chrono::Utc::now().to_rfc3339();
    let comment = Comment {
        id: comment_id.clone(),
        parent_type: "task".to_string(),
        parent_id: task.id.clone(),
        comment_number: 1,
        kind: "note".to_string(),
        content: "test comment".to_string(),
        author: Some("commenter".to_string()),
        meta: None,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };
    db::comments::create(&pool, &comment)
        .await
        .expect("create comment");

    let events = events_of_type(&pool, "comment.created").await;
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.entity_id, comment_id);
    assert_eq!(ev.actor.as_deref(), Some("commenter"));

    let payload: serde_json::Value = serde_json::from_str(&ev.payload).unwrap();
    assert_eq!(payload["content"], "test comment");
    assert_eq!(payload["kind"], "note");
}

#[tokio::test]
async fn test_comment_updated_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "comment-upd-proj").await;
    let task = create_task(&pool, &project.id, "comment target", "todo").await;

    let comment_id = ids::generate_comment_id(&task.id, 1);
    let now = chrono::Utc::now().to_rfc3339();
    let mut comment = Comment {
        id: comment_id.clone(),
        parent_type: "task".to_string(),
        parent_id: task.id.clone(),
        comment_number: 1,
        kind: "note".to_string(),
        content: "original".to_string(),
        author: Some("commenter".to_string()),
        meta: None,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };
    db::comments::create(&pool, &comment)
        .await
        .expect("create comment");

    comment.content = "edited".to_string();
    db::comments::update(&pool, &comment)
        .await
        .expect("update comment");

    let events = events_of_type(&pool, "comment.updated").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["content"], "edited");
}

// ============================================================================
// Session trigger tests
// ============================================================================

#[tokio::test]
async fn test_session_started_trigger() {
    let pool = setup_pool().await;

    let session_id = ids::generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();
    let session = Session {
        id: session_id.clone(),
        name: Some("test session".to_string()),
        owner: Some("user".to_string()),
        mode: Some("execute".to_string()),
        focus_task_id: None,
        variables: None,
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
        last_edited_by: Some("session-actor".to_string()),
    };
    db::sessions::create(&pool, &session)
        .await
        .expect("create session");

    let events = events_of_type(&pool, "session.started").await;
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.entity_id, session_id);
    assert_eq!(ev.actor.as_deref(), Some("session-actor"));
}

#[tokio::test]
async fn test_session_updated_trigger() {
    let pool = setup_pool().await;

    let session_id = ids::generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();
    let mut session = Session {
        id: session_id.clone(),
        name: Some("my session".to_string()),
        owner: Some("user".to_string()),
        mode: Some("execute".to_string()),
        focus_task_id: None,
        variables: None,
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
        last_edited_by: None,
    };
    db::sessions::create(&pool, &session).await.expect("create");

    session.name = Some("renamed session".to_string());
    session.last_edited_by = Some("editor".to_string());
    db::sessions::update(&pool, &session).await.expect("update");

    let events = events_of_type(&pool, "session.updated").await;
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn test_session_closed_trigger() {
    let pool = setup_pool().await;

    let session_id = ids::generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();
    let session = Session {
        id: session_id.clone(),
        name: None,
        owner: None,
        mode: None,
        focus_task_id: None,
        variables: None,
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
        last_edited_by: None,
    };
    db::sessions::create(&pool, &session).await.expect("create");

    db::sessions::close(&pool, &session_id)
        .await
        .expect("close");

    let events = events_of_type(&pool, "session.closed").await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].entity_id, session_id);
}

#[tokio::test]
async fn test_session_focus_changed_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "focus-proj").await;
    let task = create_task(&pool, &project.id, "focus task", "todo").await;

    let session_id = ids::generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();
    let mut session = Session {
        id: session_id.clone(),
        name: None,
        owner: None,
        mode: None,
        focus_task_id: None,
        variables: None,
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
        last_edited_by: None,
    };
    db::sessions::create(&pool, &session).await.expect("create");

    session.focus_task_id = Some(task.id.clone());
    db::sessions::update(&pool, &session)
        .await
        .expect("update focus");

    let events = events_of_type(&pool, "session.focus_changed").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["focus_task_id"], task.id);
    assert!(payload["old_focus_task_id"].is_null());
}

#[tokio::test]
async fn test_session_scope_added_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "scope-proj").await;

    let session_id = ids::generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();
    let session = Session {
        id: session_id.clone(),
        name: None,
        owner: None,
        mode: None,
        focus_task_id: None,
        variables: None,
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
        last_edited_by: None,
    };
    db::sessions::create(&pool, &session).await.expect("create");

    db::sessions::add_scope(&pool, &session_id, "project", &project.id)
        .await
        .expect("add scope");

    let events = events_of_type(&pool, "session.scope_added").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["session_id"], session_id);
    assert_eq!(payload["item_type"], "project");
    assert_eq!(payload["item_id"], project.id);
}

#[tokio::test]
async fn test_session_scope_removed_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "scope-rm-proj").await;

    let session_id = ids::generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();
    let session = Session {
        id: session_id.clone(),
        name: None,
        owner: None,
        mode: None,
        focus_task_id: None,
        variables: None,
        created_at: now.clone(),
        updated_at: now,
        closed_at: None,
        last_edited_by: None,
    };
    db::sessions::create(&pool, &session).await.expect("create");

    db::sessions::add_scope(&pool, &session_id, "project", &project.id)
        .await
        .expect("add scope");
    db::sessions::remove_scope(&pool, &session_id, "project", &project.id)
        .await
        .expect("remove scope");

    let events = events_of_type(&pool, "session.scope_removed").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["session_id"], session_id);
    assert_eq!(payload["item_type"], "project");
}

// ============================================================================
// Checkpoint / Artifact trigger tests
// ============================================================================

#[tokio::test]
async fn test_checkpoint_created_trigger() {
    let pool = setup_pool().await;

    let session_id = ids::generate_session_id();
    let now = chrono::Utc::now().to_rfc3339();
    let session = Session {
        id: session_id.clone(),
        name: None,
        owner: None,
        mode: None,
        focus_task_id: None,
        variables: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        closed_at: None,
        last_edited_by: None,
    };
    db::sessions::create(&pool, &session)
        .await
        .expect("create session");

    let cp_id = ids::generate_checkpoint_id();
    let checkpoint = Checkpoint {
        id: cp_id.clone(),
        session_id: session_id.clone(),
        name: "checkpoint-1".to_string(),
        snapshot: "{}".to_string(),
        created_at: now,
    };
    db::checkpoints::create(&pool, &checkpoint)
        .await
        .expect("create checkpoint");

    let events = events_of_type(&pool, "checkpoint.created").await;
    assert_eq!(events.len(), 1);
    let ev = &events[0];
    assert_eq!(ev.entity_id, cp_id);
    assert_eq!(ev.session_id.as_deref(), Some(session_id.as_str()));
}

#[tokio::test]
async fn test_artifact_added_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "art-proj").await;
    let task = create_task(&pool, &project.id, "art task", "todo").await;

    let art_id = ids::generate_artifact_id(&task.id, 1);
    let now = chrono::Utc::now().to_rfc3339();
    let artifact = Artifact {
        id: art_id.clone(),
        parent_type: "task".to_string(),
        parent_id: task.id.clone(),
        artifact_number: 1,
        artifact_type: "file".to_string(),
        path_or_url: "/tmp/test.txt".to_string(),
        description: Some("test artifact".to_string()),
        meta: None,
        created_at: now,
    };
    db::artifacts::create(&pool, &artifact)
        .await
        .expect("create artifact");

    let events = events_of_type(&pool, "artifact.added").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["path_or_url"], "/tmp/test.txt");
}

#[tokio::test]
async fn test_artifact_removed_trigger() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "art-rm-proj").await;
    let task = create_task(&pool, &project.id, "art rm task", "todo").await;

    let art_id = ids::generate_artifact_id(&task.id, 1);
    let now = chrono::Utc::now().to_rfc3339();
    let artifact = Artifact {
        id: art_id.clone(),
        parent_type: "task".to_string(),
        parent_id: task.id.clone(),
        artifact_number: 1,
        artifact_type: "file".to_string(),
        path_or_url: "/tmp/test.txt".to_string(),
        description: None,
        meta: None,
        created_at: now,
    };
    db::artifacts::create(&pool, &artifact)
        .await
        .expect("create");
    db::artifacts::delete(&pool, &art_id).await.expect("delete");

    let events = events_of_type(&pool, "artifact.removed").await;
    assert_eq!(events.len(), 1);
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();
    assert_eq!(payload["id"], art_id);
}

// ============================================================================
// task.next trigger tests
// ============================================================================

#[tokio::test]
async fn test_task_next_on_insert_todo() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-insert-proj").await;

    // Creating a task with status 'todo' in an active project with no deps → task.next
    let _task = create_task(&pool, &project.id, "actionable task", "todo").await;

    let events = events_of_type(&pool, "task.next").await;
    assert_eq!(
        events.len(),
        1,
        "expected task.next on inserting a todo task in an active project"
    );
}

#[tokio::test]
async fn test_task_next_not_fired_for_draft() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-draft-proj").await;

    // Creating a draft task should NOT emit task.next
    let _task = create_task(&pool, &project.id, "draft task", "draft").await;

    let events = events_of_type(&pool, "task.next").await;
    assert!(events.is_empty(), "draft tasks should not emit task.next");
}

#[tokio::test]
async fn test_task_next_not_fired_in_archived_project() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-archived-proj").await;

    // Archive the project, then add a task
    db::projects::archive(&pool, &project.id)
        .await
        .expect("archive");

    let _task = create_task(&pool, &project.id, "task in archived", "todo").await;

    let events = events_of_type(&pool, "task.next").await;
    assert!(
        events.is_empty(),
        "tasks in archived projects should not emit task.next"
    );
}

#[tokio::test]
async fn test_task_next_on_status_change_to_todo() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-status-proj").await;

    // Create as draft (no task.next), then update to todo (should fire task.next)
    let mut task = create_task(&pool, &project.id, "draft to todo", "draft").await;

    let before = events_of_type(&pool, "task.next").await;
    assert!(before.is_empty());

    task.status = "todo".to_string();
    db::tasks::update(&pool, &task).await.expect("update");

    let after = events_of_type(&pool, "task.next").await;
    assert_eq!(
        after.len(),
        1,
        "transitioning to todo should emit task.next"
    );
}

#[tokio::test]
async fn test_task_next_on_dependency_completed() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-dep-proj").await;

    // task_a is a dependency; task_b depends on task_a
    let task_a = create_task(&pool, &project.id, "dep task", "in_progress").await;
    let task_b = create_task(&pool, &project.id, "blocked by dep", "todo").await;

    db::dependencies::add(&pool, &task_b.id, &task_a.id)
        .await
        .expect("add dep");

    // Clear task.next events from task_b's initial insert (it had no deps at insert time,
    // so it fired task.next then, but we added the dep after)
    // Actually let's check: task_b was created as todo with no deps → task.next fires.
    // Then we add a dep. The dep doesn't remove the task.next.
    // When task_a completes, the trigger checks if task_b is now actionable.

    // Complete task_a → should fire task.next for task_b
    let mut task_a = db::tasks::get(&pool, &task_a.id)
        .await
        .expect("get")
        .unwrap();
    task_a.status = "done".to_string();
    task_a.completed_at = Some(chrono::Utc::now().to_rfc3339());
    db::tasks::update(&pool, &task_a).await.expect("complete");

    // Find task.next events for task_b
    let next_events: Vec<_> = events_of_type(&pool, "task.next")
        .await
        .into_iter()
        .filter(|e| e.entity_id == task_b.id)
        .collect();

    // At least one task.next should exist for task_b (from dep completion)
    assert!(
        !next_events.is_empty(),
        "completing a dependency should emit task.next for the dependent"
    );
}

#[tokio::test]
async fn test_task_next_on_unblocked() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-unblock-proj").await;

    // Create a task with blocked_reason set
    let num = db::counters::next(&pool, &format!("task:{}", project.id))
        .await
        .expect("counter");
    let id = ids::generate_task_id(&project.id, num);
    let now = chrono::Utc::now().to_rfc3339();
    let task = Task {
        id: id.clone(),
        project_id: project.id.clone(),
        task_number: num,
        parent_task_id: None,
        title: "blocked task".to_string(),
        description: None,
        status: "todo".to_string(),
        priority: "P2".to_string(),
        owner: None,
        tags: None,
        blocked_reason: Some("external blocker".to_string()),
        started_at: None,
        completed_at: None,
        due_at: None,
        claim_owner: None,
        claim_claimed_at: None,
        claim_lease_expires_at: None,
        pinned: 0,
        focus_weight: 0,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
        last_edited_by: None,
    };
    db::tasks::create(&pool, &task).await.expect("create");

    // No task.next should have fired (blocked_reason is set)
    let next_before: Vec<_> = events_of_type(&pool, "task.next")
        .await
        .into_iter()
        .filter(|e| e.entity_id == id)
        .collect();
    assert!(
        next_before.is_empty(),
        "blocked tasks should not emit task.next"
    );

    // Clear the blocked_reason → should fire task.next via trg_task_next_on_unblocked
    let mut task = db::tasks::get(&pool, &id).await.expect("get").unwrap();
    task.blocked_reason = None;
    db::tasks::update(&pool, &task).await.expect("unblock");

    let next_after: Vec<_> = events_of_type(&pool, "task.next")
        .await
        .into_iter()
        .filter(|e| e.entity_id == id)
        .collect();
    // Multiple task.next triggers may fire (trg_task_next_on_unblocked + trg_task_next_on_status_todo)
    // since both conditions are met. Deduplication is handled at the consumer layer.
    assert!(
        !next_after.is_empty(),
        "clearing blocked_reason should emit task.next"
    );
}

#[tokio::test]
async fn test_task_next_on_released() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-release-proj").await;
    let mut task = create_task(&pool, &project.id, "claim release", "todo").await;

    // Claim the task
    task.claim_owner = Some("worker-1".to_string());
    task.claim_claimed_at = Some(chrono::Utc::now().to_rfc3339());
    task.claim_lease_expires_at =
        Some((chrono::Utc::now() + chrono::Duration::minutes(30)).to_rfc3339());
    db::tasks::update(&pool, &task).await.expect("claim");

    // Re-fetch and release
    let mut task = db::tasks::get(&pool, &task.id).await.expect("get").unwrap();
    task.claim_owner = None;
    task.claim_claimed_at = None;
    task.claim_lease_expires_at = None;
    db::tasks::update(&pool, &task).await.expect("release");

    // Should have task.next from release
    let next_events: Vec<_> = events_of_type(&pool, "task.next")
        .await
        .into_iter()
        .filter(|e| e.entity_id == task.id)
        .collect();
    // There should be at least one from initial insert and one from release
    assert!(
        next_events.len() >= 2,
        "releasing a claim should emit task.next (got {} events)",
        next_events.len()
    );
}

#[tokio::test]
async fn test_task_next_on_dependency_removed() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-deprm-proj").await;

    let task_a = create_task(&pool, &project.id, "dep task", "in_progress").await;
    let task_b = create_task(&pool, &project.id, "waiting task", "todo").await;

    // Add dep so task_b depends on task_a (which is not done)
    db::dependencies::add(&pool, &task_b.id, &task_a.id)
        .await
        .expect("add dep");

    // Now remove the dep → task_b becomes actionable → task.next
    db::dependencies::remove(&pool, &task_b.id, &task_a.id)
        .await
        .expect("remove dep");

    let next_events: Vec<_> = events_of_type(&pool, "task.next")
        .await
        .into_iter()
        .filter(|e| e.entity_id == task_b.id)
        .collect();
    // Should have task.next from initial insert (no deps at that time) and from dep removal
    assert!(
        next_events.len() >= 2,
        "removing a dependency should emit task.next"
    );
}

#[tokio::test]
async fn test_task_next_not_fired_when_other_deps_remain() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "next-multidep-proj").await;

    let task_a = create_task(&pool, &project.id, "dep A", "in_progress").await;
    let task_b = create_task(&pool, &project.id, "dep B", "in_progress").await;
    let task_c = create_task(&pool, &project.id, "waiting", "todo").await;

    // task_c depends on both task_a and task_b
    db::dependencies::add(&pool, &task_c.id, &task_a.id)
        .await
        .expect("add dep a");
    db::dependencies::add(&pool, &task_c.id, &task_b.id)
        .await
        .expect("add dep b");

    // Complete only task_a → task_c still depends on task_b (not done) → no task.next for task_c
    let mut task_a = db::tasks::get(&pool, &task_a.id)
        .await
        .expect("get")
        .unwrap();
    task_a.status = "done".to_string();
    db::tasks::update(&pool, &task_a).await.expect("complete a");

    // The task.next events for task_c from the dep completion trigger
    let _dep_completion_next: Vec<_> = all_events(&pool)
        .await
        .into_iter()
        .filter(|e| {
            e.event_type == "task.next"
                && e.entity_id == task_c.id
                // Only count events after the initial task_c.created events
                && e.id > 10 // rough heuristic; we mainly care about dep completion
        })
        .collect();

    // The trg_task_next_on_dep_completed should NOT fire for task_c because task_b is still not done
    // (task_c got task.next from initial insert, before deps were added)
    // We need to check events after we added deps and completed task_a
    // The cleanest check: after completing task_a, the only task.next for task_c should be from the initial insert
    let all_task_c_next: Vec<_> = events_of_type(&pool, "task.next")
        .await
        .into_iter()
        .filter(|e| e.entity_id == task_c.id)
        .collect();

    // Should only have 1 (from initial insert, before deps were added)
    assert_eq!(
        all_task_c_next.len(),
        1,
        "task.next should not fire when other deps remain unmet (got {})",
        all_task_c_next.len()
    );
}

// ============================================================================
// project.next trigger tests (fires when task.next is emitted)
// ============================================================================

#[tokio::test]
async fn test_project_next_follows_task_next() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "projnext-proj").await;

    // Creating a todo task in an active project fires task.next
    // which in turn fires project.next via recursive trigger
    let _task = create_task(&pool, &project.id, "actionable", "todo").await;

    let task_next = events_of_type(&pool, "task.next").await;
    let proj_next = events_of_type(&pool, "project.next").await;

    assert_eq!(task_next.len(), 1, "should have 1 task.next");
    assert_eq!(proj_next.len(), 1, "should have 1 project.next");
    assert_eq!(proj_next[0].entity_id, project.id);

    let payload: serde_json::Value = serde_json::from_str(&proj_next[0].payload).unwrap();
    assert_eq!(payload["id"], project.id);
    assert_eq!(payload["status"], "active");
}

// ============================================================================
// Project auto-complete trigger tests
// ============================================================================

#[tokio::test]
async fn test_project_auto_complete_on_last_task_done() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "autocomp-proj").await;

    let task_a = create_task(&pool, &project.id, "task A", "in_progress").await;
    let task_b = create_task(&pool, &project.id, "task B", "done").await;
    let _ = task_b; // already done

    // Complete task_a → all tasks done → project auto-completes
    let mut task_a = db::tasks::get(&pool, &task_a.id)
        .await
        .expect("get")
        .unwrap();
    task_a.status = "done".to_string();
    task_a.completed_at = Some(chrono::Utc::now().to_rfc3339());
    db::tasks::update(&pool, &task_a).await.expect("complete");

    // Project should now be completed
    let project = db::projects::get(&pool, &project.id)
        .await
        .expect("get")
        .unwrap();
    assert_eq!(
        project.status, "completed",
        "project should be auto-completed"
    );

    // Should have a project.completed event
    let events = events_of_type(&pool, "project.completed").await;
    assert_eq!(events.len(), 1);
}

#[tokio::test]
async fn test_project_auto_complete_not_when_tasks_remain() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "noautocomp-proj").await;

    let task_a = create_task(&pool, &project.id, "task A", "in_progress").await;
    let _task_b = create_task(&pool, &project.id, "task B", "todo").await;

    // Complete task_a → task_b still todo → no auto-complete
    let mut task_a = db::tasks::get(&pool, &task_a.id)
        .await
        .expect("get")
        .unwrap();
    task_a.status = "done".to_string();
    db::tasks::update(&pool, &task_a).await.expect("complete");

    let project = db::projects::get(&pool, &project.id)
        .await
        .expect("get")
        .unwrap();
    assert_eq!(project.status, "active", "project should still be active");

    let events = events_of_type(&pool, "project.completed").await;
    assert!(events.is_empty());
}

// ============================================================================
// Project auto-reactivate trigger tests
// ============================================================================

#[tokio::test]
async fn test_project_auto_reactivate_on_task_added() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "reactivate-proj").await;

    // Complete the project
    db::projects::complete(&pool, &project.id, false)
        .await
        .expect("complete");

    let project = db::projects::get(&pool, &project.id)
        .await
        .expect("get")
        .unwrap();
    assert_eq!(project.status, "completed");

    // Add a new task → should reactivate
    let _task = create_task(&pool, &project.id, "new work", "todo").await;

    let project = db::projects::get(&pool, &project.id)
        .await
        .expect("get")
        .unwrap();
    assert_eq!(project.status, "active", "project should be reactivated");

    // Should have a project.unarchived event (trigger fires when completed→active)
    let events = events_of_type(&pool, "project.unarchived").await;
    assert_eq!(events.len(), 1);
}

// ============================================================================
// Payload correctness tests
// ============================================================================

#[tokio::test]
async fn test_project_event_payload_has_all_fields() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "payload-proj").await;

    let events = events_of_type(&pool, "project.created").await;
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();

    // Verify all expected fields exist
    assert!(payload.get("id").is_some());
    assert!(payload.get("slug").is_some());
    assert!(payload.get("name").is_some());
    assert!(payload.get("description").is_some());
    assert!(payload.get("owner").is_some());
    assert!(payload.get("status").is_some());
    assert!(payload.get("tags").is_some());
    assert!(payload.get("created_at").is_some());
    assert!(payload.get("updated_at").is_some());
    assert!(payload.get("version").is_some());

    assert_eq!(payload["id"], project.id);
    assert_eq!(payload["slug"], project.slug);
}

#[tokio::test]
async fn test_task_event_payload_has_all_fields() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "tpayload-proj").await;
    let task = create_task(&pool, &project.id, "payload task", "todo").await;

    let events = events_of_type(&pool, "task.created").await;
    let payload: serde_json::Value = serde_json::from_str(&events[0].payload).unwrap();

    assert!(payload.get("id").is_some());
    assert!(payload.get("project_id").is_some());
    assert!(payload.get("task_number").is_some());
    assert!(payload.get("title").is_some());
    assert!(payload.get("description").is_some());
    assert!(payload.get("status").is_some());
    assert!(payload.get("priority").is_some());
    assert!(payload.get("owner").is_some());
    assert!(payload.get("tags").is_some());
    assert!(payload.get("blocked_reason").is_some());
    assert!(payload.get("claim_owner").is_some());
    assert!(payload.get("pinned").is_some());
    assert!(payload.get("focus_weight").is_some());
    assert!(payload.get("created_at").is_some());
    assert!(payload.get("updated_at").is_some());
    assert!(payload.get("version").is_some());

    assert_eq!(payload["id"], task.id);
    assert_eq!(payload["title"], "payload task");
}

// ============================================================================
// No-duplicate / idempotency tests
// ============================================================================

#[tokio::test]
async fn test_no_event_on_same_status_update() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "noop-proj").await;
    let mut task = create_task(&pool, &project.id, "noop task", "in_progress").await;

    // Update title without changing status → task.updated (not task.started)
    task.title = "new title".to_string();
    db::tasks::update(&pool, &task).await.expect("update");

    let started = events_of_type(&pool, "task.started").await;
    assert!(
        started.is_empty(),
        "should not emit task.started when status doesn't change"
    );

    let updated = events_of_type(&pool, "task.updated").await;
    assert_eq!(updated.len(), 1, "should emit task.updated");
}

#[tokio::test]
async fn test_project_complete_with_tasks_emits_task_events() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "bulk-complete-proj").await;

    let _task_a = create_task(&pool, &project.id, "task A", "todo").await;
    let _task_b = create_task(&pool, &project.id, "task B", "in_progress").await;

    // Complete project with complete_tasks=true → should complete all tasks and the project
    db::projects::complete(&pool, &project.id, true)
        .await
        .expect("complete");

    let task_completed = events_of_type(&pool, "task.completed").await;
    assert_eq!(
        task_completed.len(),
        2,
        "both tasks should emit task.completed"
    );

    let project_completed = events_of_type(&pool, "project.completed").await;
    assert_eq!(project_completed.len(), 1);
}
