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

    // =========================================================================
    // Output trait default type tests
    // =========================================================================

    mod default_output_types {
        use super::*;
        use crate::cli::checkpoints::{CheckpointOutput, CheckpointsOutput};
        use crate::cli::comments::{CommentOutput, CommentsOutput};
        use crate::cli::initiate::InitiateOutput;
        use crate::cli::initiatives::{
            InitiativeOutput as InitiativeShowOutput, InitiativeProjectsOutput,
            InitiativeSummaryOutput, InitiativeTaskOutput, InitiativeTasksOutput,
            InitiativesOutput,
        };
        use crate::cli::plan::{ExistingPlanOutput, PlanOutput};
        use crate::cli::projects::{ProjectOutput, ProjectTasksOutput, ProjectsOutput};
        use crate::cli::run::{RunOutput, RunsOutput};
        use crate::cli::search::SearchOutput;
        use crate::cli::sessions::{SessionOutput, SessionsOutput};
        use crate::cli::show::{ArtifactOutput, ArtifactsOutput};
        use crate::cli::summary::{ContextOutput, HandoffOutput, SummaryOutput};
        use crate::cli::tasks::{NextTaskOutput, TaskCreatedOutput, TaskOutput, TasksOutput};
        use crate::cli::work::{WorkBlockOutput, WorkDoneOutput, WorkOutput, WorkReleaseOutput};
        use crate::cli::workers::{WorkerOutput, WorkersOutput};

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
    }

    // =========================================================================
    // JSON validity tests - verify to_json() produces valid JSON
    // =========================================================================

    mod json_validity {
        use super::*;
        use crate::cli::checkpoints::{CheckpointOutput, CheckpointsOutput};
        use crate::cli::comments::{CommentOutput, CommentsOutput};
        use crate::cli::projects::{ProjectOutput, ProjectTasksOutput, ProjectsOutput};
        use crate::cli::run::{RunOutput, RunsOutput};
        use crate::cli::search::SearchOutput;
        use crate::cli::sessions::{SessionOutput, SessionsOutput};
        use crate::cli::show::{ArtifactOutput, ArtifactsOutput};
        use crate::cli::tasks::{NextTaskOutput, TaskCreatedOutput, TaskOutput, TasksOutput};
        use crate::cli::work::{WorkBlockOutput, WorkDoneOutput, WorkReleaseOutput};

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
