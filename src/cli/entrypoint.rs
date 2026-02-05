use crate::error::Result;
use crate::services::Workspace;

/// Show LLM-friendly entry point guidance
pub async fn show_entry_point() -> Result<()> {
    // Check if workspace exists
    match Workspace::find() {
        Ok(_) => {
            // Workspace exists - show workflow options
            println!("Plan a feature:");
            println!("  granary plan \"Feature name\"");
            println!();
            println!("Plan multi-project work:");
            println!("  granary initiate \"Initiative name\"");
            println!();
            println!("Work on task:");
            println!("  granary work <task-id>");
            println!();
            println!("Search:");
            println!("  granary search \"keyword\"");
        }
        Err(_) => {
            // Workspace not found
            println!("Not initialized. Run: granary init");
        }
    }

    Ok(())
}
