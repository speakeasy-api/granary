use crate::error::Result;
use crate::services::Workspace;

/// Initialize a new workspace
pub async fn init() -> Result<()> {
    let workspace = Workspace::find_or_create(None)?;
    let _pool = workspace.init_db().await?;

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
