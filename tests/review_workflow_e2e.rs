use granary::db;
use granary::db::connection;
use granary::error::GranaryError;
use granary::models::*;
use granary::services;

async fn setup_pool() -> sqlx::SqlitePool {
    let pool = connection::create_pool(std::path::Path::new(":memory:"))
        .await
        .expect("create_pool");
    connection::run_migrations(&pool).await.expect("migrations");
    pool
}

async fn create_project(pool: &sqlx::SqlitePool, name: &str, status: &str) -> Project {
    let id = generate_project_id(name);
    let slug = normalize_slug(name);
    let now = chrono::Utc::now().to_rfc3339();
    let project = Project {
        id: id.clone(),
        slug,
        name: name.to_string(),
        description: None,
        owner: Some("reviewer".to_string()),
        status: status.to_string(),
        tags: None,
        default_session_policy: None,
        steering_refs: None,
        created_at: now.clone(),
        updated_at: now,
        metadata: None,
        version: 1,
        last_edited_by: Some("reviewer".to_string()),
    };
    db::projects::create(pool, &project)
        .await
        .expect("create project");
    project
}

async fn create_task(pool: &sqlx::SqlitePool, project_id: &str, title: &str, status: &str) -> Task {
    let scope = format!("project:{}:task", project_id);
    let task_number = db::counters::next(pool, &scope).await.expect("counter");
    let id = generate_task_id(project_id, task_number);
    let now = chrono::Utc::now().to_rfc3339();
    let task = Task {
        id: id.clone(),
        project_id: project_id.to_string(),
        task_number,
        parent_task_id: None,
        title: title.to_string(),
        description: None,
        status: status.to_string(),
        priority: "P2".to_string(),
        owner: Some("reviewer".to_string()),
        tags: None,
        worker_ids: None,
        run_ids: None,
        blocked_reason: None,
        started_at: None,
        completed_at: None,
        due_at: None,
        claim_owner: None,
        claim_claimed_at: None,
        claim_lease_expires_at: None,
        pinned: 0,
        focus_weight: 0,
        metadata: None,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
        last_edited_by: Some("reviewer".to_string()),
    };
    db::tasks::create(pool, &task).await.expect("create task");
    task
}

#[tokio::test]
async fn start_task_rejects_in_review_tasks() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "start-review-guard", "active").await;
    let task = create_task(&pool, &project.id, "Needs review", "in_review").await;

    let err = services::start_task(&pool, &task.id, Some("worker".to_string()))
        .await
        .expect_err("in-review task should not be startable");

    match err {
        GranaryError::Conflict(message) => {
            assert!(
                message.contains("in review"),
                "expected in-review conflict message, got: {message}"
            );
        }
        other => panic!("expected Conflict error, got {other:?}"),
    }

    let task_after = services::get_task(&pool, &task.id).await.expect("get task");
    assert_eq!(task_after.status, "in_review");
}

#[tokio::test]
async fn reject_project_rolls_back_if_review_comment_insert_fails() {
    let pool = setup_pool().await;
    let project = create_project(&pool, "reject-project-tx", "in_review").await;
    let task = create_task(&pool, &project.id, "Draft task", "draft").await;

    // Force the next project review comment insert to collide on comment id.
    let now = chrono::Utc::now().to_rfc3339();
    let conflicting_comment = Comment {
        id: generate_comment_id(&project.id, 1),
        parent_type: "project".to_string(),
        parent_id: project.id.clone(),
        comment_number: 1,
        kind: CommentKind::Review.as_str().to_string(),
        content: "existing comment".to_string(),
        author: Some("reviewer".to_string()),
        meta: None,
        created_at: now.clone(),
        updated_at: now,
        version: 1,
    };
    db::comments::create(&pool, &conflicting_comment)
        .await
        .expect("insert conflicting comment");

    let err = services::reject_project(&pool, &project.id, "needs changes")
        .await
        .expect_err("reject should fail because comment id conflicts");
    assert!(
        matches!(err, GranaryError::Database(_)),
        "expected database error from forced comment-id conflict, got {err:?}"
    );

    let project_after = services::get_project(&pool, &project.id)
        .await
        .expect("get project");
    assert_eq!(
        project_after.status, "in_review",
        "project status should roll back on failure"
    );

    let task_after = services::get_task(&pool, &task.id).await.expect("get task");
    assert_eq!(
        task_after.status, "draft",
        "draft task status should roll back on failure"
    );
}
