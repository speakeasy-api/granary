use crate::cli::args::CliOutputFormat;
use crate::cli::workspace;
use crate::error::Result;
use crate::services::Workspace;

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

/// Run diagnostic checks
pub async fn doctor() -> Result<()> {
    let workspace = Workspace::find()?;
    let results = workspace.doctor().await?;

    println!("Granary Doctor");
    println!("==============");
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
