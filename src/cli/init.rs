use crate::error::Result;
use crate::services::{
    InjectionResult, Workspace, find_global_agent_dirs, find_workspace_agent_files,
    get_global_instruction_file_path, global_config_service, inject_granary_instruction,
    inject_or_create_instruction,
};

/// Initialize a new workspace
pub async fn init() -> Result<()> {
    // Check if this is the first run BEFORE creating the ~/.granary directory
    let first_run = global_config_service::is_first_run()?;

    let workspace = Workspace::find_or_create(None)?;
    let _pool = workspace.init_db().await?;

    // Find all agent instruction files in workspace
    let agent_files = find_workspace_agent_files(&workspace.root)?;

    // Inject instruction into each file that doesn't have it
    for file in agent_files {
        match inject_granary_instruction(&file.path)? {
            InjectionResult::Injected => {
                println!("Added granary instruction to {}", file.path.display());
            }
            InjectionResult::AlreadyExists => {
                // Skip silently
            }
            _ => {}
        }
    }

    // On first run, also inject into global agent directories
    if first_run {
        let global_dirs = find_global_agent_dirs()?;
        for dir in global_dirs {
            if let Some(instruction_path) = get_global_instruction_file_path(&dir) {
                match inject_or_create_instruction(&instruction_path)? {
                    InjectionResult::Injected | InjectionResult::FileCreated => {
                        println!(
                            "Added granary instruction to {}",
                            instruction_path.display()
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    println!(
        "Initialized Granary workspace at {}",
        workspace.root.display()
    );
    println!("Database: {}", workspace.db_path.display());

    Ok(())
}

/// Run diagnostic checks
pub async fn doctor() -> Result<()> {
    let workspace = Workspace::find()?;
    let results = workspace.doctor().await?;

    println!("Granary Doctor");
    println!("==============");
    println!();
    println!("Workspace: {}", workspace.root.display());
    println!();

    for result in results {
        println!(
            "{:8} {}: {}",
            result.status_symbol(),
            result.check,
            result.message
        );
    }

    Ok(())
}
