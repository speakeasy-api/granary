//! CLI handler for the initiate command - agent-friendly initiative planning

use serde::Serialize;

use crate::cli::args::CliOutputFormat;
use crate::error::Result;
use crate::models::CreateInitiative;
use crate::output::{Output, OutputType};
use crate::services::{self, Workspace};

/// Output for the initiate command
pub struct InitiateOutput {
    pub initiative_id: String,
    pub name: String,
    pub description: Option<String>,
}

impl Output for InitiateOutput {
    fn output_type() -> OutputType {
        OutputType::Prompt // LLM-first command
    }

    fn to_json(&self) -> String {
        let json_output = InitiateJsonOutput {
            initiative_id: &self.initiative_id,
            name: &self.name,
            description: self.description.as_deref(),
        };
        serde_json::to_string_pretty(&json_output).unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        format_initiative_guidance(&self.initiative_id, &self.name, self.description.as_deref())
    }

    fn to_text(&self) -> String {
        format!("Initiative created: {}", self.initiative_id)
    }
}

#[derive(Serialize)]
struct InitiateJsonOutput<'a> {
    initiative_id: &'a str,
    name: &'a str,
    description: Option<&'a str>,
}

/// Handle the initiate command - creates an initiative and outputs guidance for project planning
pub async fn initiate(
    name: &str,
    description: Option<String>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let workspace = Workspace::find()?;
    let pool = workspace.pool().await?;

    // Create the initiative
    let initiative = services::create_initiative(
        &pool,
        CreateInitiative {
            name: name.to_string(),
            description: description.clone(),
            owner: None,
            tags: vec![],
        },
    )
    .await?;

    // Output guidance
    let output = InitiateOutput {
        initiative_id: initiative.id,
        name: name.to_string(),
        description,
    };
    println!("{}", output.format(cli_format));

    Ok(())
}

fn format_initiative_guidance(
    initiative_id: &str,
    name: &str,
    description: Option<&str>,
) -> String {
    let mut output = String::new();

    output.push_str(&format!("# Initiative: {}\n\n", name));
    output.push_str(&format!("ID: {}\n", initiative_id));

    if let Some(desc) = description {
        output.push_str("\n## Description\n\n");
        output.push_str(desc);
        output.push('\n');
    }

    output.push_str("\n## Planning Multi-Project Initiatives\n\n");
    output.push_str("As the initiative planning agent, you should:\n");
    output.push_str(
        "1. Analyze the initiative scope and break it into discrete projects, with explicit boundaries\n",
    );
    output.push_str("2. Create each project with a comprehensive description\n");
    output.push_str("3. Launch sub-agents to plan each project in parallel\n\n");

    output.push_str("## Step 1: Create Projects\n\n");
    output.push_str("For each component of the initiative, create a project:\n\n");
    output.push_str(
        r#"  granary projects create "Project name" --description "
  Comprehensive description that explains:
  - What this project accomplishes
  - How it fits into the overall initiative
  - Key requirements and constraints
  - Expected deliverables
  "
"#,
    );
    output.push_str("\nThen add each project to this initiative:\n\n");
    output.push_str(&format!(
        "  granary initiative {} add-project <project-id>\n\n",
        initiative_id
    ));

    output.push_str("## Step 2: Set Project Dependencies\n\n");
    output.push_str("If projects depend on each other:\n\n");
    output.push_str("  granary project <project-id> deps add <depends-on-project-id>\n\n");

    output.push_str("## Step 3: Launch Sub-Agents for Planning\n\n");
    output.push_str("For each project, launch a sub-agent with:\n\n");
    output.push_str("  granary plan --project <project-id>\n\n");
    output.push_str("The sub-agent will receive the project context and create tasks.\n");
    output.push_str("Run sub-agents in parallel for independent projects.\n\n");

    output.push_str("## Step 4: Verify Initiative\n\n");
    output.push_str("After sub-agents complete planning:\n\n");
    output.push_str(&format!(
        "  granary initiative {} graph     # View project dependencies\n",
        initiative_id
    ));
    output.push_str(&format!(
        "  granary initiative {} summary   # View overall status\n\n",
        initiative_id
    ));

    output.push_str("## Example Workflow\n\n");
    output.push_str("```\n");
    output.push_str(&format!("# Create projects for \"{}\"\n", name));
    output.push_str(
        "granary projects create \"Backend API\" --description \"API implementation...\"\n",
    );
    output.push_str("# -> project-id: backend-api-abc1\n\n");
    output.push_str("granary projects create \"Frontend UI\" --description \"UI components...\"\n");
    output.push_str("# -> project-id: frontend-ui-def2\n\n");
    output.push_str("# Add to initiative\n");
    output.push_str(&format!(
        "granary initiative {} add-project backend-api-abc1\n",
        initiative_id
    ));
    output.push_str(&format!(
        "granary initiative {} add-project frontend-ui-def2\n\n",
        initiative_id
    ));
    output.push_str("# Set dependencies (frontend depends on backend)\n");
    output.push_str("granary project frontend-ui-def2 deps add backend-api-abc1\n\n");
    output.push_str("# Launch sub-agents (in parallel for independent projects)\n");
    output.push_str("# Sub-agent 1: granary plan --project backend-api-abc1\n");
    output.push_str("# Sub-agent 2: granary plan --project frontend-ui-def2\n");
    output.push_str("```");

    output
}
