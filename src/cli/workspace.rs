//! Workspace command group - manages workspace creation, listing, and info

use std::env;
use std::path::Path;

use serde::Serialize;

use crate::cli::args::{CliOutputFormat, NamedWorkspaceAction, WorkspaceAction};
use crate::error::{GranaryError, Result};
use crate::output::{Output, OutputType};
use crate::services::{
    InjectionResult, Workspace, WorkspaceMode, WorkspaceRegistry, find_global_agent_dirs,
    find_workspace_agent_files, get_global_instruction_file_path, global_config_service,
    inject_granary_instruction, inject_or_create_instruction,
};

// ── Workspace Info Output ──────────────────────────────────────────────

pub struct WorkspaceInfoOutput {
    pub name: String,
    pub mode: String,
    pub db_path: String,
    pub root: Option<String>,
}

impl Output for WorkspaceInfoOutput {
    fn output_type() -> OutputType {
        OutputType::Text
    }

    fn to_json(&self) -> String {
        let json = WorkspaceInfoJson {
            name: &self.name,
            mode: &self.mode,
            database: &self.db_path,
            root: self.root.as_deref(),
        };
        serde_json::to_string_pretty(&json).unwrap_or_else(|_| "{}".to_string())
    }

    fn to_prompt(&self) -> String {
        let mut out = format!(
            "Workspace: {}\nMode: {}\nDatabase: {}",
            self.name, self.mode, self.db_path
        );
        if let Some(ref root) = self.root {
            out.push_str(&format!("\nRoot: {} (matched from registry)", root));
        }
        out
    }

    fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Workspace: {}\n", self.name));
        out.push_str(&format!("Mode:      {}\n", self.mode));
        out.push_str(&format!("Database:  {}", self.db_path));
        if let Some(ref root) = self.root {
            out.push_str(&format!("\nRoot:      {} (matched from registry)", root));
        }
        out
    }
}

#[derive(Serialize)]
struct WorkspaceInfoJson<'a> {
    name: &'a str,
    mode: &'a str,
    database: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    root: Option<&'a str>,
}

// ── Workspace List Output ──────────────────────────────────────────────

pub struct WorkspaceListOutput {
    pub workspaces: Vec<WorkspaceListEntry>,
}

pub struct WorkspaceListEntry {
    pub name: String,
    pub mode: String,
    pub database: String,
    pub roots: Vec<String>,
}

impl Output for WorkspaceListOutput {
    fn output_type() -> OutputType {
        OutputType::Text
    }

    fn to_json(&self) -> String {
        let entries: Vec<WorkspaceListJson> = self
            .workspaces
            .iter()
            .map(|e| WorkspaceListJson {
                name: &e.name,
                mode: &e.mode,
                database: &e.database,
                roots: &e.roots,
            })
            .collect();
        serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
    }

    fn to_prompt(&self) -> String {
        self.to_text()
    }

    fn to_text(&self) -> String {
        if self.workspaces.is_empty() {
            return "No workspaces found.".to_string();
        }

        // Calculate column widths
        let name_width = self
            .workspaces
            .iter()
            .map(|w| w.name.len())
            .max()
            .unwrap_or(4)
            .max(4);
        let mode_width = self
            .workspaces
            .iter()
            .map(|w| w.mode.len())
            .max()
            .unwrap_or(4)
            .max(4);
        let db_width = self
            .workspaces
            .iter()
            .map(|w| w.database.len())
            .max()
            .unwrap_or(8)
            .max(8);

        let mut out = format!(
            "{:<name_width$}  {:<mode_width$}  {:<db_width$}  ROOTS\n",
            "NAME", "MODE", "DATABASE",
        );

        for ws in &self.workspaces {
            let roots_str = if ws.roots.is_empty() {
                if ws.mode == "default" {
                    "(all unmatched)".to_string()
                } else {
                    "(none)".to_string()
                }
            } else {
                ws.roots.join(", ")
            };

            out.push_str(&format!(
                "{:<name_width$}  {:<mode_width$}  {:<db_width$}  {}\n",
                ws.name, ws.mode, ws.database, roots_str,
            ));
        }

        out.trim_end().to_string()
    }
}

#[derive(Serialize)]
struct WorkspaceListJson<'a> {
    name: &'a str,
    mode: &'a str,
    database: &'a str,
    roots: &'a [String],
}

// ── Workspace Init Output ──────────────────────────────────────────────

pub struct WorkspaceInitOutput {
    pub message: String,
}

impl Output for WorkspaceInitOutput {
    fn output_type() -> OutputType {
        OutputType::Text
    }

    fn to_json(&self) -> String {
        serde_json::json!({"status": "initialized", "message": &self.message}).to_string()
    }

    fn to_prompt(&self) -> String {
        self.message.clone()
    }

    fn to_text(&self) -> String {
        self.message.clone()
    }
}

// ── Command Handlers ───────────────────────────────────────────────────

/// Handle `granary workspace [action]`
pub async fn workspace(
    action: Option<WorkspaceAction>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    match action {
        None => workspace_info(cli_format).await,
        Some(WorkspaceAction::Init {
            local,
            force,
            skip_git_check,
            name,
        }) => workspace_init(local, force, skip_git_check, name, cli_format).await,
        Some(WorkspaceAction::List) => workspace_list(cli_format).await,
        Some(WorkspaceAction::Named(args)) => {
            let (name, action) =
                NamedWorkspaceAction::parse(&args).map_err(GranaryError::InvalidArgument)?;
            match action {
                NamedWorkspaceAction::Info => workspace_info(cli_format).await,
                NamedWorkspaceAction::Add => workspace_add(&name, cli_format).await,
                NamedWorkspaceAction::Remove => workspace_remove(&name, cli_format).await,
                NamedWorkspaceAction::Move { target } => {
                    workspace_move(&name, &target, cli_format).await
                }
                NamedWorkspaceAction::Migrate {
                    global,
                    local: _,
                    name: migrate_name,
                } => {
                    if global {
                        workspace_migrate_to_global(&name, migrate_name.as_deref(), cli_format)
                            .await
                    } else {
                        workspace_migrate_to_local(&name, cli_format).await
                    }
                }
            }
        }
    }
}

/// `granary workspace` (no args) - show workspace info for cwd
async fn workspace_info(cli_format: Option<CliOutputFormat>) -> Result<()> {
    let workspace = Workspace::find()?;

    let output = WorkspaceInfoOutput {
        name: workspace.display_name(),
        mode: workspace.mode.label().to_string(),
        db_path: workspace.db_path.display().to_string(),
        root: workspace.matched_root(),
    };

    println!("{}", output.format(cli_format));
    Ok(())
}

/// `granary workspace init` - create a named or local workspace
pub async fn workspace_init(
    local: bool,
    force: bool,
    skip_git_check: bool,
    name: Option<String>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let cwd = env::current_dir()?;

    // Validation checks
    run_init_validations(&cwd, local, force, skip_git_check)?;

    if local {
        init_local(&cwd, cli_format).await
    } else {
        init_global(&cwd, name, cli_format).await
    }
}

/// Run validation checks for workspace init (both local and global)
fn run_init_validations(cwd: &Path, local: bool, force: bool, skip_git_check: bool) -> Result<()> {
    let home = dirs::home_dir();

    // 1. Already initialized locally?
    let local_granary = cwd.join(".granary");
    if local_granary.exists() && !force {
        if local {
            return Err(GranaryError::LocalWorkspaceExistsLocal);
        } else {
            return Err(GranaryError::LocalWorkspaceExistsGlobal);
        }
    }

    // 2. Parent workspace exists?
    if !force {
        let mut current = cwd.parent();
        while let Some(dir) = current {
            // Stop before $HOME
            if home.as_ref().is_some_and(|h| dir == h) {
                break;
            }
            if dir.join(".granary").exists() {
                return Err(GranaryError::NestedWorkspace(
                    dir.join(".granary").display().to_string(),
                ));
            }
            current = dir.parent();
        }
    }

    // 3. Git directory check
    if !skip_git_check {
        let git_in_cwd = cwd.join(".git").exists();
        if !git_in_cwd {
            // Check if .git exists in a parent
            let mut current = cwd.parent();
            while let Some(dir) = current {
                if home.as_ref().is_some_and(|h| dir == h) {
                    break;
                }
                if dir.join(".git").exists() {
                    return Err(GranaryError::NotGitRoot(dir.display().to_string()));
                }
                current = dir.parent();
            }
            // No .git found anywhere - that's fine, not a git project
        }
    }

    Ok(())
}

/// Initialize a local workspace (.granary/ in cwd)
async fn init_local(cwd: &Path, cli_format: Option<CliOutputFormat>) -> Result<()> {
    // Check if this is the first run BEFORE creating the ~/.granary directory
    let first_run = global_config_service::is_first_run()?;

    let workspace = Workspace::find_or_create(Some(cwd))?;
    let _pool = workspace.init_db().await?;

    inject_agent_instructions(&workspace, first_run)?;

    let output = WorkspaceInitOutput {
        message: format!(
            "Initialized local workspace at {}\nDatabase: {}",
            workspace.granary_dir.display(),
            workspace.db_path.display()
        ),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

/// Initialize a global named workspace
async fn init_global(
    cwd: &Path,
    name: Option<String>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    // Check if this is the first run BEFORE creating the ~/.granary directory
    let first_run = global_config_service::is_first_run()?;

    // Derive workspace name
    let ws_name = derive_workspace_name(cwd, name.as_deref())?;

    // Create workspace in registry
    let mut registry = WorkspaceRegistry::load()?;

    // If name already exists, append a short random suffix
    let final_name = if registry.workspaces.contains_key(&ws_name) {
        let suffix = nanoid::nanoid!(3, &nanoid::alphabet::SAFE);
        format!("{}-{}", ws_name, suffix)
    } else {
        ws_name
    };

    registry.create_workspace(&final_name)?;
    registry.add_root(cwd.to_path_buf(), &final_name)?;
    registry.save()?;

    // Create and initialize the database
    let db_path = WorkspaceRegistry::workspace_db_path(&final_name)?;
    let granary_dir = db_path.parent().unwrap().to_path_buf();

    let workspace = Workspace {
        name: Some(final_name.clone()),
        root: cwd.to_path_buf(),
        granary_dir,
        db_path,
        mode: WorkspaceMode::Named(final_name.clone()),
    };
    let _pool = workspace.init_db().await?;

    inject_agent_instructions(&workspace, first_run)?;

    let config_dir = global_config_service::config_dir()?;
    let display_path = format!("~/.granary/workspaces/{}/", final_name);
    let _ = config_dir; // suppress unused warning

    let output = WorkspaceInitOutput {
        message: format!(
            "Initialized workspace \"{}\" at {}",
            final_name, display_path
        ),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

/// Derive workspace name from --name flag or from directory name
fn derive_workspace_name(cwd: &Path, name: Option<&str>) -> Result<String> {
    if let Some(n) = name {
        return Ok(n.to_string());
    }

    cwd.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| {
            GranaryError::InvalidArgument(
                "Cannot derive workspace name from directory. Use --name to specify one."
                    .to_string(),
            )
        })
}

/// Inject granary instructions into agent files
fn inject_agent_instructions(workspace: &Workspace, first_run: bool) -> Result<()> {
    let agent_files = find_workspace_agent_files(&workspace.root)?;
    for file in agent_files {
        match inject_granary_instruction(&file.path)? {
            InjectionResult::Injected => {
                println!("Added granary instruction to {}", file.path.display());
            }
            InjectionResult::AlreadyExists => {}
            _ => {}
        }
    }

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

    Ok(())
}

/// `granary workspace list` / `granary workspaces` - list all workspaces
pub async fn workspace_list(cli_format: Option<CliOutputFormat>) -> Result<()> {
    let config_dir = global_config_service::config_dir()?;
    let registry = WorkspaceRegistry::load()?;

    let mut entries = Vec::new();

    // Always show the default workspace
    let default_db = config_dir.join("granary.db");
    entries.push(WorkspaceListEntry {
        name: "default".to_string(),
        mode: "default".to_string(),
        database: default_db.display().to_string(),
        roots: Vec::new(),
    });

    // List named workspaces from registry
    let mut ws_list = registry.list_workspaces();
    ws_list.sort_by(|a, b| a.0.cmp(b.0));
    for (name, _meta, roots) in ws_list {
        let db_path = WorkspaceRegistry::workspace_db_path(name)?;
        let root_strings: Vec<String> = roots.iter().map(|p| p.display().to_string()).collect();

        entries.push(WorkspaceListEntry {
            name: name.to_string(),
            mode: "named".to_string(),
            database: db_path.display().to_string(),
            roots: root_strings,
        });
    }

    // If cwd is inside a local workspace, show it too
    let cwd = env::current_dir()?;
    if let Some(local_entry) = find_local_workspace_entry(&cwd, &config_dir) {
        entries.push(local_entry);
    }

    let output = WorkspaceListOutput {
        workspaces: entries,
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

// ── Workspace Add/Remove/Move Output ──────────────────────────────────

pub struct WorkspaceActionOutput {
    pub message: String,
}

impl Output for WorkspaceActionOutput {
    fn output_type() -> OutputType {
        OutputType::Text
    }

    fn to_json(&self) -> String {
        serde_json::json!({"status": "ok", "message": &self.message}).to_string()
    }

    fn to_prompt(&self) -> String {
        self.message.clone()
    }

    fn to_text(&self) -> String {
        self.message.clone()
    }
}

// ── Workspace Add ─────────────────────────────────────────────────────

/// `granary workspace <name> add` - adds cwd to the named workspace
async fn workspace_add(name: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let cwd = env::current_dir()?;
    let mut registry = WorkspaceRegistry::load()?;

    // Check workspace exists
    if !registry.workspaces.contains_key(name) {
        return Err(GranaryError::InvalidArgument(format!(
            "Workspace \"{}\" does not exist.",
            name
        )));
    }

    // Check if cwd is already registered to any workspace
    if let Some(existing) = registry.roots.get(&cwd) {
        return Err(GranaryError::DirectoryAlreadyRegistered {
            path: cwd.display().to_string(),
            workspace: existing.clone(),
        });
    }

    registry.add_root(cwd.clone(), name)?;
    registry.save()?;

    let output = WorkspaceActionOutput {
        message: format!("Added {} to workspace \"{}\".", cwd.display(), name),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

// ── Workspace Remove ──────────────────────────────────────────────────

/// `granary workspace <name> remove` - removes cwd from the named workspace
async fn workspace_remove(name: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let cwd = env::current_dir()?;
    let mut registry = WorkspaceRegistry::load()?;

    // Check that cwd is a root of the named workspace
    match registry.roots.get(&cwd) {
        Some(ws) if ws == name => {}
        _ => {
            return Err(GranaryError::NotWorkspaceRoot(format!(
                "{} is not a root of workspace \"{}\".",
                cwd.display(),
                name
            )));
        }
    }

    registry.remove_root(&cwd)?;
    registry.save()?;

    let output = WorkspaceActionOutput {
        message: format!("Removed {} from workspace \"{}\".", cwd.display(), name),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

// ── Workspace Move ────────────────────────────────────────────────────

/// `granary workspace <name> move <target>` - updates root path mapping
async fn workspace_move(
    name: &str,
    target: &Path,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let cwd = env::current_dir()?;
    let mut registry = WorkspaceRegistry::load()?;

    // Check that cwd is a root of the named workspace
    match registry.roots.get(&cwd) {
        Some(ws) if ws == name => {}
        _ => {
            return Err(GranaryError::NotWorkspaceRoot(format!(
                "{} is not a root of workspace \"{}\".",
                cwd.display(),
                name
            )));
        }
    }

    // Canonicalize target or use as-is if it doesn't exist yet
    let target_path = if target.exists() {
        target
            .canonicalize()
            .unwrap_or_else(|_| target.to_path_buf())
    } else {
        // Resolve relative to cwd
        if target.is_absolute() {
            target.to_path_buf()
        } else {
            cwd.join(target)
        }
    };

    // Check if target is already registered
    if let Some(existing) = registry.roots.get(&target_path) {
        return Err(GranaryError::DirectoryAlreadyRegistered {
            path: target_path.display().to_string(),
            workspace: existing.clone(),
        });
    }

    // Remove old root and add new one
    registry.remove_root(&cwd)?;
    registry.roots.insert(target_path.clone(), name.to_string());
    registry.save()?;

    let output = WorkspaceActionOutput {
        message: format!(
            "Updated workspace \"{}\": {} → {}",
            name,
            cwd.display(),
            target_path.display()
        ),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

// ── Workspace Migrate Output ──────────────────────────────────────────

pub struct WorkspaceMigrateOutput {
    pub message: String,
}

impl Output for WorkspaceMigrateOutput {
    fn output_type() -> OutputType {
        OutputType::Text
    }

    fn to_json(&self) -> String {
        serde_json::json!({"status": "migrated", "message": &self.message}).to_string()
    }

    fn to_prompt(&self) -> String {
        self.message.clone()
    }

    fn to_text(&self) -> String {
        self.message.clone()
    }
}

// ── Workspace Migrate: Local → Global ─────────────────────────────────

/// `granary workspace <name> migrate --global` - migrate local workspace to global named workspace
async fn workspace_migrate_to_global(
    _name: &str,
    migrate_name: Option<&str>,
    cli_format: Option<CliOutputFormat>,
) -> Result<()> {
    let cwd = env::current_dir()?;

    // 1. Find local .granary/granary.db in cwd
    let local_granary = cwd.join(".granary");
    let local_db = local_granary.join("granary.db");
    if !local_db.exists() {
        return Err(GranaryError::WorkspaceNotFound(
            "No local .granary/granary.db found in current directory.".to_string(),
        ));
    }

    // 2. Derive workspace name from --name flag or directory name
    let ws_name = derive_workspace_name(&cwd, migrate_name)?;

    // 3. Create workspace in registry (handle name collision)
    let mut registry = WorkspaceRegistry::load()?;

    let final_name = if registry.workspaces.contains_key(&ws_name) {
        let suffix = nanoid::nanoid!(3, &nanoid::alphabet::SAFE);
        format!("{}-{}", ws_name, suffix)
    } else {
        ws_name
    };

    registry.create_workspace(&final_name)?;
    registry.add_root(cwd.clone(), &final_name)?;
    registry.save()?;

    // 4. Copy database to ~/.granary/workspaces/<name>/granary.db
    let global_db = WorkspaceRegistry::workspace_db_path(&final_name)?;
    std::fs::copy(&local_db, &global_db)?;

    // 5. Remove .granary/ from cwd
    // Copy-then-delete: the database is already safely copied at this point
    std::fs::remove_dir_all(&local_granary)?;

    let output = WorkspaceMigrateOutput {
        message: format!(
            "Migrated local workspace to \"{}\". Local .granary/ removed.",
            final_name
        ),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

// ── Workspace Migrate: Global → Local ─────────────────────────────────

/// `granary workspace <name> migrate --local` - migrate global named workspace to local
async fn workspace_migrate_to_local(name: &str, cli_format: Option<CliOutputFormat>) -> Result<()> {
    let cwd = env::current_dir()?;

    // 1. Verify workspace exists and cwd is a root of it
    let mut registry = WorkspaceRegistry::load()?;

    if !registry.workspaces.contains_key(name) {
        return Err(GranaryError::InvalidArgument(format!(
            "Workspace \"{}\" does not exist.",
            name
        )));
    }

    match registry.roots.get(&cwd) {
        Some(ws) if ws == name => {}
        _ => {
            return Err(GranaryError::NotWorkspaceRoot(format!(
                "{} is not a root of workspace \"{}\".",
                cwd.display(),
                name
            )));
        }
    }

    // 2. Copy database to ./.granary/granary.db
    let global_db = WorkspaceRegistry::workspace_db_path(name)?;
    if !global_db.exists() {
        return Err(GranaryError::WorkspaceNotFound(format!(
            "Workspace database not found at {}",
            global_db.display()
        )));
    }

    let local_granary = cwd.join(".granary");
    std::fs::create_dir_all(&local_granary)?;
    let local_db = local_granary.join("granary.db");
    std::fs::copy(&global_db, &local_db)?;

    // 3. Remove cwd from the roots map in the registry
    registry.remove_root(&cwd)?;
    registry.save()?;

    // 4. If workspace has no remaining roots, clean up the workspace directory
    let has_remaining_roots = registry.roots.values().any(|ws| ws == name);
    if !has_remaining_roots {
        let ws_dir = global_db.parent().unwrap();
        if ws_dir.exists() {
            std::fs::remove_dir_all(ws_dir)?;
        }
        // Also remove the workspace entry from the registry
        registry.workspaces.remove(name);
        registry.save()?;
    }

    let output = WorkspaceMigrateOutput {
        message: format!(
            "Migrated workspace \"{}\" to local .granary/. Removed from registry.",
            name
        ),
    };
    println!("{}", output.format(cli_format));
    Ok(())
}

/// Check if cwd is inside a local .granary/ workspace (not the global one)
fn find_local_workspace_entry(cwd: &Path, config_dir: &Path) -> Option<WorkspaceListEntry> {
    let home = dirs::home_dir();
    let mut current = Some(cwd);

    while let Some(dir) = current {
        // Stop before $HOME to avoid picking up ~/.granary
        if home.as_ref().is_some_and(|h| dir == h) {
            break;
        }

        let granary_dir = dir.join(".granary");
        if granary_dir.exists() && granary_dir.is_dir() {
            // Make sure this isn't the global config dir
            if granary_dir != *config_dir {
                let db_path = granary_dir.join("granary.db");
                let name = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir.display().to_string());
                return Some(WorkspaceListEntry {
                    name: format!("{} (local)", name),
                    mode: "local".to_string(),
                    database: db_path.display().to_string(),
                    roots: vec![dir.display().to_string()],
                });
            }
        }

        current = dir.parent();
    }

    None
}
