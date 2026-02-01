//! Integration tests for agent file detection and instruction injection.
//!
//! These tests verify the agent_files module works correctly as a whole,
//! testing realistic scenarios that span multiple functions.

use std::fs;

use tempfile::TempDir;

// Import the library to access the agent_files module
use granary::services::agent_files::{
    AgentFile, AgentType, GlobalAgentDir, InjectionResult, contains_granary_instruction,
    find_workspace_agent_files, get_global_instruction_file_path, inject_granary_instruction,
    inject_or_create_instruction,
};

/// Create a temporary workspace for testing.
fn create_test_workspace() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

// ============================================================================
// Workspace Agent File Detection Integration Tests
// ============================================================================

/// Test detecting a complex workspace with multiple agent file types.
#[test]
fn test_detect_complex_workspace() {
    let tmp = create_test_workspace();

    // Set up a realistic workspace with multiple agent configurations
    // Single files at root
    fs::write(tmp.path().join("CLAUDE.md"), "# Claude instructions").unwrap();
    fs::write(tmp.path().join("AGENTS.md"), "# Unified agents").unwrap();
    fs::write(tmp.path().join(".cursorrules"), "cursor rules").unwrap();
    fs::write(tmp.path().join("CONVENTIONS.md"), "# Aider conventions").unwrap();

    // Directory-based configurations
    let cursor_rules = tmp.path().join(".cursor/rules");
    fs::create_dir_all(&cursor_rules).unwrap();
    fs::write(cursor_rules.join("typescript.md"), "# TypeScript rules").unwrap();
    fs::write(cursor_rules.join("react.md"), "# React rules").unwrap();

    let github_dir = tmp.path().join(".github");
    fs::create_dir_all(&github_dir).unwrap();
    fs::write(
        github_dir.join("copilot-instructions.md"),
        "# Copilot rules",
    )
    .unwrap();

    // Config files
    fs::write(tmp.path().join(".aider.conf.yml"), "key: value").unwrap();

    let continue_dir = tmp.path().join(".continue");
    fs::create_dir_all(&continue_dir).unwrap();
    fs::write(continue_dir.join("config.json"), "{}").unwrap();

    // Detect all files
    let files = find_workspace_agent_files(tmp.path()).unwrap();

    // Count expected files:
    // - CLAUDE.md (1)
    // - AGENTS.md (1)
    // - .cursorrules (1)
    // - CONVENTIONS.md (1)
    // - .cursor/rules/typescript.md (1)
    // - .cursor/rules/react.md (1)
    // - .github/copilot-instructions.md (1)
    // - .aider.conf.yml (1)
    // - .continue/config.json (1)
    // Total: 9 files
    assert!(
        files.len() >= 9,
        "Expected at least 9 agent files, found {}",
        files.len()
    );

    // Verify we detected each type
    let has_type = |t: AgentType| files.iter().any(|f| f.agent_type == t);
    assert!(has_type(AgentType::ClaudeCode), "Should detect Claude Code");
    assert!(
        has_type(AgentType::UnifiedAgents),
        "Should detect AGENTS.md"
    );
    assert!(has_type(AgentType::Cursor), "Should detect Cursor");
    assert!(has_type(AgentType::Aider), "Should detect Aider");
    assert!(has_type(AgentType::Copilot), "Should detect Copilot");
    assert!(has_type(AgentType::Continue), "Should detect Continue");
}

/// Test that empty directories don't cause issues.
#[test]
fn test_detect_with_empty_agent_directories() {
    let tmp = create_test_workspace();

    // Create empty directories (no files inside)
    fs::create_dir_all(tmp.path().join(".cursor/rules")).unwrap();
    fs::create_dir_all(tmp.path().join(".github")).unwrap();
    fs::create_dir_all(tmp.path().join(".continue")).unwrap();

    let files = find_workspace_agent_files(tmp.path()).unwrap();

    // Should find nothing - empty directories don't count
    assert!(files.is_empty(), "Empty directories should not be detected");
}

/// Test workspace with nested project structure.
#[test]
fn test_detect_only_root_level() {
    let tmp = create_test_workspace();

    // Create a CLAUDE.md at root
    fs::write(tmp.path().join("CLAUDE.md"), "# Root Claude").unwrap();

    // Create nested project with its own CLAUDE.md (should NOT be detected)
    let nested = tmp.path().join("packages/subproject");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("CLAUDE.md"), "# Nested Claude").unwrap();

    let files = find_workspace_agent_files(tmp.path()).unwrap();

    // Should only find root-level file
    assert_eq!(files.len(), 1, "Should only find root-level CLAUDE.md");
    assert!(
        files[0].path.ends_with("CLAUDE.md"),
        "Should be the root CLAUDE.md"
    );
    assert!(
        !files[0].path.to_string_lossy().contains("packages"),
        "Should not include nested CLAUDE.md"
    );
}

// ============================================================================
// Instruction Injection Integration Tests
// ============================================================================

/// Test the full workflow: detect file, check for instruction, inject if needed.
#[test]
fn test_detect_and_inject_workflow() {
    let tmp = create_test_workspace();

    // Create a CLAUDE.md without granary instruction
    let claude_path = tmp.path().join("CLAUDE.md");
    fs::write(&claude_path, "# My Project\n\nProject instructions here.").unwrap();

    // Step 1: Detect agent files
    let files = find_workspace_agent_files(tmp.path()).unwrap();
    assert_eq!(files.len(), 1);
    let file = &files[0];

    // Step 2: Check if instruction exists using contains_granary_instruction
    // Note: contains_granary_instruction checks for specific patterns like "use granary to plan"
    // The actual injected instruction is different, so we check for that pattern
    let has_instruction = contains_granary_instruction(&file.path).unwrap();
    assert!(!has_instruction, "Should not have instruction initially");

    // Step 3: Inject instruction
    let result = inject_granary_instruction(&file.path).unwrap();
    assert_eq!(result, InjectionResult::Injected);

    // Step 4: Verify instruction was added by checking file content directly
    // The injected instruction is: 'When user asks "use granary" run `granary` command first'
    let content = fs::read_to_string(&file.path).unwrap();
    assert!(
        content.to_lowercase().contains("use granary"),
        "Should contain 'use granary' after injection"
    );

    // Step 5: Verify original content preserved
    assert!(content.contains("# My Project"));
    assert!(content.contains("Project instructions here"));

    // Step 6: Verify re-injection is skipped (idempotency check)
    let result2 = inject_granary_instruction(&file.path).unwrap();
    assert_eq!(
        result2,
        InjectionResult::AlreadyExists,
        "Re-injection should be skipped"
    );
}

/// Test injecting into multiple files in a workspace.
#[test]
fn test_inject_into_multiple_files() {
    let tmp = create_test_workspace();

    // Create multiple agent files
    fs::write(tmp.path().join("CLAUDE.md"), "# Claude").unwrap();
    fs::write(tmp.path().join("AGENTS.md"), "# Agents").unwrap();
    fs::write(tmp.path().join(".cursorrules"), "cursor rules").unwrap();

    // Detect all files
    let files = find_workspace_agent_files(tmp.path()).unwrap();
    assert_eq!(files.len(), 3);

    // Inject into each file
    let mut injected_count = 0;
    for file in &files {
        let result = inject_granary_instruction(&file.path).unwrap();
        if result == InjectionResult::Injected {
            injected_count += 1;
        }
    }
    assert_eq!(injected_count, 3, "Should inject into all 3 files");

    // Verify all files have instruction
    for file in &files {
        let content = fs::read_to_string(&file.path).unwrap();
        assert!(
            content.to_lowercase().contains("use granary"),
            "File {:?} should contain instruction",
            file.path
        );
    }
}

/// Test that idempotent injection works correctly.
#[test]
fn test_idempotent_injection() {
    let tmp = create_test_workspace();

    let path = tmp.path().join("CLAUDE.md");
    fs::write(&path, "# Instructions").unwrap();

    // First injection
    let result1 = inject_granary_instruction(&path).unwrap();
    assert_eq!(result1, InjectionResult::Injected);
    let content_after_first = fs::read_to_string(&path).unwrap();

    // Second injection (should be skipped)
    let result2 = inject_granary_instruction(&path).unwrap();
    assert_eq!(result2, InjectionResult::AlreadyExists);
    let content_after_second = fs::read_to_string(&path).unwrap();

    // Content should be identical
    assert_eq!(
        content_after_first, content_after_second,
        "Content should not change on second injection"
    );

    // Third injection (still should be skipped)
    let result3 = inject_granary_instruction(&path).unwrap();
    assert_eq!(result3, InjectionResult::AlreadyExists);
}

// ============================================================================
// Global Agent Directory Integration Tests
// ============================================================================

/// Test the global directory workflow with simulated home.
#[test]
fn test_global_dir_create_and_inject_workflow() {
    let tmp = create_test_workspace();

    // Simulate global directories
    let claude_dir = tmp.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();

    let codex_dir = tmp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    // Create GlobalAgentDir instances
    let global_dirs = vec![
        GlobalAgentDir::new(claude_dir.clone(), AgentType::ClaudeCode, true),
        GlobalAgentDir::new(codex_dir.clone(), AgentType::Codex, true),
    ];

    // For each directory, get instruction path and inject
    for dir in &global_dirs {
        let instruction_path = get_global_instruction_file_path(dir);
        assert!(
            instruction_path.is_some(),
            "Should get instruction path for {:?}",
            dir.agent_type
        );

        let path = instruction_path.unwrap();
        let result = inject_or_create_instruction(&path).unwrap();
        assert_eq!(
            result,
            InjectionResult::FileCreated,
            "Should create file for {:?}",
            dir.agent_type
        );

        // Verify file was created
        assert!(path.exists(), "File should exist after injection");
        let content = fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("use granary"),
            "File should contain instruction"
        );
    }
}

/// Test mixed scenario: some files exist, some don't.
#[test]
fn test_mixed_existing_and_new_files() {
    let tmp = create_test_workspace();

    // Create .claude with existing CLAUDE.md
    let claude_dir = tmp.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    let claude_md = claude_dir.join("CLAUDE.md");
    fs::write(&claude_md, "# Existing Claude instructions").unwrap();

    // Create .codex without AGENTS.md
    let codex_dir = tmp.path().join(".codex");
    fs::create_dir_all(&codex_dir).unwrap();

    // Create .gemini without GEMINI.md
    let gemini_dir = tmp.path().join(".gemini");
    fs::create_dir_all(&gemini_dir).unwrap();

    let test_cases = vec![
        (
            GlobalAgentDir::new(claude_dir.clone(), AgentType::ClaudeCode, true),
            InjectionResult::Injected,
        ),
        (
            GlobalAgentDir::new(codex_dir.clone(), AgentType::Codex, true),
            InjectionResult::FileCreated,
        ),
        (
            GlobalAgentDir::new(gemini_dir.clone(), AgentType::Gemini, true),
            InjectionResult::FileCreated,
        ),
    ];

    for (dir, expected_result) in test_cases {
        let path = get_global_instruction_file_path(&dir).unwrap();
        let result = inject_or_create_instruction(&path).unwrap();
        assert_eq!(
            result, expected_result,
            "Unexpected result for {:?}",
            dir.agent_type
        );
    }

    // Verify Claude's original content was preserved
    let claude_content = fs::read_to_string(&claude_md).unwrap();
    assert!(claude_content.contains("# Existing Claude instructions"));
    assert!(claude_content.contains("use granary"));
}

// ============================================================================
// Edge Cases and Error Handling Integration Tests
// ============================================================================

/// Test handling of symlinks (if supported on platform).
#[test]
#[cfg(unix)]
fn test_symlink_handling() {
    let tmp = create_test_workspace();

    // Create a real CLAUDE.md
    let real_file = tmp.path().join("real_claude.md");
    fs::write(&real_file, "# Real Claude").unwrap();

    // Create a symlink to it
    let symlink_path = tmp.path().join("CLAUDE.md");
    std::os::unix::fs::symlink(&real_file, &symlink_path).unwrap();

    // Detection should follow the symlink
    let files = find_workspace_agent_files(tmp.path()).unwrap();
    assert_eq!(files.len(), 1, "Should detect CLAUDE.md symlink");
    assert_eq!(files[0].agent_type, AgentType::ClaudeCode);

    // Injection should work through the symlink
    let result = inject_granary_instruction(&symlink_path).unwrap();
    assert_eq!(result, InjectionResult::Injected);

    // The real file should have the instruction
    let content = fs::read_to_string(&real_file).unwrap();
    assert!(content.contains("use granary"));
}

/// Test with special characters in path.
#[test]
fn test_special_characters_in_path() {
    let tmp = create_test_workspace();

    // Create CLAUDE.md (path will have temp directory with unique chars)
    let claude_path = tmp.path().join("CLAUDE.md");
    fs::write(&claude_path, "# Claude with special path").unwrap();

    let files = find_workspace_agent_files(tmp.path()).unwrap();
    assert_eq!(files.len(), 1);

    let result = inject_granary_instruction(&files[0].path).unwrap();
    assert_eq!(result, InjectionResult::Injected);
}

/// Test with large file content.
#[test]
fn test_large_file_injection() {
    let tmp = create_test_workspace();

    // Create a large CLAUDE.md (simulate a detailed instruction file)
    let mut content = String::from("# Large Claude Instructions\n\n");
    for i in 0..1000 {
        content.push_str(&format!("## Section {}\n\nThis is section {} with detailed instructions about how to handle various scenarios.\n\n", i, i));
    }

    let claude_path = tmp.path().join("CLAUDE.md");
    fs::write(&claude_path, &content).unwrap();

    // Should handle large files
    let result = inject_granary_instruction(&claude_path).unwrap();
    assert_eq!(result, InjectionResult::Injected);

    // Verify the file wasn't corrupted
    let final_content = fs::read_to_string(&claude_path).unwrap();
    assert!(
        final_content.len() > content.len(),
        "File should be larger after injection"
    );
    assert!(final_content.contains("# Large Claude Instructions"));
    assert!(final_content.contains("Section 999"));
    assert!(final_content.contains("use granary"));
}

/// Test AgentFile struct usage.
#[test]
fn test_agent_file_struct() {
    let path = std::path::PathBuf::from("/test/CLAUDE.md");
    let agent_file = AgentFile::new(path.clone(), AgentType::ClaudeCode);

    assert_eq!(agent_file.path, path);
    assert_eq!(agent_file.agent_type, AgentType::ClaudeCode);
    assert_eq!(agent_file.agent_type.display_name(), "Claude Code");
}

/// Test GlobalAgentDir struct usage.
#[test]
fn test_global_agent_dir_struct() {
    let path = std::path::PathBuf::from("/home/user/.claude");
    let dir = GlobalAgentDir::new(path.clone(), AgentType::ClaudeCode, true);

    assert_eq!(dir.path, path);
    assert_eq!(dir.agent_type, AgentType::ClaudeCode);
    assert!(dir.exists);
}
