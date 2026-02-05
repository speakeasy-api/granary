//! CLI handler for the initiate command - agent-friendly initiative planning

use crate::error::Result;
use crate::models::CreateInitiative;
use crate::services::{self, Workspace};

/// Handle the initiate command - creates an initiative and outputs guidance for project planning
pub async fn initiate(name: &str, description: Option<String>) -> Result<()> {
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

    // Output agent guidance
    print_initiative_guidance(&initiative.id, name, description.as_deref());

    Ok(())
}

fn print_initiative_guidance(initiative_id: &str, name: &str, description: Option<&str>) {
    println!("# Initiative: {}", name);
    println!();
    println!("ID: {}", initiative_id);

    if let Some(desc) = description {
        println!();
        println!("## Description");
        println!();
        println!("{}", desc);
    }

    println!();
    println!("## Planning Multi-Project Initiatives");
    println!();
    println!("As the initiative planning agent, you should:");
    println!(
        "1. Analyze the initiative scope and break it into discrete projects, with explicit boundaries"
    );
    println!("2. Create each project with a comprehensive description");
    println!("3. Launch sub-agents to plan each project in parallel");
    println!();

    println!("## Step 1: Create Projects");
    println!();
    println!("For each component of the initiative, create a project:");
    println!();
    println!(
        r#"  granary projects create "Project name" --description "
  Comprehensive description that explains:
  - What this project accomplishes
  - How it fits into the overall initiative
  - Key requirements and constraints
  - Expected deliverables
  ""#
    );
    println!();
    println!("Then add each project to this initiative:");
    println!();
    println!(
        "  granary initiative {} add-project <project-id>",
        initiative_id
    );
    println!();

    println!("## Step 2: Set Project Dependencies");
    println!();
    println!("If projects depend on each other:");
    println!();
    println!("  granary project <project-id> deps add <depends-on-project-id>");
    println!();

    println!("## Step 3: Launch Sub-Agents for Planning");
    println!();
    println!("For each project, launch a sub-agent with:");
    println!();
    println!("  granary plan --project <project-id>");
    println!();
    println!("The sub-agent will receive the project context and create tasks.");
    println!("Run sub-agents in parallel for independent projects.");
    println!();

    println!("## Step 4: Verify Initiative");
    println!();
    println!("After sub-agents complete planning:");
    println!();
    println!(
        "  granary initiative {} graph     # View project dependencies",
        initiative_id
    );
    println!(
        "  granary initiative {} summary   # View overall status",
        initiative_id
    );
    println!();

    println!("## Example Workflow");
    println!();
    println!("```");
    println!("# Create projects for \"{}\"", name);
    println!("granary projects create \"Backend API\" --description \"API implementation...\"");
    println!("# -> project-id: backend-api-abc1");
    println!();
    println!("granary projects create \"Frontend UI\" --description \"UI components...\"");
    println!("# -> project-id: frontend-ui-def2");
    println!();
    println!("# Add to initiative");
    println!(
        "granary initiative {} add-project backend-api-abc1",
        initiative_id
    );
    println!(
        "granary initiative {} add-project frontend-ui-def2",
        initiative_id
    );
    println!();
    println!("# Set dependencies (frontend depends on backend)");
    println!("granary project frontend-ui-def2 deps add backend-api-abc1");
    println!();
    println!("# Launch sub-agents (in parallel for independent projects)");
    println!("# Sub-agent 1: granary plan --project backend-api-abc1");
    println!("# Sub-agent 2: granary plan --project frontend-ui-def2");
    println!("```");
}
