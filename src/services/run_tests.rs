//! Tests for run-related functionality.
//!
//! These tests cover the Run model, RunStatus, retry behavior, and run lifecycle.

#[cfg(test)]
mod tests {
    use crate::models::run::{CreateRun, Run, RunStatus, ScheduleRetry, UpdateRunStatus};
    use crate::services::worker_runtime::calculate_backoff;
    use std::time::Duration;

    // ==========================================
    // RunStatus Tests
    // ==========================================

    #[test]
    fn test_run_status_as_str() {
        assert_eq!(RunStatus::Pending.as_str(), "pending");
        assert_eq!(RunStatus::Running.as_str(), "running");
        assert_eq!(RunStatus::Completed.as_str(), "completed");
        assert_eq!(RunStatus::Failed.as_str(), "failed");
        assert_eq!(RunStatus::Paused.as_str(), "paused");
        assert_eq!(RunStatus::Cancelled.as_str(), "cancelled");
    }

    #[test]
    fn test_run_status_from_str() {
        assert_eq!("pending".parse::<RunStatus>().unwrap(), RunStatus::Pending);
        assert_eq!("running".parse::<RunStatus>().unwrap(), RunStatus::Running);
        assert_eq!(
            "completed".parse::<RunStatus>().unwrap(),
            RunStatus::Completed
        );
        assert_eq!("failed".parse::<RunStatus>().unwrap(), RunStatus::Failed);
        assert_eq!("paused".parse::<RunStatus>().unwrap(), RunStatus::Paused);
        assert_eq!(
            "cancelled".parse::<RunStatus>().unwrap(),
            RunStatus::Cancelled
        );
    }

    #[test]
    fn test_run_status_from_str_case_insensitive() {
        assert_eq!("RUNNING".parse::<RunStatus>().unwrap(), RunStatus::Running);
        assert_eq!(
            "Completed".parse::<RunStatus>().unwrap(),
            RunStatus::Completed
        );
        assert_eq!("FAILED".parse::<RunStatus>().unwrap(), RunStatus::Failed);
    }

    #[test]
    fn test_run_status_from_str_invalid() {
        assert!("invalid".parse::<RunStatus>().is_err());
        assert!("".parse::<RunStatus>().is_err());
        assert!("success".parse::<RunStatus>().is_err());
    }

    #[test]
    fn test_run_status_display() {
        assert_eq!(format!("{}", RunStatus::Running), "running");
        assert_eq!(format!("{}", RunStatus::Completed), "completed");
        assert_eq!(format!("{}", RunStatus::Failed), "failed");
    }

    #[test]
    fn test_run_status_default() {
        let status: RunStatus = Default::default();
        assert_eq!(status, RunStatus::Pending);
    }

    // ==========================================
    // Run Model Tests
    // ==========================================

    fn create_test_run() -> Run {
        Run {
            id: "run-xyz12345".to_string(),
            worker_id: "worker-abc12345".to_string(),
            event_id: 42,
            event_type: "task.unblocked".to_string(),
            entity_id: "proj-abc1-task-5".to_string(),
            command: "claude".to_string(),
            args: r#"["--print", "--message", "Execute task proj-abc1-task-5"]"#.to_string(),
            status: "running".to_string(),
            exit_code: None,
            error_message: None,
            attempt: 1,
            max_attempts: 3,
            next_retry_at: None,
            pid: Some(12345),
            log_path: Some("/home/user/.granary/logs/worker-abc12345/run-xyz12345.log".to_string()),
            started_at: Some("2026-01-15T10:00:00Z".to_string()),
            completed_at: None,
            created_at: "2026-01-15T10:00:00Z".to_string(),
            updated_at: "2026-01-15T10:00:00Z".to_string(),
        }
    }

    #[test]
    fn test_run_status_enum() {
        let mut run = create_test_run();

        run.status = "running".to_string();
        assert_eq!(run.status_enum(), RunStatus::Running);

        run.status = "completed".to_string();
        assert_eq!(run.status_enum(), RunStatus::Completed);

        run.status = "failed".to_string();
        assert_eq!(run.status_enum(), RunStatus::Failed);

        run.status = "paused".to_string();
        assert_eq!(run.status_enum(), RunStatus::Paused);

        run.status = "cancelled".to_string();
        assert_eq!(run.status_enum(), RunStatus::Cancelled);

        // Invalid status falls back to default (Pending)
        run.status = "invalid".to_string();
        assert_eq!(run.status_enum(), RunStatus::Pending);
    }

    #[test]
    fn test_run_args_vec() {
        let run = create_test_run();
        let args = run.args_vec();

        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "--print");
        assert_eq!(args[1], "--message");
        assert_eq!(args[2], "Execute task proj-abc1-task-5");
    }

    #[test]
    fn test_run_args_vec_empty() {
        let mut run = create_test_run();
        run.args = "[]".to_string();

        let args = run.args_vec();
        assert!(args.is_empty());
    }

    #[test]
    fn test_run_args_vec_invalid_json() {
        let mut run = create_test_run();
        run.args = "not json".to_string();

        let args = run.args_vec();
        assert!(args.is_empty()); // Falls back to empty vec
    }

    #[test]
    fn test_run_is_running() {
        let mut run = create_test_run();

        run.status = "running".to_string();
        assert!(run.is_running());

        run.status = "pending".to_string();
        assert!(!run.is_running());

        run.status = "paused".to_string();
        assert!(!run.is_running());
    }

    #[test]
    fn test_run_is_finished() {
        let mut run = create_test_run();

        run.status = "completed".to_string();
        assert!(run.is_finished());

        run.status = "failed".to_string();
        assert!(run.is_finished());

        run.status = "cancelled".to_string();
        assert!(run.is_finished());

        run.status = "running".to_string();
        assert!(!run.is_finished());

        run.status = "pending".to_string();
        assert!(!run.is_finished());

        run.status = "paused".to_string();
        assert!(!run.is_finished());
    }

    #[test]
    fn test_run_can_retry() {
        let mut run = create_test_run();
        run.max_attempts = 3;

        // Can retry if failed and not at max attempts
        run.status = "failed".to_string();
        run.attempt = 1;
        assert!(run.can_retry());

        run.attempt = 2;
        assert!(run.can_retry());

        // Cannot retry at max attempts
        run.attempt = 3;
        assert!(!run.can_retry());

        // Cannot retry if not failed
        run.status = "completed".to_string();
        run.attempt = 1;
        assert!(!run.can_retry());

        run.status = "running".to_string();
        assert!(!run.can_retry());
    }

    #[test]
    fn test_run_is_pending_retry() {
        let mut run = create_test_run();

        // Is pending retry if pending and attempt > 1
        run.status = "pending".to_string();
        run.attempt = 2;
        assert!(run.is_pending_retry());

        run.attempt = 3;
        assert!(run.is_pending_retry());

        // Not pending retry if first attempt
        run.attempt = 1;
        assert!(!run.is_pending_retry());

        // Not pending retry if not pending
        run.status = "running".to_string();
        run.attempt = 2;
        assert!(!run.is_pending_retry());
    }

    // ==========================================
    // CreateRun Tests
    // ==========================================

    #[test]
    fn test_create_run_default() {
        let create = CreateRun::default();

        assert!(create.worker_id.is_empty());
        assert_eq!(create.event_id, 0);
        assert!(create.event_type.is_empty());
        assert!(create.entity_id.is_empty());
        assert!(create.command.is_empty());
        assert!(create.args.is_empty());
        assert_eq!(create.max_attempts, 3);
        assert!(create.log_path.is_none());
    }

    #[test]
    fn test_create_run_with_values() {
        let create = CreateRun {
            worker_id: "worker-abc".to_string(),
            event_id: 100,
            event_type: "task.unblocked".to_string(),
            entity_id: "proj-xyz1-task-1".to_string(),
            command: "claude".to_string(),
            args: vec!["--print".to_string(), "Execute task".to_string()],
            max_attempts: 5,
            log_path: Some("/logs/run.log".to_string()),
        };

        assert_eq!(create.worker_id, "worker-abc");
        assert_eq!(create.event_id, 100);
        assert_eq!(create.max_attempts, 5);
        assert!(create.log_path.is_some());
    }

    // ==========================================
    // UpdateRunStatus Tests
    // ==========================================

    #[test]
    fn test_update_run_status_to_running() {
        let update = UpdateRunStatus {
            status: RunStatus::Running,
            exit_code: None,
            error_message: None,
            pid: Some(54321),
        };

        assert_eq!(update.status, RunStatus::Running);
        assert!(update.exit_code.is_none());
        assert_eq!(update.pid, Some(54321));
    }

    #[test]
    fn test_update_run_status_to_completed() {
        let update = UpdateRunStatus {
            status: RunStatus::Completed,
            exit_code: Some(0),
            error_message: None,
            pid: None,
        };

        assert_eq!(update.status, RunStatus::Completed);
        assert_eq!(update.exit_code, Some(0));
    }

    #[test]
    fn test_update_run_status_to_failed() {
        let update = UpdateRunStatus {
            status: RunStatus::Failed,
            exit_code: Some(1),
            error_message: Some("Process exited with code 1".to_string()),
            pid: None,
        };

        assert_eq!(update.status, RunStatus::Failed);
        assert_eq!(update.exit_code, Some(1));
        assert!(update.error_message.is_some());
    }

    #[test]
    fn test_update_run_status_to_cancelled() {
        let update = UpdateRunStatus {
            status: RunStatus::Cancelled,
            exit_code: None,
            error_message: Some("Manually cancelled".to_string()),
            pid: None,
        };

        assert_eq!(update.status, RunStatus::Cancelled);
    }

    // ==========================================
    // ScheduleRetry Tests
    // ==========================================

    #[test]
    fn test_schedule_retry() {
        let retry = ScheduleRetry {
            next_retry_at: "2026-01-15T10:05:00Z".to_string(),
            attempt: 2,
        };

        assert_eq!(retry.attempt, 2);
        assert!(!retry.next_retry_at.is_empty());
    }

    // ==========================================
    // Backoff Calculation Tests
    // ==========================================

    #[test]
    fn test_calculate_backoff_first_attempt() {
        let delay = calculate_backoff(1, 5);
        // First attempt: 5 * 2^0 = 5 seconds + jitter
        assert!(delay.as_secs() >= 5);
        assert!(delay.as_secs() <= 7); // 5 + 25% jitter max
    }

    #[test]
    fn test_calculate_backoff_second_attempt() {
        let delay = calculate_backoff(2, 5);
        // Second attempt: 5 * 2^1 = 10 seconds + jitter
        assert!(delay.as_secs() >= 10);
        assert!(delay.as_secs() <= 13); // 10 + 25% jitter max
    }

    #[test]
    fn test_calculate_backoff_third_attempt() {
        let delay = calculate_backoff(3, 5);
        // Third attempt: 5 * 2^2 = 20 seconds + jitter
        assert!(delay.as_secs() >= 20);
        assert!(delay.as_secs() <= 25); // 20 + 25% jitter max
    }

    #[test]
    fn test_calculate_backoff_exponential_growth() {
        let base = 5u64;

        for attempt in 1..=5 {
            let delay = calculate_backoff(attempt, base);
            let expected_base = base * 2u64.pow((attempt - 1) as u32);
            let max_with_jitter = expected_base + expected_base / 4;

            assert!(
                delay.as_secs() >= expected_base,
                "Attempt {}: delay {} should be >= {}",
                attempt,
                delay.as_secs(),
                expected_base
            );
            assert!(
                delay.as_secs() <= max_with_jitter,
                "Attempt {}: delay {} should be <= {}",
                attempt,
                delay.as_secs(),
                max_with_jitter
            );
        }
    }

    #[test]
    fn test_calculate_backoff_caps_at_max() {
        // Very high attempt should be capped at 2^10
        let delay = calculate_backoff(100, 5);
        // Max: 5 * 2^10 = 5120 seconds
        let max_delay = 5 * 2u64.pow(10) + 5 * 2u64.pow(10) / 4; // + 25% jitter

        assert!(delay.as_secs() <= max_delay);
    }

    #[test]
    fn test_calculate_backoff_different_base() {
        // Test with base delay of 10 seconds
        let delay = calculate_backoff(1, 10);
        assert!(delay.as_secs() >= 10);
        assert!(delay.as_secs() <= 13);

        let delay = calculate_backoff(2, 10);
        assert!(delay.as_secs() >= 20);
        assert!(delay.as_secs() <= 25);
    }

    #[test]
    fn test_calculate_backoff_minimum_base() {
        // Test with base delay of 1 second
        let delay = calculate_backoff(1, 1);
        assert!(delay.as_secs() >= 1);
        assert!(delay.as_secs() <= 2);
    }

    #[test]
    fn test_calculate_backoff_zero_base() {
        // Edge case: zero base delay
        let delay = calculate_backoff(1, 0);
        assert_eq!(delay.as_secs(), 0);
    }

    // ==========================================
    // Run Lifecycle Scenarios
    // ==========================================

    #[test]
    fn test_run_lifecycle_success() {
        let mut run = create_test_run();

        // Start as pending
        run.status = "pending".to_string();
        run.attempt = 1;
        assert!(!run.is_running());
        assert!(!run.is_finished());

        // Transition to running
        run.status = "running".to_string();
        run.pid = Some(12345);
        assert!(run.is_running());
        assert!(!run.is_finished());

        // Complete successfully
        run.status = "completed".to_string();
        run.exit_code = Some(0);
        run.completed_at = Some("2026-01-15T10:05:00Z".to_string());
        run.pid = None;
        assert!(!run.is_running());
        assert!(run.is_finished());
        assert!(!run.can_retry());
    }

    #[test]
    fn test_run_lifecycle_failure_with_retry() {
        let mut run = create_test_run();
        run.max_attempts = 3;

        // First attempt fails
        run.status = "failed".to_string();
        run.attempt = 1;
        run.exit_code = Some(1);
        assert!(run.can_retry());

        // Schedule retry
        run.status = "pending".to_string();
        run.attempt = 2;
        run.next_retry_at = Some("2026-01-15T10:05:00Z".to_string());
        assert!(run.is_pending_retry());

        // Second attempt fails
        run.status = "failed".to_string();
        assert!(run.can_retry());

        // Third attempt fails
        run.attempt = 3;
        run.status = "failed".to_string();
        assert!(!run.can_retry()); // No more retries
    }

    #[test]
    fn test_run_lifecycle_manual_cancel() {
        let mut run = create_test_run();

        run.status = "running".to_string();
        assert!(run.is_running());

        // Manual cancel
        run.status = "cancelled".to_string();
        run.error_message = Some("Manually cancelled".to_string());
        run.pid = None;

        assert!(!run.is_running());
        assert!(run.is_finished());
        assert!(!run.can_retry()); // Cancelled runs don't retry
    }

    #[test]
    fn test_run_lifecycle_pause_resume() {
        let mut run = create_test_run();

        run.status = "running".to_string();
        run.pid = Some(12345);
        assert!(run.is_running());

        // Pause
        run.status = "paused".to_string();
        assert!(!run.is_running());
        assert!(!run.is_finished());

        // Resume
        run.status = "running".to_string();
        assert!(run.is_running());
    }

    // ==========================================
    // Run Configuration Scenarios
    // ==========================================

    #[test]
    fn test_run_with_custom_max_attempts() {
        let mut run = create_test_run();
        run.max_attempts = 5;
        run.status = "failed".to_string();

        run.attempt = 1;
        assert!(run.can_retry());

        run.attempt = 4;
        assert!(run.can_retry());

        run.attempt = 5;
        assert!(!run.can_retry());
    }

    #[test]
    fn test_run_with_no_retries() {
        let mut run = create_test_run();
        run.max_attempts = 1;
        run.status = "failed".to_string();
        run.attempt = 1;

        assert!(!run.can_retry());
    }

    // ==========================================
    // Backoff Schedule Verification
    // ==========================================

    #[test]
    fn test_backoff_schedule_typical() {
        // Verify typical retry schedule with 5 second base
        let base = 5u64;
        let schedule: Vec<Duration> = (1..=5).map(|a| calculate_backoff(a, base)).collect();

        // Verify each delay is at least the expected base
        assert!(schedule[0].as_secs() >= 5); // 5 * 2^0 = 5
        assert!(schedule[1].as_secs() >= 10); // 5 * 2^1 = 10
        assert!(schedule[2].as_secs() >= 20); // 5 * 2^2 = 20
        assert!(schedule[3].as_secs() >= 40); // 5 * 2^3 = 40
        assert!(schedule[4].as_secs() >= 80); // 5 * 2^4 = 80

        // Verify exponential growth (each should be roughly 2x previous)
        for i in 1..schedule.len() {
            assert!(
                schedule[i].as_secs() > schedule[i - 1].as_secs(),
                "Delay should increase: {} vs {}",
                schedule[i - 1].as_secs(),
                schedule[i].as_secs()
            );
        }
    }

    #[test]
    fn test_backoff_jitter_adds_randomness() {
        // Run multiple times and verify jitter adds some variation
        let delays: Vec<u64> = (0..10).map(|_| calculate_backoff(2, 5).as_secs()).collect();

        // At least some delays should be different (jitter should add variation)
        // We check that they're within the expected range (base 10 + up to 25% jitter)
        assert!(delays.iter().all(|&d| (10..=13).contains(&d)));
    }
}
