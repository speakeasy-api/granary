//! Tests for worker-related functionality.
//!
//! These tests cover the Worker model, WorkerStatus, and worker lifecycle.

#[cfg(test)]
mod tests {
    use crate::models::{CreateWorker, UpdateWorkerStatus, Worker, WorkerStatus};

    // ==========================================
    // WorkerStatus Tests
    // ==========================================

    #[test]
    fn test_worker_status_as_str() {
        assert_eq!(WorkerStatus::Pending.as_str(), "pending");
        assert_eq!(WorkerStatus::Running.as_str(), "running");
        assert_eq!(WorkerStatus::Stopped.as_str(), "stopped");
        assert_eq!(WorkerStatus::Error.as_str(), "error");
    }

    #[test]
    fn test_worker_status_from_str() {
        assert_eq!(
            "pending".parse::<WorkerStatus>().unwrap(),
            WorkerStatus::Pending
        );
        assert_eq!(
            "running".parse::<WorkerStatus>().unwrap(),
            WorkerStatus::Running
        );
        assert_eq!(
            "stopped".parse::<WorkerStatus>().unwrap(),
            WorkerStatus::Stopped
        );
        assert_eq!(
            "error".parse::<WorkerStatus>().unwrap(),
            WorkerStatus::Error
        );
    }

    #[test]
    fn test_worker_status_from_str_case_insensitive() {
        assert_eq!(
            "RUNNING".parse::<WorkerStatus>().unwrap(),
            WorkerStatus::Running
        );
        assert_eq!(
            "Running".parse::<WorkerStatus>().unwrap(),
            WorkerStatus::Running
        );
        assert_eq!(
            "STOPPED".parse::<WorkerStatus>().unwrap(),
            WorkerStatus::Stopped
        );
    }

    #[test]
    fn test_worker_status_from_str_invalid() {
        assert!("invalid".parse::<WorkerStatus>().is_err());
        assert!("".parse::<WorkerStatus>().is_err());
    }

    #[test]
    fn test_worker_status_display() {
        assert_eq!(format!("{}", WorkerStatus::Running), "running");
        assert_eq!(format!("{}", WorkerStatus::Error), "error");
    }

    #[test]
    fn test_worker_status_default() {
        let status: WorkerStatus = Default::default();
        assert_eq!(status, WorkerStatus::Pending);
    }

    // ==========================================
    // Worker Model Tests
    // ==========================================

    fn create_test_worker() -> Worker {
        Worker {
            id: "worker-abc12345".to_string(),
            runner_name: Some("claude".to_string()),
            command: "claude".to_string(),
            args: r#"["--print", "--message", "test"]"#.to_string(),
            event_type: "task.unblocked".to_string(),
            filters: r#"["status!=draft"]"#.to_string(),
            concurrency: 2,
            instance_path: "/path/to/workspace".to_string(),
            status: "running".to_string(),
            error_message: None,
            pid: Some(12345),
            detached: false,
            created_at: "2026-01-15T10:00:00Z".to_string(),
            updated_at: "2026-01-15T10:30:00Z".to_string(),
            stopped_at: None,
            last_event_id: 100,
        }
    }

    #[test]
    fn test_worker_status_enum() {
        let mut worker = create_test_worker();

        worker.status = "running".to_string();
        assert_eq!(worker.status_enum(), WorkerStatus::Running);

        worker.status = "stopped".to_string();
        assert_eq!(worker.status_enum(), WorkerStatus::Stopped);

        worker.status = "error".to_string();
        assert_eq!(worker.status_enum(), WorkerStatus::Error);

        // Invalid status falls back to default (Pending)
        worker.status = "invalid".to_string();
        assert_eq!(worker.status_enum(), WorkerStatus::Pending);
    }

    #[test]
    fn test_worker_args_vec() {
        let worker = create_test_worker();
        let args = worker.args_vec();

        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "--print");
        assert_eq!(args[1], "--message");
        assert_eq!(args[2], "test");
    }

    #[test]
    fn test_worker_args_vec_empty() {
        let mut worker = create_test_worker();
        worker.args = "[]".to_string();

        let args = worker.args_vec();
        assert!(args.is_empty());
    }

    #[test]
    fn test_worker_args_vec_invalid_json() {
        let mut worker = create_test_worker();
        worker.args = "not json".to_string();

        let args = worker.args_vec();
        assert!(args.is_empty()); // Falls back to empty vec
    }

    #[test]
    fn test_worker_filters_vec() {
        let worker = create_test_worker();
        let filters = worker.filters_vec();

        assert_eq!(filters.len(), 1);
        assert_eq!(filters[0], "status!=draft");
    }

    #[test]
    fn test_worker_filters_vec_multiple() {
        let mut worker = create_test_worker();
        worker.filters = r#"["status=todo", "priority!=P4"]"#.to_string();

        let filters = worker.filters_vec();
        assert_eq!(filters.len(), 2);
        assert_eq!(filters[0], "status=todo");
        assert_eq!(filters[1], "priority!=P4");
    }

    #[test]
    fn test_worker_filters_vec_empty() {
        let mut worker = create_test_worker();
        worker.filters = "[]".to_string();

        let filters = worker.filters_vec();
        assert!(filters.is_empty());
    }

    #[test]
    fn test_worker_is_running() {
        let mut worker = create_test_worker();

        worker.status = "running".to_string();
        assert!(worker.is_running());

        worker.status = "pending".to_string();
        assert!(!worker.is_running());

        worker.status = "stopped".to_string();
        assert!(!worker.is_running());
    }

    #[test]
    fn test_worker_is_stopped() {
        let mut worker = create_test_worker();

        worker.status = "stopped".to_string();
        assert!(worker.is_stopped());

        worker.status = "error".to_string();
        assert!(worker.is_stopped());

        worker.status = "running".to_string();
        assert!(!worker.is_stopped());

        worker.status = "pending".to_string();
        assert!(!worker.is_stopped());
    }

    // ==========================================
    // CreateWorker Tests
    // ==========================================

    #[test]
    fn test_create_worker_default() {
        let create = CreateWorker::default();

        assert!(create.runner_name.is_none());
        assert!(create.command.is_empty());
        assert!(create.args.is_empty());
        assert!(create.event_type.is_empty());
        assert!(create.filters.is_empty());
        assert_eq!(create.concurrency, 1);
        assert!(create.instance_path.is_empty());
        assert!(!create.detached);
    }

    #[test]
    fn test_create_worker_with_values() {
        let create = CreateWorker {
            runner_name: Some("claude".to_string()),
            command: "claude".to_string(),
            args: vec!["--print".to_string(), "{task.id}".to_string()],
            event_type: "task.unblocked".to_string(),
            filters: vec!["priority!=P4".to_string()],
            concurrency: 4,
            instance_path: "/home/user/project".to_string(),
            detached: true,
            since: None,
        };

        assert_eq!(create.runner_name, Some("claude".to_string()));
        assert_eq!(create.command, "claude");
        assert_eq!(create.args.len(), 2);
        assert_eq!(create.event_type, "task.unblocked");
        assert_eq!(create.filters.len(), 1);
        assert_eq!(create.concurrency, 4);
        assert!(create.detached);
    }

    // ==========================================
    // UpdateWorkerStatus Tests
    // ==========================================

    #[test]
    fn test_update_worker_status_to_running() {
        let update = UpdateWorkerStatus {
            status: WorkerStatus::Running,
            error_message: None,
            pid: Some(12345),
        };

        assert_eq!(update.status, WorkerStatus::Running);
        assert!(update.error_message.is_none());
        assert_eq!(update.pid, Some(12345));
    }

    #[test]
    fn test_update_worker_status_to_error() {
        let update = UpdateWorkerStatus {
            status: WorkerStatus::Error,
            error_message: Some("Workspace not found".to_string()),
            pid: None,
        };

        assert_eq!(update.status, WorkerStatus::Error);
        assert_eq!(
            update.error_message,
            Some("Workspace not found".to_string())
        );
        assert!(update.pid.is_none());
    }

    #[test]
    fn test_update_worker_status_to_stopped() {
        let update = UpdateWorkerStatus {
            status: WorkerStatus::Stopped,
            error_message: None,
            pid: None,
        };

        assert_eq!(update.status, WorkerStatus::Stopped);
    }

    // ==========================================
    // Worker Lifecycle Scenarios
    // ==========================================

    #[test]
    fn test_worker_lifecycle_pending_to_running() {
        let mut worker = create_test_worker();
        worker.status = "pending".to_string();
        worker.pid = None;

        assert!(!worker.is_running());
        assert!(!worker.is_stopped());

        // Simulate start
        worker.status = "running".to_string();
        worker.pid = Some(54321);

        assert!(worker.is_running());
        assert!(!worker.is_stopped());
    }

    #[test]
    fn test_worker_lifecycle_running_to_stopped() {
        let mut worker = create_test_worker();
        worker.status = "running".to_string();

        assert!(worker.is_running());

        // Simulate stop
        worker.status = "stopped".to_string();
        worker.stopped_at = Some("2026-01-15T11:00:00Z".to_string());
        worker.pid = None;

        assert!(!worker.is_running());
        assert!(worker.is_stopped());
    }

    #[test]
    fn test_worker_lifecycle_running_to_error() {
        let mut worker = create_test_worker();
        worker.status = "running".to_string();

        // Simulate error (e.g., workspace deleted)
        worker.status = "error".to_string();
        worker.error_message = Some("Workspace no longer exists".to_string());
        worker.pid = None;

        assert!(!worker.is_running());
        assert!(worker.is_stopped());
        assert!(worker.error_message.is_some());
    }

    // ==========================================
    // Worker Configuration Scenarios
    // ==========================================

    #[test]
    fn test_worker_with_runner_config() {
        let create = CreateWorker {
            runner_name: Some("claude".to_string()),
            command: "claude".to_string(),
            args: vec![
                "--print".to_string(),
                "--message".to_string(),
                "Execute task {task.id}".to_string(),
            ],
            event_type: "task.unblocked".to_string(),
            filters: vec!["task.priority=P0".to_string()],
            concurrency: 2,
            instance_path: "/projects/myapp".to_string(),
            detached: false,
            since: None,
        };

        assert!(create.runner_name.is_some());
        assert_eq!(create.concurrency, 2);
    }

    #[test]
    fn test_worker_with_inline_command() {
        let create = CreateWorker {
            runner_name: None,
            command: "echo".to_string(),
            args: vec!["Task {task.id} unblocked!".to_string()],
            event_type: "task.unblocked".to_string(),
            filters: vec![],
            concurrency: 1,
            instance_path: "/projects/myapp".to_string(),
            detached: false,
            since: None,
        };

        assert!(create.runner_name.is_none());
        assert_eq!(create.command, "echo");
    }

    #[test]
    fn test_worker_with_multiple_filters() {
        let create = CreateWorker {
            runner_name: Some("notifier".to_string()),
            command: "slack-notify".to_string(),
            args: vec!["{task.title}".to_string()],
            event_type: "task.done".to_string(),
            filters: vec![
                "task.priority=P0".to_string(),
                "task.owner!=".to_string(),
                "project.name~=api".to_string(),
            ],
            concurrency: 10,
            instance_path: "/projects/backend".to_string(),
            detached: true,
            since: None,
        };

        assert_eq!(create.filters.len(), 3);
        assert!(create.detached);
        assert_eq!(create.concurrency, 10);
    }
}
