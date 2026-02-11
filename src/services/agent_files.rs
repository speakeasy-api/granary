//! Agent instruction file detection module.
//!
//! Detects and lists all known AI coding agent instruction files in a given directory.
//! Supports all major AI coding assistants including Claude Code, GitHub Copilot,
//! Cursor, Windsurf, Cline, and many others.

use crate::error::Result;
use std::path::{Path, PathBuf};

/// The granary instruction to inject into agent files.
const GRANARY_INSTRUCTION: &str = r#"IMPORTANT: when user requests to "use granary", run `granary` command before performing any other task."#;

/// Result of attempting to inject a granary instruction into an agent file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectionResult {
    /// Instruction was successfully injected into an existing file.
    Injected,
    /// The file already contains the granary instruction.
    AlreadyExists,
    /// A new file was created with the instruction.
    FileCreated,
    /// An error occurred during injection.
    Error(String),
}

/// Represents the type of AI coding agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentType {
    /// Claude Code (Anthropic) - CLAUDE.md
    ClaudeCode,
    /// Unified AGENTS.md standard (OpenAI Codex, Gemini CLI, Factory AI, Cursor, Roo Code)
    UnifiedAgents,
    /// OpenAI Codex legacy - CODEX.md
    Codex,
    /// Gemini CLI (Google) - GEMINI.md
    Gemini,
    /// Google Jules - JULES.md
    Jules,
    /// Kiro (AWS) - POWER.md or .kiro/rules.md
    Kiro,
    /// Cursor - .cursorrules or .cursor/rules/*.md
    Cursor,
    /// Windsurf - .windsurfrules or .windsurf/rules/rules.md
    Windsurf,
    /// Cline - .clinerules or .clinerules/*.md
    Cline,
    /// Roo Code - .roorules or .roo/rules/*.md
    RooCode,
    /// Continue.dev - .continue/agents/*.md or .continue/config.json
    Continue,
    /// Aider - CONVENTIONS.md or .aider.conf.yml
    Aider,
    /// Augment Code - .augment-guidelines or .augment/rules/*.md
    Augment,
    /// Tabnine - .tabnine/guidelines/*.md
    Tabnine,
    /// OpenCode - .opencode/agent/*.md
    OpenCode,
    /// Void - .void/rules/*.md
    Void,
    /// GitHub Copilot - .github/copilot-instructions.md
    Copilot,
    /// JetBrains Junie - .junie/guidelines.md
    Junie,
    /// Zed - .rules
    Zed,
}

impl AgentType {
    /// Returns a human-readable name for the agent type.
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentType::ClaudeCode => "Claude Code",
            AgentType::UnifiedAgents => "AGENTS.md (Unified Standard)",
            AgentType::Codex => "OpenAI Codex",
            AgentType::Gemini => "Gemini CLI",
            AgentType::Jules => "Google Jules",
            AgentType::Kiro => "Kiro (AWS)",
            AgentType::Cursor => "Cursor",
            AgentType::Windsurf => "Windsurf",
            AgentType::Cline => "Cline",
            AgentType::RooCode => "Roo Code",
            AgentType::Continue => "Continue.dev",
            AgentType::Aider => "Aider",
            AgentType::Augment => "Augment Code",
            AgentType::Tabnine => "Tabnine",
            AgentType::OpenCode => "OpenCode",
            AgentType::Void => "Void",
            AgentType::Copilot => "GitHub Copilot",
            AgentType::Junie => "JetBrains Junie",
            AgentType::Zed => "Zed",
        }
    }
}

/// Represents a detected agent instruction file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFile {
    /// The path to the agent instruction file.
    pub path: PathBuf,
    /// The type of AI coding agent this file belongs to.
    pub agent_type: AgentType,
}

impl AgentFile {
    /// Creates a new AgentFile.
    pub fn new(path: PathBuf, agent_type: AgentType) -> Self {
        Self { path, agent_type }
    }
}

/// Single-file agent instruction patterns at workspace root.
const SINGLE_FILE_PATTERNS: &[(&str, AgentType)] = &[
    ("CLAUDE.md", AgentType::ClaudeCode),
    ("AGENTS.md", AgentType::UnifiedAgents),
    ("CODEX.md", AgentType::Codex),
    ("GEMINI.md", AgentType::Gemini),
    ("JULES.md", AgentType::Jules),
    ("POWER.md", AgentType::Kiro),
    (".cursorrules", AgentType::Cursor),
    (".windsurfrules", AgentType::Windsurf),
    (".clinerules", AgentType::Cline),
    (".roorules", AgentType::RooCode),
    (".augment-guidelines", AgentType::Augment),
    ("CONVENTIONS.md", AgentType::Aider),
    (".rules", AgentType::Zed),
];

/// Config files to detect (but may not inject into).
const CONFIG_FILE_PATTERNS: &[(&str, AgentType)] = &[
    (".aider.conf.yml", AgentType::Aider),
    (".continue/config.json", AgentType::Continue),
];

/// Directory-based agent instruction patterns.
/// Format: (directory_path, file_pattern, agent_type)
/// file_pattern can be:
/// - A specific filename (e.g., "rules.md")
/// - "*.md" for all markdown files in the directory
const DIRECTORY_PATTERNS: &[(&str, &str, AgentType)] = &[
    (".cursor/rules", "*.md", AgentType::Cursor),
    (".windsurf/rules", "rules.md", AgentType::Windsurf),
    (".clinerules", "*.md", AgentType::Cline),
    (".roo/rules", "*.md", AgentType::RooCode),
    (".continue/agents", "*.md", AgentType::Continue),
    (".augment/rules", "*.md", AgentType::Augment),
    (".tabnine/guidelines", "*.md", AgentType::Tabnine),
    (".opencode/agent", "*.md", AgentType::OpenCode),
    (".void/rules", "*.md", AgentType::Void),
    (".kiro", "rules.md", AgentType::Kiro),
];

/// IDE-specific agent instruction patterns.
const IDE_PATTERNS: &[(&str, AgentType)] = &[
    (".github/copilot-instructions.md", AgentType::Copilot),
    (".junie/guidelines.md", AgentType::Junie),
];

/// Finds all agent instruction files in the given workspace root directory.
///
/// This function scans for:
/// - Single-file patterns (e.g., CLAUDE.md, .cursorrules)
/// - Directory-based patterns (e.g., .cursor/rules/*.md)
/// - IDE-specific patterns (e.g., .github/copilot-instructions.md)
/// - Config files (e.g., .aider.conf.yml)
///
/// # Arguments
/// * `root` - The workspace root directory to scan
///
/// # Returns
/// A vector of detected agent files, or an error if the directory cannot be read.
pub fn find_workspace_agent_files(root: &Path) -> Result<Vec<AgentFile>> {
    let mut agent_files = Vec::new();

    // Check single-file patterns
    for (filename, agent_type) in SINGLE_FILE_PATTERNS {
        let path = root.join(filename);
        // For .clinerules, it could be a file or a directory
        if *filename == ".clinerules" {
            if path.is_file() {
                agent_files.push(AgentFile::new(path, *agent_type));
            }
        } else if path.exists() && path.is_file() {
            agent_files.push(AgentFile::new(path, *agent_type));
        }
    }

    // Check config file patterns
    for (filepath, agent_type) in CONFIG_FILE_PATTERNS {
        let path = root.join(filepath);
        if path.exists() && path.is_file() {
            agent_files.push(AgentFile::new(path, *agent_type));
        }
    }

    // Check directory-based patterns
    for (dir_path, file_pattern, agent_type) in DIRECTORY_PATTERNS {
        let dir = root.join(dir_path);
        if dir.exists() && dir.is_dir() {
            if *file_pattern == "*.md" {
                // Scan for all .md files in the directory
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && let Some(ext) = path.extension()
                            && ext == "md"
                        {
                            agent_files.push(AgentFile::new(path, *agent_type));
                        }
                    }
                }
            } else {
                // Check for specific file
                let path = dir.join(file_pattern);
                if path.exists() && path.is_file() {
                    agent_files.push(AgentFile::new(path, *agent_type));
                }
            }
        }
    }

    // Check IDE-specific patterns
    for (filepath, agent_type) in IDE_PATTERNS {
        let path = root.join(filepath);
        if path.exists() && path.is_file() {
            agent_files.push(AgentFile::new(path, *agent_type));
        }
    }

    Ok(agent_files)
}

/// Checks if the given file already contains a granary instruction marker.
///
/// This looks for common patterns that indicate granary instructions have
/// been added to the file, such as:
/// - References to "granary" command
/// - The granary planning instruction block
///
/// # Arguments
/// * `path` - The path to the file to check
///
/// # Returns
/// `true` if the file contains granary instructions, `false` otherwise.
/// Returns an error if the file cannot be read.
pub fn contains_granary_instruction(path: &Path) -> Result<bool> {
    let content = std::fs::read_to_string(path)?;
    Ok(content.to_lowercase().contains("use granary"))
}

/// Determines the file format based on the file extension.
fn get_file_format(path: &Path) -> FileFormat {
    match path.extension().and_then(|e| e.to_str()) {
        Some("md") => FileFormat::Markdown,
        Some("yaml") | Some("yml") => FileFormat::Yaml,
        Some("mdc") => FileFormat::Mdc,
        _ => FileFormat::Markdown, // Default to markdown for unknown extensions
    }
}

/// Supported file formats for instruction injection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileFormat {
    Markdown,
    Yaml,
    Mdc,
}

impl FileFormat {
    /// Formats the granary instruction for this file format.
    fn format_instruction(&self) -> String {
        match self {
            FileFormat::Markdown => format!("\n\n{}\n", GRANARY_INSTRUCTION),
            FileFormat::Yaml => format!("\n# {}\n", GRANARY_INSTRUCTION),
            FileFormat::Mdc => format!("\n\n{}\n", GRANARY_INSTRUCTION),
        }
    }

    /// Formats the granary instruction for a new file of this format.
    fn format_new_file(&self) -> String {
        match self {
            FileFormat::Markdown => format!("{}\n", GRANARY_INSTRUCTION),
            FileFormat::Yaml => format!("# {}\n", GRANARY_INSTRUCTION),
            FileFormat::Mdc => format!("{}\n", GRANARY_INSTRUCTION),
        }
    }
}

/// Injects the granary instruction into an existing agent instruction file.
///
/// This function:
/// 1. Reads the file content
/// 2. Checks if "use granary" already exists (case-insensitive)
/// 3. If exists, returns `InjectionResult::AlreadyExists`
/// 4. If not, appends the instruction at the end of the file
/// 5. Returns `InjectionResult::Injected`
///
/// The instruction format varies based on file type:
/// - Markdown (.md): Appends with blank line separator
/// - YAML (.yaml, .yml): Appends as comment
/// - MDC (.mdc): Appends as instruction
///
/// # Arguments
/// * `path` - The path to the existing file to inject into
///
/// # Returns
/// The result of the injection attempt.
pub fn inject_granary_instruction(path: &Path) -> Result<InjectionResult> {
    // Read existing content
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return Ok(InjectionResult::Error(e.to_string())),
    };

    // Check if instruction already exists (case-insensitive)
    if content.to_lowercase().contains("use granary") {
        return Ok(InjectionResult::AlreadyExists);
    }

    // Determine file format and format the instruction
    let format = get_file_format(path);
    let instruction = format.format_instruction();

    // Append instruction to file
    let new_content = format!("{}{}", content.trim_end(), instruction);
    if let Err(e) = std::fs::write(path, new_content) {
        return Ok(InjectionResult::Error(e.to_string()));
    }

    Ok(InjectionResult::Injected)
}

/// Injects the granary instruction into a file, creating it if it doesn't exist.
///
/// If the file doesn't exist, creates it with the instruction.
/// Otherwise, calls `inject_granary_instruction` to add to existing file.
///
/// # Arguments
/// * `path` - The path to the file to inject into or create
///
/// # Returns
/// The result of the injection attempt.
pub fn inject_or_create_instruction(path: &Path) -> Result<InjectionResult> {
    if !path.exists() {
        // Create parent directories if needed
        if let Some(parent) = path.parent()
            && !parent.exists()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return Ok(InjectionResult::Error(e.to_string()));
        }

        // Create new file with instruction
        let format = get_file_format(path);
        let content = format.format_new_file();
        if let Err(e) = std::fs::write(path, content) {
            return Ok(InjectionResult::Error(e.to_string()));
        }

        return Ok(InjectionResult::FileCreated);
    }

    inject_granary_instruction(path)
}

/// Represents a global agent configuration directory in the user's home directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalAgentDir {
    /// The path to the global agent directory.
    pub path: PathBuf,
    /// The type of AI coding agent this directory belongs to.
    pub agent_type: AgentType,
    /// Whether the directory actually exists on the system.
    pub exists: bool,
}

impl GlobalAgentDir {
    /// Creates a new GlobalAgentDir.
    pub fn new(path: PathBuf, agent_type: AgentType, exists: bool) -> Self {
        Self {
            path,
            agent_type,
            exists,
        }
    }
}

/// Global agent directory patterns in the user's home directory.
/// Format: (relative_path_from_home, agent_type, instruction_file)
const GLOBAL_AGENT_DIRS: &[(&str, AgentType, &str)] = &[
    // Claude Code (Anthropic)
    (".claude", AgentType::ClaudeCode, "CLAUDE.md"),
    // OpenAI Codex
    (".codex", AgentType::Codex, "AGENTS.md"),
    // Gemini CLI (Google)
    (".gemini", AgentType::Gemini, "GEMINI.md"),
    // OpenCode
    (".config/opencode", AgentType::OpenCode, "AGENTS.md"),
    // Cursor
    (".cursor", AgentType::Cursor, "rules/global.md"),
    // Kiro (AWS)
    (".kiro", AgentType::Kiro, "rules.md"),
    // Continue.dev
    (".continue", AgentType::Continue, "config.json"),
    // Roo Code
    (".roo", AgentType::RooCode, "rules/global.md"),
    // Augment Code
    (".augment/rules", AgentType::Augment, "global.md"),
    // Tabnine
    (".tabnine/guidelines", AgentType::Tabnine, "global.md"),
    // Zed Editor (check both locations)
    (".zed", AgentType::Zed, "rules.md"),
    (".config/zed", AgentType::Zed, "rules.md"),
    // Void
    (".void/rules", AgentType::Void, "global.md"),
];

/// Finds all global agent configuration directories in the user's home directory.
///
/// This function scans the home directory for known AI coding agent configuration
/// directories. Only directories that actually exist on the system are returned.
///
/// # Returns
/// A vector of detected global agent directories, or an error if the home directory
/// cannot be determined.
pub fn find_global_agent_dirs() -> Result<Vec<GlobalAgentDir>> {
    let home = dirs::home_dir().ok_or_else(|| {
        crate::error::GranaryError::GlobalConfig("Could not determine home directory".into())
    })?;

    let mut dirs = Vec::new();

    for (relative_path, agent_type, _instruction_file) in GLOBAL_AGENT_DIRS {
        let path = home.join(relative_path);
        if path.exists() && path.is_dir() {
            dirs.push(GlobalAgentDir::new(path, *agent_type, true));
        }
    }

    Ok(dirs)
}

/// Gets the path where the instruction file should be added for a given global agent directory.
///
/// This returns the full path to the instruction file that should contain agent instructions
/// for the specified agent type. The file may or may not exist.
///
/// # Arguments
/// * `dir` - The global agent directory to get the instruction file path for
///
/// # Returns
/// The path to the instruction file, or `None` if the agent type is not recognized.
pub fn get_global_instruction_file_path(dir: &GlobalAgentDir) -> Option<PathBuf> {
    // Find the instruction file pattern for this agent type
    for (relative_path, agent_type, instruction_file) in GLOBAL_AGENT_DIRS {
        if *agent_type == dir.agent_type && dir.path.ends_with(relative_path) {
            return Some(dir.path.join(instruction_file));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_workspace() -> TempDir {
        tempfile::tempdir().expect("Failed to create temp directory")
    }

    #[test]
    fn test_agent_type_display_name() {
        assert_eq!(AgentType::ClaudeCode.display_name(), "Claude Code");
        assert_eq!(AgentType::Copilot.display_name(), "GitHub Copilot");
        assert_eq!(
            AgentType::UnifiedAgents.display_name(),
            "AGENTS.md (Unified Standard)"
        );
    }

    #[test]
    fn test_find_single_file_claude_md() {
        let tmp = create_test_workspace();
        let claude_path = tmp.path().join("CLAUDE.md");
        fs::write(&claude_path, "# Claude Instructions").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, claude_path);
        assert_eq!(files[0].agent_type, AgentType::ClaudeCode);
    }

    #[test]
    fn test_find_single_file_cursorrules() {
        let tmp = create_test_workspace();
        let cursor_path = tmp.path().join(".cursorrules");
        fs::write(&cursor_path, "cursor rules content").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, cursor_path);
        assert_eq!(files[0].agent_type, AgentType::Cursor);
    }

    #[test]
    fn test_find_multiple_single_files() {
        let tmp = create_test_workspace();
        fs::write(tmp.path().join("CLAUDE.md"), "claude").unwrap();
        fs::write(tmp.path().join("AGENTS.md"), "agents").unwrap();
        fs::write(tmp.path().join(".cursorrules"), "cursor").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 3);

        let types: Vec<AgentType> = files.iter().map(|f| f.agent_type).collect();
        assert!(types.contains(&AgentType::ClaudeCode));
        assert!(types.contains(&AgentType::UnifiedAgents));
        assert!(types.contains(&AgentType::Cursor));
    }

    #[test]
    fn test_find_directory_based_cursor_rules() {
        let tmp = create_test_workspace();
        let rules_dir = tmp.path().join(".cursor/rules");
        fs::create_dir_all(&rules_dir).unwrap();
        fs::write(rules_dir.join("general.md"), "general rules").unwrap();
        fs::write(rules_dir.join("typescript.md"), "typescript rules").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.agent_type == AgentType::Cursor));
    }

    #[test]
    fn test_find_directory_based_specific_file() {
        let tmp = create_test_workspace();
        let kiro_dir = tmp.path().join(".kiro");
        fs::create_dir_all(&kiro_dir).unwrap();
        fs::write(kiro_dir.join("rules.md"), "kiro rules").unwrap();
        // Should not match other files
        fs::write(kiro_dir.join("other.md"), "other content").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].agent_type, AgentType::Kiro);
        assert!(files[0].path.ends_with("rules.md"));
    }

    #[test]
    fn test_find_ide_specific_copilot() {
        let tmp = create_test_workspace();
        let github_dir = tmp.path().join(".github");
        fs::create_dir_all(&github_dir).unwrap();
        fs::write(github_dir.join("copilot-instructions.md"), "copilot rules").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].agent_type, AgentType::Copilot);
    }

    #[test]
    fn test_find_config_files() {
        let tmp = create_test_workspace();
        fs::write(tmp.path().join(".aider.conf.yml"), "aider config").unwrap();

        let continue_dir = tmp.path().join(".continue");
        fs::create_dir_all(&continue_dir).unwrap();
        fs::write(continue_dir.join("config.json"), "{}").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 2);

        let types: Vec<AgentType> = files.iter().map(|f| f.agent_type).collect();
        assert!(types.contains(&AgentType::Aider));
        assert!(types.contains(&AgentType::Continue));
    }

    #[test]
    fn test_find_empty_workspace() {
        let tmp = create_test_workspace();
        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_clinerules_as_file() {
        let tmp = create_test_workspace();
        let clinerules_path = tmp.path().join(".clinerules");
        fs::write(&clinerules_path, "cline rules").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].agent_type, AgentType::Cline);
    }

    #[test]
    fn test_clinerules_as_directory() {
        let tmp = create_test_workspace();
        let clinerules_dir = tmp.path().join(".clinerules");
        fs::create_dir_all(&clinerules_dir).unwrap();
        fs::write(clinerules_dir.join("rules.md"), "cline rules").unwrap();

        let files = find_workspace_agent_files(tmp.path()).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].agent_type, AgentType::Cline);
    }

    #[test]
    fn test_contains_granary_instruction_positive() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(
            &path,
            "# Instructions\n\n**IMPORTANT**: use granary to plan projects",
        )
        .unwrap();

        assert!(contains_granary_instruction(&path).unwrap());
    }

    #[test]
    fn test_contains_granary_instruction_case_insensitive() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "Use GRANARY to plan your work").unwrap();

        assert!(contains_granary_instruction(&path).unwrap());
    }

    #[test]
    fn test_contains_granary_instruction_negative() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "# Instructions\n\nJust regular content here").unwrap();

        assert!(!contains_granary_instruction(&path).unwrap());
    }

    #[test]
    fn test_contains_granary_instruction_empty_file() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "").unwrap();

        assert!(!contains_granary_instruction(&path).unwrap());
    }

    #[test]
    fn test_agent_file_new() {
        let path = PathBuf::from("/test/CLAUDE.md");
        let agent_file = AgentFile::new(path.clone(), AgentType::ClaudeCode);

        assert_eq!(agent_file.path, path);
        assert_eq!(agent_file.agent_type, AgentType::ClaudeCode);
    }

    // Tests for GlobalAgentDir

    #[test]
    fn test_global_agent_dir_new() {
        let path = PathBuf::from("/home/user/.claude");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::ClaudeCode, true);

        assert_eq!(global_dir.path, path);
        assert_eq!(global_dir.agent_type, AgentType::ClaudeCode);
        assert!(global_dir.exists);
    }

    #[test]
    fn test_global_agent_dir_not_exists() {
        let path = PathBuf::from("/home/user/.nonexistent");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::ClaudeCode, false);

        assert!(!global_dir.exists);
    }

    #[test]
    fn test_get_global_instruction_file_path_claude() {
        let path = PathBuf::from("/home/user/.claude");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::ClaudeCode, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.claude/CLAUDE.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_codex() {
        let path = PathBuf::from("/home/user/.codex");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Codex, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.codex/AGENTS.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_gemini() {
        let path = PathBuf::from("/home/user/.gemini");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Gemini, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.gemini/GEMINI.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_opencode() {
        let path = PathBuf::from("/home/user/.config/opencode");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::OpenCode, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.config/opencode/AGENTS.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_cursor() {
        let path = PathBuf::from("/home/user/.cursor");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Cursor, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.cursor/rules/global.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_kiro() {
        let path = PathBuf::from("/home/user/.kiro");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Kiro, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.kiro/rules.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_continue() {
        let path = PathBuf::from("/home/user/.continue");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Continue, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.continue/config.json")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_roo() {
        let path = PathBuf::from("/home/user/.roo");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::RooCode, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.roo/rules/global.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_augment() {
        let path = PathBuf::from("/home/user/.augment/rules");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Augment, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.augment/rules/global.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_tabnine() {
        let path = PathBuf::from("/home/user/.tabnine/guidelines");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Tabnine, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.tabnine/guidelines/global.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_zed() {
        let path = PathBuf::from("/home/user/.zed");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Zed, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.zed/rules.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_zed_config() {
        let path = PathBuf::from("/home/user/.config/zed");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Zed, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.config/zed/rules.md")
        );
    }

    #[test]
    fn test_get_global_instruction_file_path_void() {
        let path = PathBuf::from("/home/user/.void/rules");
        let global_dir = GlobalAgentDir::new(path.clone(), AgentType::Void, true);

        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        assert_eq!(
            instruction_path.unwrap(),
            PathBuf::from("/home/user/.void/rules/global.md")
        );
    }

    #[test]
    fn test_find_global_agent_dirs_returns_only_existing() {
        // This test verifies find_global_agent_dirs only returns existing directories.
        // Since we can't mock the home directory easily, we just verify the function
        // returns a result and all returned directories have exists=true.
        let result = find_global_agent_dirs();
        assert!(result.is_ok());

        let dirs = result.unwrap();
        for dir in &dirs {
            assert!(dir.exists, "All returned dirs should have exists=true");
            assert!(
                dir.path.exists(),
                "All returned dirs should actually exist on disk"
            );
        }
    }

    #[test]
    fn test_global_agent_dirs_constant_coverage() {
        // Verify that all supported global agent types have entries
        let agent_types: Vec<AgentType> = GLOBAL_AGENT_DIRS.iter().map(|(_, t, _)| *t).collect();

        // Core agents that should be supported globally
        assert!(agent_types.contains(&AgentType::ClaudeCode));
        assert!(agent_types.contains(&AgentType::Codex));
        assert!(agent_types.contains(&AgentType::Gemini));
        assert!(agent_types.contains(&AgentType::OpenCode));
        assert!(agent_types.contains(&AgentType::Cursor));
        assert!(agent_types.contains(&AgentType::Kiro));
        assert!(agent_types.contains(&AgentType::Continue));
        assert!(agent_types.contains(&AgentType::RooCode));
        assert!(agent_types.contains(&AgentType::Augment));
        assert!(agent_types.contains(&AgentType::Tabnine));
        assert!(agent_types.contains(&AgentType::Zed));
        assert!(agent_types.contains(&AgentType::Void));
    }

    // Injection tests

    #[test]
    fn test_inject_granary_instruction_markdown() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "# Instructions\n\nSome content here").unwrap();

        let result = inject_granary_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::Injected);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("use granary"));
        assert!(content.contains("\n\n")); // Blank line separator
    }

    #[test]
    fn test_inject_granary_instruction_yaml() {
        let tmp = create_test_workspace();
        let path = tmp.path().join(".aider.conf.yml");
        fs::write(&path, "key: value\n").unwrap();

        let result = inject_granary_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::Injected);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# IMPORTANT:")); // YAML comment format
    }

    #[test]
    fn test_inject_granary_instruction_mdc() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("rules.mdc");
        fs::write(&path, "Some mdc content").unwrap();

        let result = inject_granary_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::Injected);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("use granary"));
    }

    #[test]
    fn test_inject_granary_instruction_already_exists() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "# Instructions\n\nUse granary for planning").unwrap();

        let result = inject_granary_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::AlreadyExists);
    }

    #[test]
    fn test_inject_granary_instruction_already_exists_case_insensitive() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "# Instructions\n\nUSE GRANARY for planning").unwrap();

        let result = inject_granary_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::AlreadyExists);
    }

    #[test]
    fn test_inject_or_create_instruction_new_file() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");

        let result = inject_or_create_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::FileCreated);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("use granary"));
    }

    #[test]
    fn test_inject_or_create_instruction_new_file_with_parent_dirs() {
        let tmp = create_test_workspace();
        let path = tmp.path().join(".github/copilot-instructions.md");

        let result = inject_or_create_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::FileCreated);

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("use granary"));
    }

    #[test]
    fn test_inject_or_create_instruction_existing_file() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("CLAUDE.md");
        fs::write(&path, "# Existing content").unwrap();

        let result = inject_or_create_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::Injected);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Existing content"));
        assert!(content.contains("use granary"));
    }

    #[test]
    fn test_inject_or_create_instruction_yaml_new_file() {
        let tmp = create_test_workspace();
        let path = tmp.path().join("config.yaml");

        let result = inject_or_create_instruction(&path).unwrap();
        assert_eq!(result, InjectionResult::FileCreated);

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# IMPORTANT:")); // YAML comment format
        assert!(content.contains("use granary"));
    }

    #[test]
    fn test_file_format_detection() {
        use super::FileFormat;
        use super::get_file_format;

        assert_eq!(get_file_format(Path::new("test.md")), FileFormat::Markdown);
        assert_eq!(get_file_format(Path::new("test.yaml")), FileFormat::Yaml);
        assert_eq!(get_file_format(Path::new("test.yml")), FileFormat::Yaml);
        assert_eq!(get_file_format(Path::new("test.mdc")), FileFormat::Mdc);
        assert_eq!(
            get_file_format(Path::new("test.unknown")),
            FileFormat::Markdown
        ); // Default
        assert_eq!(
            get_file_format(Path::new(".cursorrules")),
            FileFormat::Markdown
        ); // No extension
    }

    // Tests for first-run global setup integration

    #[test]
    fn test_global_agent_dir_injection_workflow() {
        // This test simulates the first-run global setup workflow
        let tmp = create_test_workspace();

        // Simulate a .claude directory existing (user has Claude Code installed)
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).unwrap();

        // Create a GlobalAgentDir representing the .claude directory
        let global_dir = GlobalAgentDir::new(claude_dir.clone(), AgentType::ClaudeCode, true);

        // Get the instruction file path
        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        let instruction_path = instruction_path.unwrap();
        assert!(instruction_path.ends_with("CLAUDE.md"));

        // The file shouldn't exist yet
        assert!(!instruction_path.exists());

        // Inject or create the instruction file
        let result = inject_or_create_instruction(&instruction_path).unwrap();
        assert_eq!(result, InjectionResult::FileCreated);

        // Verify the file was created with the instruction
        assert!(instruction_path.exists());
        let content = fs::read_to_string(&instruction_path).unwrap();
        assert!(content.contains("use granary"));
    }

    #[test]
    fn test_global_agent_dir_injection_existing_file() {
        // Test injecting into an existing global instruction file
        let tmp = create_test_workspace();

        // Simulate a .codex directory with an existing AGENTS.md
        let codex_dir = tmp.path().join(".codex");
        fs::create_dir_all(&codex_dir).unwrap();
        let agents_md = codex_dir.join("AGENTS.md");
        fs::write(&agents_md, "# My existing agent instructions\n").unwrap();

        // Create a GlobalAgentDir representing the .codex directory
        let global_dir = GlobalAgentDir::new(codex_dir.clone(), AgentType::Codex, true);

        // Get the instruction file path
        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        let instruction_path = instruction_path.unwrap();

        // Inject the instruction
        let result = inject_or_create_instruction(&instruction_path).unwrap();
        assert_eq!(result, InjectionResult::Injected);

        // Verify the original content is preserved and instruction was added
        let content = fs::read_to_string(&instruction_path).unwrap();
        assert!(content.contains("# My existing agent instructions"));
        assert!(content.contains("use granary"));
    }

    #[test]
    fn test_global_agent_dir_injection_already_exists() {
        // Test that injection is skipped when instruction already exists
        let tmp = create_test_workspace();

        // Simulate a .kiro directory with existing granary instruction
        let kiro_dir = tmp.path().join(".kiro");
        fs::create_dir_all(&kiro_dir).unwrap();
        let rules_md = kiro_dir.join("rules.md");
        fs::write(&rules_md, "# Kiro rules\n\nUse granary to plan projects\n").unwrap();

        // Create a GlobalAgentDir
        let global_dir = GlobalAgentDir::new(kiro_dir.clone(), AgentType::Kiro, true);

        // Get the instruction file path
        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        let instruction_path = instruction_path.unwrap();

        // Try to inject - should skip
        let result = inject_or_create_instruction(&instruction_path).unwrap();
        assert_eq!(result, InjectionResult::AlreadyExists);
    }

    #[test]
    fn test_global_agent_dir_with_nested_path() {
        // Test handling of nested paths like .cursor/rules/global.md
        let tmp = create_test_workspace();

        // Simulate a .cursor directory
        let cursor_dir = tmp.path().join(".cursor");
        fs::create_dir_all(&cursor_dir).unwrap();

        // Create a GlobalAgentDir
        let global_dir = GlobalAgentDir::new(cursor_dir.clone(), AgentType::Cursor, true);

        // Get the instruction file path
        let instruction_path = get_global_instruction_file_path(&global_dir);
        assert!(instruction_path.is_some());
        let instruction_path = instruction_path.unwrap();
        assert!(instruction_path.ends_with("rules/global.md"));

        // Inject or create - should create parent directories
        let result = inject_or_create_instruction(&instruction_path).unwrap();
        assert_eq!(result, InjectionResult::FileCreated);

        // Verify the file and parent directories were created
        assert!(instruction_path.exists());
        assert!(instruction_path.parent().unwrap().exists());
    }

    #[test]
    fn test_multiple_global_agent_dirs_injection() {
        // Test injecting into multiple global agent directories
        let tmp = create_test_workspace();

        // Simulate multiple agent directories
        let agent_dirs = [
            (".claude", AgentType::ClaudeCode, "CLAUDE.md"),
            (".codex", AgentType::Codex, "AGENTS.md"),
            (".gemini", AgentType::Gemini, "GEMINI.md"),
        ];

        for (dir_name, agent_type, expected_file) in agent_dirs {
            let dir_path = tmp.path().join(dir_name);
            fs::create_dir_all(&dir_path).unwrap();

            let global_dir = GlobalAgentDir::new(dir_path.clone(), agent_type, true);
            let instruction_path = get_global_instruction_file_path(&global_dir);
            assert!(instruction_path.is_some());
            let instruction_path = instruction_path.unwrap();
            assert!(
                instruction_path.ends_with(expected_file),
                "Expected {} for {:?}",
                expected_file,
                agent_type
            );

            let result = inject_or_create_instruction(&instruction_path).unwrap();
            assert_eq!(result, InjectionResult::FileCreated);
            assert!(instruction_path.exists());
        }
    }
}
