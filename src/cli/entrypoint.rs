use serde::Serialize;

use crate::cli::args::CliOutputFormat;
use crate::error::Result;
use crate::output::Output;
use crate::services::Workspace;

#[derive(Serialize)]
pub struct CommandHint {
    pub label: String,
    pub command: String,
}

pub struct EntrypointOutput {
    pub initialized: bool,
    pub hints: Vec<CommandHint>,
}

impl Output for EntrypointOutput {
    fn to_json(&self) -> String {
        serde_json::json!({
            "initialized": self.initialized,
            "hints": self.hints.iter().map(|h| {
                serde_json::json!({
                    "label": h.label,
                    "command": h.command,
                })
            }).collect::<Vec<_>>(),
        })
        .to_string()
    }

    fn to_prompt(&self) -> String {
        if !self.initialized {
            return "Granary is not initialized in this workspace. Run `granary init` to set up."
                .to_string();
        }

        let mut lines = vec!["Available granary workflows:".to_string()];
        for hint in &self.hints {
            lines.push(format!("- {}: `{}`", hint.label, hint.command));
        }
        lines.join("\n")
    }

    fn to_text(&self) -> String {
        if !self.initialized {
            return "Not initialized. Run: granary init".to_string();
        }

        let mut lines = Vec::new();
        for (i, hint) in self.hints.iter().enumerate() {
            if i > 0 {
                lines.push(String::new());
            }
            lines.push(format!("{}:", hint.label));
            lines.push(format!("  {}", hint.command));
        }
        lines.join("\n")
    }
}

/// Show LLM-friendly entry point guidance
pub async fn show_entry_point(cli_format: Option<CliOutputFormat>) -> Result<()> {
    let output = match Workspace::find() {
        Ok(_) => EntrypointOutput {
            initialized: true,
            hints: vec![
                CommandHint {
                    label: "Plan a feature".to_string(),
                    command: "granary plan \"Feature name\"".to_string(),
                },
                CommandHint {
                    label: "Plan multi-project work".to_string(),
                    command: "granary initiate \"Initiative name\"".to_string(),
                },
                CommandHint {
                    label: "Work on task".to_string(),
                    command: "granary work <task-id>".to_string(),
                },
                CommandHint {
                    label: "Search".to_string(),
                    command: "granary search \"keyword\"".to_string(),
                },
            ],
        },
        Err(_) => EntrypointOutput {
            initialized: false,
            hints: vec![],
        },
    };

    println!("{}", output.format(cli_format));
    Ok(())
}
