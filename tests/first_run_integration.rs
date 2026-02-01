//! Integration tests for first-run global agent setup.
//!
//! These tests verify that granary correctly detects first run and injects
//! instructions into global agent directories.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tempfile::TempDir;

/// Find the granary CLI binary in the target directory.
fn find_granary_binary() -> Result<PathBuf, String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let target_dir = PathBuf::from(manifest_dir).join("target");

    let debug_path = target_dir.join("debug").join("granary");
    if debug_path.exists() {
        return Ok(debug_path);
    }

    let release_path = target_dir.join("release").join("granary");
    if release_path.exists() {
        return Ok(release_path);
    }

    // Check if we're running from cargo test
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling_path = dir.join("granary");
        if sibling_path.exists() {
            return Ok(sibling_path);
        }
    }

    Err(format!(
        "granary binary not found. Build it first with 'cargo build'. Searched in: {:?}",
        target_dir
    ))
}

/// Test that first run injects instructions into existing global agent directories.
#[test]
fn test_first_run_injects_into_global_dirs() {
    let granary_bin = match find_granary_binary() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Create a temporary home directory
    let temp_home = TempDir::new().expect("Failed to create temp home dir");

    // Create some global agent directories to simulate installed agents
    let claude_dir = temp_home.path().join(".claude");
    fs::create_dir_all(&claude_dir).expect("Failed to create .claude dir");

    let codex_dir = temp_home.path().join(".codex");
    fs::create_dir_all(&codex_dir).expect("Failed to create .codex dir");

    // Create a workspace directory
    let workspace_dir = temp_home.path().join("workspace");
    fs::create_dir_all(&workspace_dir).expect("Failed to create workspace dir");

    // Run granary init with the temp home
    let output = Command::new(&granary_bin)
        .arg("init")
        .env("HOME", temp_home.path())
        .current_dir(&workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run granary init");

    // Check that the command succeeded
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("granary init failed: {}", stderr);
    }
    assert!(output.status.success(), "granary init should succeed");

    // Verify that global instruction files were created
    let claude_md = claude_dir.join("CLAUDE.md");
    assert!(
        claude_md.exists(),
        "CLAUDE.md should be created in .claude directory"
    );
    let claude_content = fs::read_to_string(&claude_md).expect("Failed to read CLAUDE.md");
    assert!(
        claude_content.contains("use granary"),
        "CLAUDE.md should contain granary instruction"
    );

    let codex_agents_md = codex_dir.join("AGENTS.md");
    assert!(
        codex_agents_md.exists(),
        "AGENTS.md should be created in .codex directory"
    );
    let codex_content = fs::read_to_string(&codex_agents_md).expect("Failed to read AGENTS.md");
    assert!(
        codex_content.contains("use granary"),
        "AGENTS.md should contain granary instruction"
    );
}

/// Test that subsequent runs do not modify global agent directories.
#[test]
fn test_subsequent_run_skips_global_injection() {
    let granary_bin = match find_granary_binary() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Create a temporary home directory
    let temp_home = TempDir::new().expect("Failed to create temp home dir");

    // Create a .granary directory to simulate existing installation
    let granary_dir = temp_home.path().join(".granary");
    fs::create_dir_all(&granary_dir).expect("Failed to create .granary dir");

    // Create a global agent directory
    let claude_dir = temp_home.path().join(".claude");
    fs::create_dir_all(&claude_dir).expect("Failed to create .claude dir");

    // Create a workspace directory
    let workspace_dir = temp_home.path().join("workspace");
    fs::create_dir_all(&workspace_dir).expect("Failed to create workspace dir");

    // Run granary init
    let output = Command::new(&granary_bin)
        .arg("init")
        .env("HOME", temp_home.path())
        .current_dir(&workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run granary init");

    assert!(output.status.success(), "granary init should succeed");

    // The CLAUDE.md should NOT be created because this is not first run
    let claude_md = claude_dir.join("CLAUDE.md");
    assert!(
        !claude_md.exists(),
        "CLAUDE.md should NOT be created on subsequent runs"
    );
}

/// Test that first run handles missing global directories gracefully.
#[test]
fn test_first_run_with_no_global_dirs() {
    let granary_bin = match find_granary_binary() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Create a temporary home directory with no agent directories
    let temp_home = TempDir::new().expect("Failed to create temp home dir");

    // Create a workspace directory
    let workspace_dir = temp_home.path().join("workspace");
    fs::create_dir_all(&workspace_dir).expect("Failed to create workspace dir");

    // Run granary init
    let output = Command::new(&granary_bin)
        .arg("init")
        .env("HOME", temp_home.path())
        .current_dir(&workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run granary init");

    // Should succeed even with no global agent directories
    assert!(
        output.status.success(),
        "granary init should succeed with no global dirs"
    );
}

/// Test that existing global instruction files are injected into, not overwritten.
#[test]
fn test_first_run_injects_into_existing_files() {
    let granary_bin = match find_granary_binary() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Create a temporary home directory
    let temp_home = TempDir::new().expect("Failed to create temp home dir");

    // Create a .claude directory with existing CLAUDE.md
    let claude_dir = temp_home.path().join(".claude");
    fs::create_dir_all(&claude_dir).expect("Failed to create .claude dir");
    let claude_md = claude_dir.join("CLAUDE.md");
    let original_content = "# My Custom Instructions\n\nDo something important.\n";
    fs::write(&claude_md, original_content).expect("Failed to write CLAUDE.md");

    // Create a workspace directory
    let workspace_dir = temp_home.path().join("workspace");
    fs::create_dir_all(&workspace_dir).expect("Failed to create workspace dir");

    // Run granary init
    let output = Command::new(&granary_bin)
        .arg("init")
        .env("HOME", temp_home.path())
        .current_dir(&workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run granary init");

    assert!(output.status.success(), "granary init should succeed");

    // Verify that original content is preserved and instruction was added
    let updated_content = fs::read_to_string(&claude_md).expect("Failed to read CLAUDE.md");
    assert!(
        updated_content.contains("# My Custom Instructions"),
        "Original content should be preserved"
    );
    assert!(
        updated_content.contains("Do something important"),
        "Original content should be preserved"
    );
    assert!(
        updated_content.contains("use granary"),
        "Granary instruction should be added"
    );
}

/// Test that files with existing granary instruction are not modified.
#[test]
fn test_first_run_skips_files_with_existing_instruction() {
    let granary_bin = match find_granary_binary() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Skipping test: {}", e);
            return;
        }
    };

    // Create a temporary home directory
    let temp_home = TempDir::new().expect("Failed to create temp home dir");

    // Create a .claude directory with CLAUDE.md that already has granary instruction
    let claude_dir = temp_home.path().join(".claude");
    fs::create_dir_all(&claude_dir).expect("Failed to create .claude dir");
    let claude_md = claude_dir.join("CLAUDE.md");
    let existing_content = "# Instructions\n\nUse granary to plan your work.\n";
    fs::write(&claude_md, existing_content).expect("Failed to write CLAUDE.md");

    // Create a workspace directory
    let workspace_dir = temp_home.path().join("workspace");
    fs::create_dir_all(&workspace_dir).expect("Failed to create workspace dir");

    // Run granary init
    let output = Command::new(&granary_bin)
        .arg("init")
        .env("HOME", temp_home.path())
        .current_dir(&workspace_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run granary init");

    assert!(output.status.success(), "granary init should succeed");

    // Verify that file content was not modified
    let final_content = fs::read_to_string(&claude_md).expect("Failed to read CLAUDE.md");
    assert_eq!(
        final_content, existing_content,
        "File with existing instruction should not be modified"
    );
}
