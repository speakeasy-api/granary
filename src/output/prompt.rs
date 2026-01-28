//! Prompt output format - LLM-consumable structured text
//!
//! This format is designed to be machine-readable by LLMs while remaining
//! human-readable. It uses a consistent structure that's easy to parse.

use crate::models::initiative::Initiative;
use crate::models::*;
use crate::output::json::{ContextOutput, HandoffOutput, SummaryOutput};

/// Format a project for LLM consumption
pub fn format_project(project: &Project) -> String {
    let mut output = String::new();
    output.push_str("<project>\n");
    output.push_str(&format!("id: {}\n", project.id));
    output.push_str(&format!("name: {}\n", project.name));
    output.push_str(&format!("status: {}\n", project.status));
    if let Some(owner) = &project.owner {
        output.push_str(&format!("owner: {}\n", owner));
    }
    if let Some(desc) = &project.description {
        output.push_str(&format!("description: {}\n", desc));
    }
    let tags = project.tags_vec();
    if !tags.is_empty() {
        output.push_str(&format!("tags: {}\n", tags.join(", ")));
    }
    output.push_str("</project>\n");
    output
}

pub fn format_projects(projects: &[Project]) -> String {
    let mut output = String::new();
    output.push_str(&format!("<projects count=\"{}\">\n", projects.len()));
    for project in projects {
        output.push_str(&format!(
            "  - {} ({}) [{}]\n",
            project.name, project.id, project.status
        ));
    }
    output.push_str("</projects>\n");
    output
}

pub fn format_task(task: &Task) -> String {
    format_task_with_deps(task, &[])
}

pub fn format_task_with_deps(task: &Task, blocked_by: &[String]) -> String {
    let mut output = String::new();
    output.push_str("<task>\n");
    output.push_str(&format!("id: {}\n", task.id));
    output.push_str(&format!("title: {}\n", task.title));
    output.push_str(&format!("status: {}\n", task.status));
    output.push_str(&format!("priority: {}\n", task.priority));
    output.push_str(&format!("project: {}\n", task.project_id));
    if let Some(owner) = &task.owner {
        output.push_str(&format!("owner: {}\n", owner));
    }
    if let Some(parent) = &task.parent_task_id {
        output.push_str(&format!("parent_task: {}\n", parent));
    }
    if let Some(desc) = &task.description {
        output.push_str(&format!("description: {}\n", desc));
    }
    if let Some(reason) = &task.blocked_reason {
        output.push_str(&format!("blocked_reason: {}\n", reason));
    }
    if !blocked_by.is_empty() {
        output.push_str(&format!("blocked_by: {}\n", blocked_by.join(", ")));
    }
    if let Some(due) = &task.due_at {
        output.push_str(&format!("due_at: {}\n", due));
    }
    if task.pinned != 0 {
        output.push_str("pinned: true\n");
    }
    output.push_str("</task>\n");
    output
}

pub fn format_tasks(tasks: &[Task]) -> String {
    // Create tasks with empty deps for backwards compatibility
    let tasks_with_deps: Vec<(&Task, &[String])> = tasks.iter().map(|t| (t, &[][..])).collect();
    format_tasks_with_deps(&tasks_with_deps)
}

pub fn format_tasks_with_deps(tasks_with_deps: &[(&Task, &[String])]) -> String {
    let mut output = String::new();
    output.push_str(&format!("<tasks count=\"{}\">\n", tasks_with_deps.len()));
    for (task, blocked_by) in tasks_with_deps {
        let blocked = if task.blocked_reason.is_some() || !blocked_by.is_empty() {
            " [BLOCKED]"
        } else {
            ""
        };
        let deps_info = if !blocked_by.is_empty() {
            format!(" blocked_by: {}", blocked_by.join(", "))
        } else {
            String::new()
        };
        output.push_str(&format!(
            "  - [{}] {} ({}) {}{}{}\n",
            task.priority, task.title, task.id, task.status, blocked, deps_info
        ));
    }
    output.push_str("</tasks>\n");
    output
}

pub fn format_comment(comment: &Comment) -> String {
    let mut output = String::new();
    output.push_str("<comment>\n");
    output.push_str(&format!("id: {}\n", comment.id));
    output.push_str(&format!("kind: {}\n", comment.kind));
    output.push_str(&format!("parent: {}\n", comment.parent_id));
    if let Some(author) = &comment.author {
        output.push_str(&format!("author: {}\n", author));
    }
    output.push_str(&format!("created_at: {}\n", comment.created_at));
    output.push_str(&format!("content:\n{}\n", comment.content));
    output.push_str("</comment>\n");
    output
}

pub fn format_comments(comments: &[Comment]) -> String {
    let mut output = String::new();
    output.push_str(&format!("<comments count=\"{}\">\n", comments.len()));
    for comment in comments {
        let author = comment.author.as_deref().unwrap_or("anonymous");
        output.push_str(&format!(
            "  - [{}] {}: {}\n",
            comment.kind,
            author,
            truncate(&comment.content, 60)
        ));
    }
    output.push_str("</comments>\n");
    output
}

pub fn format_session(session: &Session) -> String {
    let mut output = String::new();
    output.push_str("<session>\n");
    output.push_str(&format!("id: {}\n", session.id));
    if let Some(name) = &session.name {
        output.push_str(&format!("name: {}\n", name));
    }
    if let Some(mode) = &session.mode {
        output.push_str(&format!("mode: {}\n", mode));
    }
    if let Some(owner) = &session.owner {
        output.push_str(&format!("owner: {}\n", owner));
    }
    let status = if session.is_closed() {
        "closed"
    } else {
        "active"
    };
    output.push_str(&format!("status: {}\n", status));
    if let Some(focus) = &session.focus_task_id {
        output.push_str(&format!("focus_task: {}\n", focus));
    }
    output.push_str(&format!("created_at: {}\n", session.created_at));
    output.push_str("</session>\n");
    output
}

pub fn format_sessions(sessions: &[Session]) -> String {
    let mut output = String::new();
    output.push_str(&format!("<sessions count=\"{}\">\n", sessions.len()));
    for session in sessions {
        let name = session.name.as_deref().unwrap_or("unnamed");
        let status = if session.is_closed() {
            "closed"
        } else {
            "active"
        };
        output.push_str(&format!("  - {} ({}) [{}]\n", name, session.id, status));
    }
    output.push_str("</sessions>\n");
    output
}

pub fn format_checkpoint(checkpoint: &Checkpoint) -> String {
    let mut output = String::new();
    output.push_str("<checkpoint>\n");
    output.push_str(&format!("id: {}\n", checkpoint.id));
    output.push_str(&format!("name: {}\n", checkpoint.name));
    output.push_str(&format!("session: {}\n", checkpoint.session_id));
    output.push_str(&format!("created_at: {}\n", checkpoint.created_at));
    output.push_str("</checkpoint>\n");
    output
}

pub fn format_checkpoints(checkpoints: &[Checkpoint]) -> String {
    let mut output = String::new();
    output.push_str(&format!("<checkpoints count=\"{}\">\n", checkpoints.len()));
    for cp in checkpoints {
        output.push_str(&format!("  - {} ({}) {}\n", cp.name, cp.id, cp.created_at));
    }
    output.push_str("</checkpoints>\n");
    output
}

pub fn format_next_task(task: Option<&Task>, reason: Option<&str>) -> String {
    let mut output = String::new();
    output.push_str("<next_task>\n");
    match task {
        Some(t) => {
            output.push_str(&format!("id: {}\n", t.id));
            output.push_str(&format!("title: {}\n", t.title));
            output.push_str(&format!("priority: {}\n", t.priority));
            output.push_str(&format!("status: {}\n", t.status));
            output.push_str(&format!("project: {}\n", t.project_id));
            if let Some(desc) = &t.description {
                output.push_str(&format!("description: {}\n", desc));
            }
            if let Some(r) = reason {
                output.push_str(&format!("selection_reason: {}\n", r));
            }
        }
        None => {
            output.push_str("status: no_task_available\n");
            if let Some(r) = reason {
                output.push_str(&format!("reason: {}\n", r));
            }
        }
    }
    output.push_str("</next_task>\n");
    output
}

/// Format a summary for LLM consumption
/// This follows the recommended structure from the spec
pub fn format_summary(summary: &SummaryOutput) -> String {
    let mut output = String::new();

    // Session header
    output.push_str("<summary>\n");

    if let Some(session) = &summary.session {
        output.push_str("<session_header>\n");
        output.push_str(&format!("id: {}\n", session.id));
        if let Some(name) = &session.name {
            output.push_str(&format!("name: {}\n", name));
        }
        if let Some(mode) = &session.mode {
            output.push_str(&format!("mode: {}\n", mode));
        }
        if let Some(owner) = &session.owner {
            output.push_str(&format!("owner: {}\n", owner));
        }
        if let Some(focus) = &session.focus_task_id {
            output.push_str(&format!("focus_task: {}\n", focus));
        }
        output.push_str("</session_header>\n\n");
    }

    // State of work
    output.push_str("<state_of_work>\n");
    output.push_str(&format!("total_tasks: {}\n", summary.state.total_tasks));
    output.push_str("by_status:\n");
    output.push_str(&format!("  todo: {}\n", summary.state.by_status.todo));
    output.push_str(&format!(
        "  in_progress: {}\n",
        summary.state.by_status.in_progress
    ));
    output.push_str(&format!("  done: {}\n", summary.state.by_status.done));
    output.push_str(&format!("  blocked: {}\n", summary.state.by_status.blocked));
    output.push_str("by_priority:\n");
    output.push_str(&format!("  P0: {}\n", summary.state.by_priority.p0));
    output.push_str(&format!("  P1: {}\n", summary.state.by_priority.p1));
    output.push_str(&format!("  P2: {}\n", summary.state.by_priority.p2));
    output.push_str(&format!("  P3: {}\n", summary.state.by_priority.p3));
    output.push_str(&format!("  P4: {}\n", summary.state.by_priority.p4));
    output.push_str("</state_of_work>\n\n");

    // Focus task detail
    if let Some(focus_task) = &summary.focus_task {
        output.push_str("<focus_task>\n");
        output.push_str(&format!("id: {}\n", focus_task.id));
        output.push_str(&format!("title: {}\n", focus_task.title));
        output.push_str(&format!("status: {}\n", focus_task.status));
        output.push_str(&format!("priority: {}\n", focus_task.priority));
        if let Some(desc) = &focus_task.description {
            output.push_str(&format!("description: {}\n", desc));
        }
        output.push_str("</focus_task>\n\n");
    }

    // Blockers
    if !summary.blockers.is_empty() {
        output.push_str("<blockers>\n");
        for task in &summary.blockers {
            output.push_str(&format!("  - {} ({})", task.title, task.id));
            if let Some(reason) = &task.blocked_reason {
                output.push_str(&format!(": {}", reason));
            }
            output.push('\n');
        }
        output.push_str("</blockers>\n\n");
    }

    // Next actionable tasks
    if !summary.next_actions.is_empty() {
        output.push_str("<next_actions>\n");
        for task in &summary.next_actions {
            output.push_str(&format!(
                "  - [{}] {} ({})\n",
                task.priority, task.title, task.id
            ));
        }
        output.push_str("</next_actions>\n\n");
    }

    // Recent decisions
    if !summary.recent_decisions.is_empty() {
        output.push_str("<recent_decisions>\n");
        for comment in &summary.recent_decisions {
            let author = comment.author.as_deref().unwrap_or("unknown");
            output.push_str(&format!("  - {}: {}\n", author, comment.content));
        }
        output.push_str("</recent_decisions>\n\n");
    }

    // Recent artifacts
    if !summary.recent_artifacts.is_empty() {
        output.push_str("<recent_artifacts>\n");
        for artifact in &summary.recent_artifacts {
            output.push_str(&format!(
                "  - [{}] {}\n",
                artifact.artifact_type, artifact.path_or_url
            ));
        }
        output.push_str("</recent_artifacts>\n");
    }

    output.push_str("</summary>\n");
    output
}

/// Format a context pack for LLM consumption
pub fn format_context(context: &ContextOutput) -> String {
    let mut output = String::new();

    output.push_str("<context_pack>\n");

    // Session info
    if let Some(session) = &context.session {
        output.push_str("<session>\n");
        output.push_str(&format!("id: {}\n", session.id));
        if let Some(name) = &session.name {
            output.push_str(&format!("name: {}\n", name));
        }
        if let Some(mode) = &session.mode {
            output.push_str(&format!("mode: {}\n", mode));
        }
        output.push_str("</session>\n\n");
    }

    // Projects
    if !context.projects.is_empty() {
        output.push_str(&format!(
            "<projects count=\"{}\">\n",
            context.projects.len()
        ));
        for project in &context.projects {
            output.push_str(&format!("  - {} ({})\n", project.name, project.id));
        }
        output.push_str("</projects>\n\n");
    }

    // Tasks
    if !context.tasks.is_empty() {
        output.push_str(&format!("<tasks count=\"{}\">\n", context.tasks.len()));
        for task in &context.tasks {
            let blocked = if task.blocked_reason.is_some() {
                " [BLOCKED]"
            } else {
                ""
            };
            output.push_str(&format!(
                "  - [{}] {} ({}) {}{}\n",
                task.priority, task.title, task.id, task.status, blocked
            ));
        }
        output.push_str("</tasks>\n\n");
    }

    // Decisions
    if !context.decisions.is_empty() {
        output.push_str(&format!(
            "<decisions count=\"{}\">\n",
            context.decisions.len()
        ));
        for decision in &context.decisions {
            output.push_str(&format!(
                "  - {}: {}\n",
                decision.parent_id, decision.content
            ));
        }
        output.push_str("</decisions>\n\n");
    }

    // Blockers
    if !context.blockers.is_empty() {
        output.push_str(&format!(
            "<blockers count=\"{}\">\n",
            context.blockers.len()
        ));
        for blocker in &context.blockers {
            output.push_str(&format!("  - {} ({})", blocker.task_title, blocker.task_id));
            if let Some(reason) = &blocker.reason {
                output.push_str(&format!(": {}", reason));
            }
            if !blocker.unmet_deps.is_empty() {
                output.push_str(&format!(" [deps: {}]", blocker.unmet_deps.join(", ")));
            }
            output.push('\n');
        }
        output.push_str("</blockers>\n\n");
    }

    // Comments
    if !context.comments.is_empty() {
        output.push_str(&format!(
            "<comments count=\"{}\">\n",
            context.comments.len()
        ));
        for comment in &context.comments {
            let author = comment.author.as_deref().unwrap_or("unknown");
            output.push_str(&format!(
                "  - [{}] {}: {}\n",
                comment.kind, author, comment.content
            ));
        }
        output.push_str("</comments>\n\n");
    }

    // Artifacts
    if !context.artifacts.is_empty() {
        output.push_str(&format!(
            "<artifacts count=\"{}\">\n",
            context.artifacts.len()
        ));
        for artifact in &context.artifacts {
            output.push_str(&format!(
                "  - [{}] {}\n",
                artifact.artifact_type, artifact.path_or_url
            ));
        }
        output.push_str("</artifacts>\n\n");
    }

    // Steering files
    if !context.steering.is_empty() {
        output.push_str(&format!(
            "<steering count=\"{}\">\n",
            context.steering.len()
        ));
        for steering in &context.steering {
            output.push_str(&format!(
                "<steering_file path=\"{}\" mode=\"{}\">\n",
                steering.path, steering.mode
            ));
            if let Some(content) = &steering.content {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            } else {
                output.push_str("(content not available - external reference)\n");
            }
            output.push_str("</steering_file>\n");
        }
        output.push_str("</steering>\n");
    }

    output.push_str("</context_pack>\n");
    output
}

/// Format a handoff document for agent delegation
pub fn format_handoff(handoff: &HandoffOutput) -> String {
    let mut output = String::new();

    output.push_str("<handoff>\n");
    output.push_str(&format!("to: {}\n\n", handoff.to));

    output.push_str("<tasks>\n");
    for task in &handoff.tasks {
        output.push_str(&format!("- id: {}\n", task.id));
        output.push_str(&format!("  title: {}\n", task.title));
        output.push_str(&format!("  priority: {}\n", task.priority));
        output.push_str(&format!("  status: {}\n", task.status));
        if let Some(desc) = &task.description {
            output.push_str(&format!("  description: {}\n", desc));
        }
    }
    output.push_str("</tasks>\n\n");

    if !handoff.context.is_empty() {
        output.push_str("<context>\n");
        for comment in &handoff.context {
            let author = comment.author.as_deref().unwrap_or("unknown");
            output.push_str(&format!(
                "- [{}] {}: {}\n",
                comment.kind, author, comment.content
            ));
        }
        output.push_str("</context>\n\n");
    }

    if let Some(constraints) = &handoff.constraints {
        output.push_str(&format!(
            "<constraints>\n{}\n</constraints>\n\n",
            constraints
        ));
    }

    if let Some(criteria) = &handoff.acceptance_criteria {
        output.push_str(&format!(
            "<acceptance_criteria>\n{}\n</acceptance_criteria>\n\n",
            criteria
        ));
    }

    if let Some(schema) = &handoff.output_schema {
        output.push_str("<output_schema>\n");
        output.push_str(&serde_json::to_string_pretty(schema).unwrap_or_else(|_| "{}".to_string()));
        output.push_str("\n</output_schema>\n\n");
    }

    // Steering files for the delegated agent
    if !handoff.steering.is_empty() {
        output.push_str(&format!(
            "<steering count=\"{}\">\n",
            handoff.steering.len()
        ));
        output.push_str("The following standards and conventions must be followed:\n\n");
        for steering in &handoff.steering {
            output.push_str(&format!("<steering_file path=\"{}\">\n", steering.path));
            if let Some(content) = &steering.content {
                output.push_str(content);
                if !content.ends_with('\n') {
                    output.push('\n');
                }
            } else {
                output.push_str("(reference to external document)\n");
            }
            output.push_str("</steering_file>\n");
        }
        output.push_str("</steering>\n\n");
    }

    output.push_str("<instructions>\n");
    output.push_str("1. Complete the assigned task(s) according to the context provided.\n");
    output.push_str("2. Follow any constraints specified.\n");
    output.push_str("3. Follow the steering documents for coding standards and conventions.\n");
    output.push_str("4. Ensure acceptance criteria are met.\n");
    output.push_str("5. Report findings using the output schema if provided.\n");
    output.push_str("6. Update task status upon completion.\n");
    output.push_str("</instructions>\n");

    output.push_str("</handoff>\n");
    output
}

pub fn format_search_results(results: &[SearchResult]) -> String {
    let mut output = String::new();
    output.push_str(&format!("<search_results count=\"{}\">\n", results.len()));

    for result in results {
        match result {
            SearchResult::Initiative {
                id,
                name,
                description,
                status,
            } => {
                output.push_str("<initiative>\n");
                output.push_str(&format!("id: {}\n", id));
                output.push_str(&format!("name: {}\n", name));
                output.push_str(&format!("status: {}\n", status));
                if let Some(desc) = description {
                    output.push_str(&format!("description: {}\n", desc));
                }
                output.push_str("</initiative>\n");
            }
            SearchResult::Project {
                id,
                name,
                description,
                status,
            } => {
                output.push_str("<project>\n");
                output.push_str(&format!("id: {}\n", id));
                output.push_str(&format!("name: {}\n", name));
                output.push_str(&format!("status: {}\n", status));
                if let Some(desc) = description {
                    output.push_str(&format!("description: {}\n", desc));
                }
                output.push_str("</project>\n");
            }
            SearchResult::Task {
                id,
                title,
                description,
                status,
                priority,
                project_id,
            } => {
                output.push_str("<task>\n");
                output.push_str(&format!("id: {}\n", id));
                output.push_str(&format!("title: {}\n", title));
                output.push_str(&format!("status: {}\n", status));
                output.push_str(&format!("priority: {}\n", priority));
                output.push_str(&format!("project: {}\n", project_id));
                if let Some(desc) = description {
                    output.push_str(&format!("description: {}\n", desc));
                }
                output.push_str("</task>\n");
            }
        }
    }

    output.push_str("</search_results>\n");
    output
}

// Helper function
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}

/// Format an initiative for LLM consumption
pub fn format_initiative(initiative: &Initiative) -> String {
    let mut output = String::new();
    output.push_str("<initiative>\n");
    output.push_str(&format!("id: {}\n", initiative.id));
    output.push_str(&format!("name: {}\n", initiative.name));
    output.push_str(&format!("status: {}\n", initiative.status));
    if let Some(owner) = &initiative.owner {
        output.push_str(&format!("owner: {}\n", owner));
    }
    if let Some(desc) = &initiative.description {
        output.push_str(&format!("description: {}\n", desc));
    }
    let tags = initiative.tags_vec();
    if !tags.is_empty() {
        output.push_str(&format!("tags: {}\n", tags.join(", ")));
    }
    output.push_str("</initiative>\n");
    output
}

pub fn format_initiatives(initiatives: &[Initiative]) -> String {
    let mut output = String::new();
    output.push_str(&format!("<initiatives count=\"{}\">\n", initiatives.len()));
    for initiative in initiatives {
        output.push_str(&format!(
            "  - {} ({}) [{}]\n",
            initiative.name, initiative.id, initiative.status
        ));
    }
    output.push_str("</initiatives>\n");
    output
}

// === Initiative Summary ===

use crate::models::initiative::InitiativeSummary;

/// Format an initiative summary for LLM consumption.
/// Optimized for low token count while conveying actionable information.
pub fn format_initiative_summary(summary: &InitiativeSummary) -> String {
    let mut lines = Vec::new();

    // Header - compact
    lines.push(format!("# Initiative: {}", summary.initiative.name));
    lines.push(format!(
        "Progress: {:.0}% ({}/{} tasks)",
        summary.status.percent_complete, summary.status.tasks_done, summary.status.total_tasks
    ));
    lines.push(format!(
        "Projects: {} total, {} complete, {} blocked",
        summary.status.total_projects,
        summary.status.completed_projects,
        summary.status.blocked_projects
    ));

    // Blockers - if any
    if !summary.blockers.is_empty() {
        lines.push(String::new());
        lines.push("## Blockers".to_string());
        for b in &summary.blockers {
            lines.push(format!(
                "- [{}] {}: {}",
                b.blocker_type, b.project_name, b.description
            ));
        }
    }

    // Next actions - compact list
    if !summary.next_actions.is_empty() {
        lines.push(String::new());
        lines.push("## Next Actions".to_string());
        for a in &summary.next_actions {
            lines.push(format!(
                "- [{}] {} > {}",
                a.priority, a.project_name, a.task_title
            ));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_task() -> Task {
        Task {
            id: "test-proj-task-1".to_string(),
            project_id: "test-proj".to_string(),
            task_number: 1,
            parent_task_id: None,
            title: "Test Task".to_string(),
            description: Some("A test task description".to_string()),
            status: "todo".to_string(),
            priority: "P1".to_string(),
            owner: Some("test-user".to_string()),
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
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-01T00:00:00Z".to_string(),
            version: 1,
        }
    }

    #[test]
    fn test_format_task_no_dependencies() {
        let task = create_test_task();
        let output = format_task(&task);

        assert!(output.contains("id: test-proj-task-1"));
        assert!(output.contains("title: Test Task"));
        assert!(output.contains("status: todo"));
        // Should NOT contain blocked_by when empty
        assert!(!output.contains("blocked_by:"));
    }

    #[test]
    fn test_format_task_with_dependencies() {
        let task = create_test_task();
        let blocked_by = vec!["dep-1".to_string(), "dep-2".to_string()];
        let output = format_task_with_deps(&task, &blocked_by);

        assert!(output.contains("id: test-proj-task-1"));
        assert!(output.contains("blocked_by: dep-1, dep-2"));
    }

    #[test]
    fn test_format_tasks_with_dependencies() {
        let task1 = create_test_task();
        let mut task2 = create_test_task();
        task2.id = "test-proj-task-2".to_string();
        task2.title = "Task 2".to_string();

        let blocked_by1: &[String] = &["blocker-1".to_string()];
        let blocked_by2: &[String] = &[];

        let tasks_with_deps: Vec<(&Task, &[String])> =
            vec![(&task1, blocked_by1), (&task2, blocked_by2)];
        let output = format_tasks_with_deps(&tasks_with_deps);

        // First task should show blocked
        assert!(output.contains("test-proj-task-1"));
        assert!(output.contains("[BLOCKED]"));
        assert!(output.contains("blocked_by: blocker-1"));

        // Second task should not show blocked
        assert!(output.contains("test-proj-task-2"));
    }

    #[test]
    fn test_format_task_with_deps_shows_blocked_by_line() {
        let task = create_test_task();
        let blocked_by = vec!["dependency-task".to_string()];
        let output = format_task_with_deps(&task, &blocked_by);

        // Verify the blocked_by line appears in the output
        assert!(output.contains("blocked_by: dependency-task"));
        // And it should be between the task tags
        assert!(output.starts_with("<task>"));
        assert!(output.ends_with("</task>\n"));
    }
}
