use serde::Serialize;

use crate::cli::args::CliOutputFormat;
use crate::cli::workspace;
use crate::error::Result;
use crate::output::{Output, OutputType};
use crate::services::{
    DiagnosticResult, DiagnosticStatus, InjectionResult, Workspace, contains_granary_instruction,
    find_global_agent_dirs, find_workspace_agent_files, get_global_instruction_file_path,
    inject_granary_instruction,
};

/// Initialize a new workspace (delegates to `workspace init`)
pub async fn init(
    local: bool,
    force: bool,
    skip_git_check: bool,
    name: Option<String>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    workspace::workspace_init(local, force, skip_git_check, name, cli_format).await
}

// ── Doctor Output ──────────────────────────────────────────────────────

pub struct DoctorOutput {
    pub diagnostics: Vec<DiagnosticResult>,
    pub agent_checks: Vec<AgentFileCheck>,
    pub has_unfixed_errors: bool,
}

pub struct AgentFileCheck {
    pub agent: String,
    pub path: String,
    pub status: DiagnosticStatus,
    pub message: Option<String>,
}

impl Output for DoctorOutput {
    fn output_type() -> OutputType {
        OutputType::Text
    }

    fn to_json(&self) -> String {
        let diagnostics: Vec<DiagnosticJson> = self
            .diagnostics
            .iter()
            .map(|d| DiagnosticJson {
                check: &d.check,
                status: d.status_str(),
                message: &d.message,
            })
            .collect();

        let agent_files: Vec<AgentFileJson> = self
            .agent_checks
            .iter()
            .map(|a| AgentFileJson {
                agent: &a.agent,
                path: &a.path,
                status: a.status.status_str(),
                message: a.message.as_deref(),
            })
            .collect();

        let json = DoctorJson {
            diagnostics,
            agent_files,
        };
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        let mut out = String::new();

        for d in &self.diagnostics {
            out.push_str(&format!(
                "{}: {} ({})\n",
                d.check,
                d.message,
                d.status_str()
            ));
        }

        if !self.agent_checks.is_empty() {
            out.push_str("\nAgent Files:\n");
            for a in &self.agent_checks {
                let msg = a
                    .message
                    .as_deref()
                    .map(|m| format!(" - {}", m))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "{}: {} [{}]{}\n",
                    a.agent,
                    a.path,
                    a.status.status_str(),
                    msg
                ));
            }
        }

        if self.has_unfixed_errors {
            out.push_str(
                "\nRun `granary doctor --fix` to add granary instructions to missing files.\n",
            );
        }

        out.trim_end().to_string()
    }

    fn to_text(&self) -> String {
        let mut out = String::new();

        out.push_str("Granary Doctor\n");
        out.push_str("==============\n\n");

        for d in &self.diagnostics {
            out.push_str(&format!(
                "{:8} {}: {}\n",
                d.status_symbol(),
                d.check,
                d.message
            ));
        }

        if !self.agent_checks.is_empty() {
            out.push_str("\nAgent Files\n");
            out.push_str("-----------\n\n");

            for a in &self.agent_checks {
                let symbol = a.status.status_symbol();
                let msg = a
                    .message
                    .as_deref()
                    .map(|m| format!(" ({})", m))
                    .unwrap_or_default();
                out.push_str(&format!("{:8} {}: {}{}\n", symbol, a.agent, a.path, msg));
            }
        }

        if self.has_unfixed_errors {
            out.push_str(
                "\nRun `granary doctor --fix` to add granary instructions to missing files.\n",
            );
        }

        out.trim_end().to_string()
    }
}

impl DiagnosticStatus {
    fn status_str(&self) -> &'static str {
        match self {
            DiagnosticStatus::Ok => "ok",
            DiagnosticStatus::Warning => "warning",
            DiagnosticStatus::Error => "error",
            DiagnosticStatus::Info => "info",
            DiagnosticStatus::Fix => "fixed",
        }
    }

    fn status_symbol(&self) -> &'static str {
        match self {
            DiagnosticStatus::Ok => "[OK]",
            DiagnosticStatus::Warning => "[WARN]",
            DiagnosticStatus::Error => "[ERR]",
            DiagnosticStatus::Info => "[INFO]",
            DiagnosticStatus::Fix => "[FIX]",
        }
    }
}

#[derive(Serialize)]
struct DoctorJson<'a> {
    diagnostics: Vec<DiagnosticJson<'a>>,
    agent_files: Vec<AgentFileJson<'a>>,
}

#[derive(Serialize)]
struct DiagnosticJson<'a> {
    check: &'a str,
    status: &'a str,
    message: &'a str,
}

#[derive(Serialize)]
struct AgentFileJson<'a> {
    agent: &'a str,
    path: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
}

// ── Doctor Command ─────────────────────────────────────────────────────

/// Run diagnostic checks
pub async fn doctor(fix: bool, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;
    let diagnostics = workspace.doctor().await?;

    let agent_files = find_workspace_agent_files(&workspace.root)?;
    let global_dirs = find_global_agent_dirs()?;

    let mut agent_checks = Vec::new();

    // Check workspace agent files
    for file in &agent_files {
        let (status, message) = check_and_maybe_fix(&file.path, fix);
        agent_checks.push(AgentFileCheck {
            agent: file.agent_type.display_name().to_string(),
            path: file.path.display().to_string(),
            status,
            message,
        });
    }

    // Check global agent files
    for dir in &global_dirs {
        if let Some(instruction_path) = get_global_instruction_file_path(dir) {
            if instruction_path.exists() {
                let (status, message) = check_and_maybe_fix(&instruction_path, fix);
                agent_checks.push(AgentFileCheck {
                    agent: format!("{} (global)", dir.agent_type.display_name()),
                    path: instruction_path.display().to_string(),
                    status,
                    message,
                });
            }
        }
    }

    let has_unfixed_errors = agent_checks
        .iter()
        .any(|a| matches!(a.status, DiagnosticStatus::Error));

    let output = DoctorOutput {
        diagnostics,
        agent_checks,
        has_unfixed_errors,
    };

    println!("{}", output.format(cli_format));
    Ok(())
}

/// Check a single agent file for granary instructions, optionally fixing it.
fn check_and_maybe_fix(path: &std::path::Path, fix: bool) -> (DiagnosticStatus, Option<String>) {
    match contains_granary_instruction(path) {
        Ok(true) => (DiagnosticStatus::Ok, None),
        Ok(false) => {
            if fix {
                match inject_granary_instruction(path) {
                    Ok(InjectionResult::Injected) => (
                        DiagnosticStatus::Fix,
                        Some("added granary instruction".to_string()),
                    ),
                    Ok(_) => (
                        DiagnosticStatus::Error,
                        Some("missing \"use granary\"".to_string()),
                    ),
                    Err(e) => (DiagnosticStatus::Error, Some(format!("fix failed: {}", e))),
                }
            } else {
                (
                    DiagnosticStatus::Error,
                    Some("missing \"use granary\"".to_string()),
                )
            }
        }
        Err(e) => (DiagnosticStatus::Error, Some(e.to_string())),
    }
}
