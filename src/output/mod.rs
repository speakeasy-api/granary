pub mod json;
pub mod prompt;
pub mod table;
pub mod types;

use crate::cli::args::CliOutputFormat;
pub use types::OutputType;

/// Trait for command outputs that can be formatted in multiple ways
pub trait Output: Sized {
    /// The preferred output type for this output (can be overridden by --format)
    fn output_type() -> OutputType {
        OutputType::Text
    }

    /// Format as JSON
    fn to_json(&self) -> String;

    /// Format as prompt (LLM-optimized)
    fn to_prompt(&self) -> String;

    /// Format as text (human-readable table/text)
    fn to_text(&self) -> String;

    /// Format according to the given output format, falling back to preferred type
    fn format(&self, format: Option<CliOutputFormat>) -> String {
        match format.unwrap_or_else(|| Self::output_type().into()) {
            CliOutputFormat::Json => self.to_json(),
            CliOutputFormat::Prompt => self.to_prompt(),
            _ => self.to_text(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::CliOutputFormat;
    use crate::models::run::Run;
    use crate::models::*;

    // =========================================================================
    // Test helpers - construct minimal valid instances
    // =========================================================================

    fn make_task(id: &str, title: &str) -> Task {
        Task {
            id: id.to_string(),
            project_id: "proj-test".to_string(),
            task_number: 1,
            parent_task_id: None,
            title: title.to_string(),
            description: None,
            status: "todo".to_string(),
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
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            version: 1,
            last_edited_by: None,
        }
    }

    fn make_project(id: &str, name: &str) -> Project {
        Project {
            id: id.to_string(),
            slug: id.to_string(),
            name: name.to_string(),
            description: None,
            owner: None,
            status: "active".to_string(),
            tags: None,
            default_session_policy: None,
            steering_refs: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            version: 1,
            last_edited_by: None,
        }
    }

    fn make_run(id: &str) -> Run {
        Run {
            id: id.to_string(),
            worker_id: "worker-1".to_string(),
            event_id: 1,
            event_type: "task.created".to_string(),
            entity_id: "task-1".to_string(),
            command: "test".to_string(),
            args: "".to_string(),
            status: "running".to_string(),
            exit_code: None,
            error_message: None,
            attempt: 1,
            max_attempts: 3,
            next_retry_at: None,
            pid: None,
            log_path: None,
            started_at: Some("2025-01-01T00:00:00Z".to_string()),
            completed_at: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_comment(id: &str) -> Comment {
        Comment {
            id: id.to_string(),
            parent_type: "task".to_string(),
            parent_id: "task-1".to_string(),
            comment_number: 1,
            kind: "note".to_string(),
            content: "Test comment".to_string(),
            author: Some("tester".to_string()),
            meta: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            version: 1,
        }
    }

    fn make_session(id: &str) -> Session {
        Session {
            id: id.to_string(),
            name: Some("test-session".to_string()),
            owner: None,
            mode: None,
            focus_task_id: None,
            variables: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            closed_at: None,
            last_edited_by: None,
        }
    }

    fn make_checkpoint(id: &str) -> Checkpoint {
        Checkpoint {
            id: id.to_string(),
            session_id: "sess-1".to_string(),
            name: "checkpoint-1".to_string(),
            snapshot: "{}".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_artifact(id: &str) -> Artifact {
        Artifact {
            id: id.to_string(),
            parent_type: "task".to_string(),
            parent_id: "task-1".to_string(),
            artifact_number: 1,
            artifact_type: "file".to_string(),
            path_or_url: "/tmp/test.txt".to_string(),
            description: Some("Test artifact".to_string()),
            meta: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_worker(id: &str) -> Worker {
        Worker {
            id: id.to_string(),
            runner_name: None,
            command: "echo".to_string(),
            args: "[]".to_string(),
            event_type: "task.created".to_string(),
            filters: "[]".to_string(),
            concurrency: 1,
            instance_path: "/tmp/test".to_string(),
            status: "running".to_string(),
            error_message: None,
            pid: Some(1234),
            detached: false,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            stopped_at: None,
            last_event_id: 0,
        }
    }

    // =========================================================================
    // Output trait default type tests
    // =========================================================================

    mod default_output_types {
        use super::*;
        use crate::cli::batch::{BatchOutput, BatchStreamOutput};
        use crate::cli::checkpoints::{CheckpointOutput, CheckpointsOutput};
        use crate::cli::comments::{CommentOutput, CommentsOutput};
        use crate::cli::daemon::{
            DaemonLogsOutput, DaemonRestartOutput, DaemonStartOutput, DaemonStatusOutput,
            DaemonStopOutput,
        };
        use crate::cli::entrypoint::EntrypointOutput;
        use crate::cli::initiate::InitiateOutput;
        use crate::cli::initiatives::{
            InitiativeOutput as InitiativeShowOutput, InitiativeProjectsOutput,
            InitiativeSummaryOutput, InitiativeTaskOutput, InitiativeTasksOutput,
            InitiativesOutput,
        };
        use crate::cli::plan::{ExistingPlanOutput, PlanOutput};
        use crate::cli::projects::{ProjectOutput, ProjectTasksOutput, ProjectsOutput};
        use crate::cli::run::{
            RunOutput, RunPauseOutput, RunResumeOutput, RunStopOutput, RunsOutput,
        };
        use crate::cli::search::SearchOutput;
        use crate::cli::sessions::{SessionOutput, SessionsOutput};
        use crate::cli::show::{ArtifactOutput, ArtifactsOutput};
        use crate::cli::summary::{ContextOutput, HandoffOutput, SummaryOutput};
        use crate::cli::tasks::{NextTaskOutput, TaskCreatedOutput, TaskOutput, TasksOutput};
        use crate::cli::update::{UpdateCheckOutput, UpdateOutput};
        use crate::cli::work::{WorkBlockOutput, WorkDoneOutput, WorkOutput, WorkReleaseOutput};
        use crate::cli::worker::{WorkerPruneOutput, WorkerStatusOutput, WorkerStopOutput};
        use crate::cli::workers::{WorkerOutput, WorkersOutput};

        // Batch commands → Text (default)
        #[test]
        fn batch_output_defaults_to_text() {
            assert_eq!(BatchOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn batch_stream_output_defaults_to_text() {
            assert_eq!(BatchStreamOutput::output_type(), OutputType::Text);
        }

        // Entrypoint → Text (default)
        #[test]
        fn entrypoint_output_defaults_to_text() {
            assert_eq!(EntrypointOutput::output_type(), OutputType::Text);
        }

        // LLM-first commands → Prompt
        #[test]
        fn plan_output_defaults_to_prompt() {
            assert_eq!(PlanOutput::output_type(), OutputType::Prompt);
        }

        #[test]
        fn existing_plan_output_defaults_to_prompt() {
            assert_eq!(ExistingPlanOutput::output_type(), OutputType::Prompt);
        }

        #[test]
        fn work_output_defaults_to_prompt() {
            assert_eq!(WorkOutput::output_type(), OutputType::Prompt);
        }

        #[test]
        fn context_output_defaults_to_prompt() {
            assert_eq!(ContextOutput::output_type(), OutputType::Prompt);
        }

        #[test]
        fn handoff_output_defaults_to_prompt() {
            assert_eq!(HandoffOutput::output_type(), OutputType::Prompt);
        }

        #[test]
        fn initiate_output_defaults_to_prompt() {
            assert_eq!(InitiateOutput::output_type(), OutputType::Prompt);
        }

        // Simple status commands → Text
        #[test]
        fn work_done_output_defaults_to_text() {
            assert_eq!(WorkDoneOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn work_block_output_defaults_to_text() {
            assert_eq!(WorkBlockOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn work_release_output_defaults_to_text() {
            assert_eq!(WorkReleaseOutput::output_type(), OutputType::Text);
        }

        // Data listing commands → Text (trait default)
        #[test]
        fn tasks_output_defaults_to_text() {
            assert_eq!(TasksOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn task_output_defaults_to_text() {
            assert_eq!(TaskOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn next_task_output_defaults_to_text() {
            assert_eq!(NextTaskOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn task_created_output_defaults_to_text() {
            assert_eq!(TaskCreatedOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn runs_output_defaults_to_text() {
            assert_eq!(RunsOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn run_output_defaults_to_text() {
            assert_eq!(RunOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn search_output_defaults_to_text() {
            assert_eq!(SearchOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn projects_output_defaults_to_text() {
            assert_eq!(ProjectsOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn project_output_defaults_to_text() {
            assert_eq!(ProjectOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn project_tasks_output_defaults_to_text() {
            assert_eq!(ProjectTasksOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn summary_output_defaults_to_text() {
            assert_eq!(SummaryOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn sessions_output_defaults_to_text() {
            assert_eq!(SessionsOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn session_output_defaults_to_text() {
            assert_eq!(SessionOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn checkpoints_output_defaults_to_text() {
            assert_eq!(CheckpointsOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn checkpoint_output_defaults_to_text() {
            assert_eq!(CheckpointOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn comments_output_defaults_to_text() {
            assert_eq!(CommentsOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn comment_output_defaults_to_text() {
            assert_eq!(CommentOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn artifacts_output_defaults_to_text() {
            assert_eq!(ArtifactsOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn artifact_output_defaults_to_text() {
            assert_eq!(ArtifactOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn workers_output_defaults_to_text() {
            assert_eq!(WorkersOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn worker_output_defaults_to_text() {
            assert_eq!(WorkerOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn initiatives_output_defaults_to_text() {
            assert_eq!(InitiativesOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn initiative_output_defaults_to_text() {
            assert_eq!(InitiativeShowOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn initiative_summary_output_defaults_to_text() {
            assert_eq!(InitiativeSummaryOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn initiative_projects_output_defaults_to_text() {
            assert_eq!(InitiativeProjectsOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn initiative_tasks_output_defaults_to_text() {
            assert_eq!(InitiativeTasksOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn initiative_task_output_defaults_to_text() {
            assert_eq!(InitiativeTaskOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn daemon_status_output_defaults_to_text() {
            assert_eq!(DaemonStatusOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn daemon_start_output_defaults_to_text() {
            assert_eq!(DaemonStartOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn daemon_stop_output_defaults_to_text() {
            assert_eq!(DaemonStopOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn daemon_restart_output_defaults_to_text() {
            assert_eq!(DaemonRestartOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn daemon_logs_output_defaults_to_text() {
            assert_eq!(DaemonLogsOutput::output_type(), OutputType::Text);
        }

        // Worker action outputs → Text (default)
        #[test]
        fn worker_stop_output_defaults_to_text() {
            assert_eq!(WorkerStopOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn worker_prune_output_defaults_to_text() {
            assert_eq!(WorkerPruneOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn worker_status_output_defaults_to_text() {
            assert_eq!(WorkerStatusOutput::output_type(), OutputType::Text);
        }

        // Run action outputs → Text (default)
        #[test]
        fn run_stop_output_defaults_to_text() {
            assert_eq!(RunStopOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn run_pause_output_defaults_to_text() {
            assert_eq!(RunPauseOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn run_resume_output_defaults_to_text() {
            assert_eq!(RunResumeOutput::output_type(), OutputType::Text);
        }

        // Update outputs → Text (default)
        #[test]
        fn update_check_output_defaults_to_text() {
            assert_eq!(UpdateCheckOutput::output_type(), OutputType::Text);
        }

        #[test]
        fn update_output_defaults_to_text() {
            assert_eq!(UpdateOutput::output_type(), OutputType::Text);
        }
    }

    // =========================================================================
    // JSON validity tests - verify to_json() produces valid JSON
    // =========================================================================

    mod json_validity {
        use super::*;
        use crate::cli::batch::{BatchOutput, BatchStreamOutput};
        use crate::cli::checkpoints::{CheckpointOutput, CheckpointsOutput};
        use crate::cli::comments::{CommentOutput, CommentsOutput};
        use crate::cli::daemon::{
            DaemonLogsOutput, DaemonRestartOutput, DaemonStartOutput, DaemonStatusOutput,
            DaemonStopOutput,
        };
        use crate::cli::entrypoint::{CommandHint, EntrypointOutput};
        use crate::cli::projects::{ProjectOutput, ProjectTasksOutput, ProjectsOutput};
        use crate::cli::run::{
            RunOutput, RunPauseOutput, RunResumeOutput, RunStopOutput, RunsOutput,
        };
        use crate::cli::search::SearchOutput;
        use crate::cli::sessions::{SessionOutput, SessionsOutput};
        use crate::cli::show::{ArtifactOutput, ArtifactsOutput};
        use crate::cli::tasks::{NextTaskOutput, TaskCreatedOutput, TaskOutput, TasksOutput};
        use crate::cli::update::{UpdateCheckOutput, UpdateOutput};
        use crate::cli::work::{WorkBlockOutput, WorkDoneOutput, WorkReleaseOutput};
        use crate::cli::worker::{WorkerPruneOutput, WorkerStatusOutput, WorkerStopOutput};
        use crate::services::batch_service::BatchResult;

        fn make_batch_result(index: usize, op: &str, success: bool) -> BatchResult {
            BatchResult {
                index,
                op: op.to_string(),
                success,
                id: if success {
                    Some(format!("id-{}", index))
                } else {
                    None
                },
                error: if success {
                    None
                } else {
                    Some("something went wrong".to_string())
                },
            }
        }

        #[test]
        fn batch_output_json_empty() {
            let output = BatchOutput {
                results: vec![],
                success_count: 0,
                fail_count: 0,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            assert_eq!(parsed.as_array().unwrap().len(), 0);
        }

        #[test]
        fn batch_output_json_with_data() {
            let output = BatchOutput {
                results: vec![
                    make_batch_result(0, "task.create", true),
                    make_batch_result(1, "task.update", false),
                ],
                success_count: 1,
                fail_count: 1,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            assert_eq!(parsed.as_array().unwrap().len(), 2);
        }

        #[test]
        fn batch_stream_output_json_empty() {
            let output = BatchStreamOutput {
                results: vec![],
                success_count: 0,
                fail_count: 0,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            assert_eq!(parsed.as_array().unwrap().len(), 0);
        }

        #[test]
        fn batch_stream_output_json_with_data() {
            let output = BatchStreamOutput {
                results: vec![make_batch_result(0, "project.create", true)],
                success_count: 1,
                fail_count: 0,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            assert_eq!(parsed.as_array().unwrap().len(), 1);
        }

        #[test]
        fn entrypoint_output_json_initialized() {
            let output = EntrypointOutput {
                initialized: true,
                hints: vec![CommandHint {
                    label: "Plan a feature".to_string(),
                    command: "granary plan \"Feature name\"".to_string(),
                }],
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["initialized"], true);
            assert_eq!(parsed["hints"].as_array().unwrap().len(), 1);
        }

        #[test]
        fn entrypoint_output_json_not_initialized() {
            let output = EntrypointOutput {
                initialized: false,
                hints: vec![],
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["initialized"], false);
            assert_eq!(parsed["hints"].as_array().unwrap().len(), 0);
        }

        #[test]
        fn tasks_output_json_empty() {
            let output = TasksOutput { tasks: vec![] };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            assert_eq!(parsed.as_array().unwrap().len(), 0);
        }

        #[test]
        fn tasks_output_json_with_data() {
            let task = make_task("task-1", "Test task");
            let output = TasksOutput {
                tasks: vec![(task, vec!["task-0".to_string()])],
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            assert_eq!(parsed.as_array().unwrap().len(), 1);
        }

        #[test]
        fn next_task_output_json_none() {
            let output = NextTaskOutput {
                task: None,
                reason: None,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_object());
        }

        #[test]
        fn next_task_output_json_some() {
            let output = NextTaskOutput {
                task: Some(make_task("task-1", "Test")),
                reason: Some("priority".to_string()),
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_object());
            assert!(parsed.get("task").is_some());
        }

        #[test]
        fn task_output_json_valid() {
            let output = TaskOutput {
                task: make_task("task-1", "Test"),
                blocked_by: vec![],
            };
            let json = output.to_json();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }

        #[test]
        fn task_created_output_json_valid() {
            let output = TaskCreatedOutput {
                task: make_task("task-1", "Created task"),
            };
            let json = output.to_json();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }

        #[test]
        fn runs_output_json_empty() {
            let output = RunsOutput {
                runs: vec![],
                show_all_hint: true,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            assert_eq!(parsed.as_array().unwrap().len(), 0);
        }

        #[test]
        fn runs_output_json_with_data() {
            let output = RunsOutput {
                runs: vec![make_run("run-1")],
                show_all_hint: false,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
            assert_eq!(parsed.as_array().unwrap().len(), 1);
        }

        #[test]
        fn run_output_json_valid() {
            let output = RunOutput {
                run: make_run("run-1"),
            };
            let json = output.to_json();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }

        #[test]
        fn search_output_json_empty() {
            let output = SearchOutput { results: vec![] };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
        }

        #[test]
        fn projects_output_json_empty() {
            let output = ProjectsOutput { projects: vec![] };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
        }

        #[test]
        fn project_output_json_valid() {
            let output = ProjectOutput {
                project: make_project("proj-1", "Test Project"),
            };
            let json = output.to_json();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }

        #[test]
        fn project_tasks_output_json_empty() {
            let output = ProjectTasksOutput { tasks: vec![] };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
        }

        #[test]
        fn sessions_output_json_empty() {
            let output = SessionsOutput { sessions: vec![] };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
        }

        #[test]
        fn session_output_json_valid() {
            let output = SessionOutput {
                session: make_session("sess-1"),
            };
            let json = output.to_json();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }

        #[test]
        fn checkpoints_output_json_empty() {
            let output = CheckpointsOutput {
                checkpoints: vec![],
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
        }

        #[test]
        fn checkpoint_output_json_valid() {
            let output = CheckpointOutput {
                checkpoint: make_checkpoint("cp-1"),
            };
            let json = output.to_json();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }

        #[test]
        fn comments_output_json_empty() {
            let output = CommentsOutput { comments: vec![] };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
        }

        #[test]
        fn comment_output_json_valid() {
            let output = CommentOutput {
                comment: make_comment("cmt-1"),
            };
            let json = output.to_json();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }

        #[test]
        fn artifacts_output_json_empty() {
            let output = ArtifactsOutput { artifacts: vec![] };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.is_array());
        }

        #[test]
        fn artifact_output_json_valid() {
            let output = ArtifactOutput {
                artifact: make_artifact("art-1"),
            };
            let json = output.to_json();
            let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        }

        #[test]
        fn work_done_output_json_valid() {
            let output = WorkDoneOutput;
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["status"], "done");
        }

        #[test]
        fn work_block_output_json_valid() {
            let output = WorkBlockOutput;
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["status"], "blocked");
        }

        #[test]
        fn work_release_output_json_valid() {
            let output = WorkReleaseOutput;
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["status"], "released");
        }

        #[test]
        fn daemon_status_output_json_running() {
            let output = DaemonStatusOutput {
                running: true,
                pid: Some(1234),
                version: Some("0.1.0".to_string()),
                endpoint: Some("Socket: /tmp/granary.sock".to_string()),
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["running"], true);
            assert_eq!(parsed["pid"], 1234);
        }

        #[test]
        fn daemon_status_output_json_not_running() {
            let output = DaemonStatusOutput {
                running: false,
                pid: None,
                version: None,
                endpoint: None,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["running"], false);
        }

        #[test]
        fn daemon_start_output_json_valid() {
            let output = DaemonStartOutput {
                success: true,
                version: Some("0.1.0".to_string()),
                pid: Some(5678),
                error: None,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["success"], true);
        }

        #[test]
        fn daemon_stop_output_json_valid() {
            let output = DaemonStopOutput {
                stopped: true,
                warning: None,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["stopped"], true);
        }

        #[test]
        fn daemon_restart_output_json_valid() {
            let output = DaemonRestartOutput {
                stop: DaemonStopOutput {
                    stopped: true,
                    warning: None,
                },
                start: DaemonStartOutput {
                    success: true,
                    version: Some("0.1.0".to_string()),
                    pid: Some(9999),
                    error: None,
                },
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["stop"]["stopped"], true);
            assert_eq!(parsed["start"]["success"], true);
        }

        #[test]
        fn daemon_logs_output_json_valid() {
            let output = DaemonLogsOutput {
                logs: "test log line".to_string(),
                log_path: "/tmp/daemon.log".to_string(),
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed["logs"].as_str().unwrap().contains("test log line"));
        }

        #[test]
        fn worker_stop_output_json_valid() {
            let output = WorkerStopOutput {
                worker: make_worker("worker-1"),
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["stopped"], true);
            assert!(parsed["worker"].is_object());
        }

        #[test]
        fn worker_prune_output_json_zero() {
            let output = WorkerPruneOutput { pruned_count: 0 };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["pruned_count"], 0);
        }

        #[test]
        fn worker_prune_output_json_with_data() {
            let output = WorkerPruneOutput { pruned_count: 3 };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["pruned_count"], 3);
        }

        #[test]
        fn worker_status_output_json_valid() {
            let output = WorkerStatusOutput {
                worker: make_worker("worker-1"),
                running: 2,
                pending: 1,
                completed: 5,
                failed: 0,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed["worker"].is_object());
            assert_eq!(parsed["run_statistics"]["running"], 2);
            assert_eq!(parsed["run_statistics"]["pending"], 1);
            assert_eq!(parsed["run_statistics"]["completed"], 5);
            assert_eq!(parsed["run_statistics"]["failed"], 0);
        }

        #[test]
        fn run_stop_output_json_valid() {
            let output = RunStopOutput {
                run: make_run("run-1"),
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["stopped"], true);
            assert!(parsed["run"].is_object());
        }

        #[test]
        fn run_pause_output_json_valid() {
            let output = RunPauseOutput {
                run: make_run("run-1"),
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["paused"], true);
            assert!(parsed["run"].is_object());
        }

        #[test]
        fn run_resume_output_json_valid() {
            let output = RunResumeOutput {
                run: make_run("run-1"),
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["resumed"], true);
            assert!(parsed["run"].is_object());
        }

        #[test]
        fn update_check_output_json_has_update() {
            let output = UpdateCheckOutput {
                current_version: "0.1.0".to_string(),
                latest_stable: "0.2.0".to_string(),
                latest_prerelease: Some("0.3.0-pre.1".to_string()),
                has_update: true,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["current_version"], "0.1.0");
            assert_eq!(parsed["latest_stable"], "0.2.0");
            assert_eq!(parsed["latest_prerelease"], "0.3.0-pre.1");
            assert_eq!(parsed["has_update"], true);
        }

        #[test]
        fn update_check_output_json_up_to_date() {
            let output = UpdateCheckOutput {
                current_version: "0.2.0".to_string(),
                latest_stable: "0.2.0".to_string(),
                latest_prerelease: None,
                has_update: false,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["has_update"], false);
            assert!(parsed["latest_prerelease"].is_null());
        }

        #[test]
        fn update_output_json_success() {
            let output = UpdateOutput {
                from_version: "0.1.0".to_string(),
                to_version: "0.2.0".to_string(),
                success: true,
                latest_prerelease: None,
            };
            let json = output.to_json();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed["from_version"], "0.1.0");
            assert_eq!(parsed["to_version"], "0.2.0");
            assert_eq!(parsed["success"], true);
        }
    }

    // =========================================================================
    // Format dispatch tests - verify format() routes correctly
    // =========================================================================

    mod format_dispatch {
        use super::*;
        use crate::cli::tasks::TasksOutput;
        use crate::cli::work::WorkDoneOutput;

        #[test]
        fn format_none_uses_default_output_type() {
            let output = TasksOutput { tasks: vec![] };
            // TasksOutput defaults to Text, so format(None) should use to_text()
            let via_format = output.format(None);
            let via_text = output.to_text();
            assert_eq!(via_format, via_text);
        }

        #[test]
        fn format_json_override() {
            let output = TasksOutput { tasks: vec![] };
            let via_format = output.format(Some(CliOutputFormat::Json));
            let via_json = output.to_json();
            assert_eq!(via_format, via_json);
        }

        #[test]
        fn format_prompt_override() {
            let output = TasksOutput { tasks: vec![] };
            let via_format = output.format(Some(CliOutputFormat::Prompt));
            let via_prompt = output.to_prompt();
            assert_eq!(via_format, via_prompt);
        }

        #[test]
        fn format_table_override() {
            let output = WorkDoneOutput;
            let via_format = output.format(Some(CliOutputFormat::Table));
            let via_text = output.to_text();
            assert_eq!(via_format, via_text);
        }

        #[test]
        fn format_md_falls_back_to_text() {
            let output = WorkDoneOutput;
            let via_format = output.format(Some(CliOutputFormat::Md));
            let via_text = output.to_text();
            assert_eq!(via_format, via_text);
        }

        #[test]
        fn format_yaml_falls_back_to_text() {
            let output = WorkDoneOutput;
            let via_format = output.format(Some(CliOutputFormat::Yaml));
            let via_text = output.to_text();
            assert_eq!(via_format, via_text);
        }
    }

    // =========================================================================
    // OutputType conversion tests
    // =========================================================================

    mod output_type_conversions {
        use super::*;

        #[test]
        fn output_type_to_cli_format() {
            assert!(matches!(
                CliOutputFormat::from(OutputType::Text),
                CliOutputFormat::Table
            ));
            assert!(matches!(
                CliOutputFormat::from(OutputType::Prompt),
                CliOutputFormat::Prompt
            ));
            assert!(matches!(
                CliOutputFormat::from(OutputType::Json),
                CliOutputFormat::Json
            ));
        }

        #[test]
        fn cli_format_to_output_type() {
            assert_eq!(OutputType::from(CliOutputFormat::Table), OutputType::Text);
            assert_eq!(OutputType::from(CliOutputFormat::Json), OutputType::Json);
            assert_eq!(
                OutputType::from(CliOutputFormat::Prompt),
                OutputType::Prompt
            );
            assert_eq!(OutputType::from(CliOutputFormat::Md), OutputType::Text);
            assert_eq!(OutputType::from(CliOutputFormat::Yaml), OutputType::Json);
        }

        #[test]
        fn output_type_default_is_text() {
            assert_eq!(OutputType::default(), OutputType::Text);
        }
    }
}
