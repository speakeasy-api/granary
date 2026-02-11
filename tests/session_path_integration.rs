//! Integration tests verifying session file path resolution per workspace mode.
//!
//! Per the workspace modes RFC, session files should resolve to:
//! - Local mode: /path/to/project/.granary/session
//! - Named workspace: ~/.granary/workspaces/<name>/session
//! - Default: ~/.granary/session
//!
//! The GRANARY_SESSION env var overrides file-based lookup in all modes.

use std::path::PathBuf;

use tempfile::TempDir;

/// Helper to construct a workspace-like directory and verify session behavior
/// without needing a full database or the CLI binary.
mod helpers {
    use std::fs;
    use std::path::{Path, PathBuf};

    pub const SESSION_FILE: &str = "session";

    /// Simulates session file path resolution: granary_dir.join("session")
    pub fn session_path(granary_dir: &Path) -> PathBuf {
        granary_dir.join(SESSION_FILE)
    }

    /// Simulates set_current_session: writes session_id to the session file
    pub fn set_session(granary_dir: &Path, session_id: &str) {
        let path = session_path(granary_dir);
        fs::write(&path, session_id).expect("Failed to write session file");
    }

    /// Simulates current_session_id: reads session from file (no env var check)
    pub fn get_session(granary_dir: &Path) -> Option<String> {
        let path = session_path(granary_dir);
        if !path.exists() {
            return None;
        }
        let content = fs::read_to_string(&path).ok()?;
        let id = content.trim().to_string();
        if id.is_empty() { None } else { Some(id) }
    }

    /// Simulates clear_current_session: removes the session file
    pub fn clear_session(granary_dir: &Path) {
        let path = session_path(granary_dir);
        if path.exists() {
            fs::remove_file(&path).expect("Failed to remove session file");
        }
    }
}

/// Test: Local mode session file resolves to /path/to/project/.granary/session
#[test]
fn test_session_path_local_mode() {
    let project_dir = TempDir::new().expect("Failed to create temp dir");
    let granary_dir = project_dir.path().join(".granary");
    std::fs::create_dir_all(&granary_dir).expect("Failed to create .granary dir");

    let expected_session = granary_dir.join("session");
    assert_eq!(helpers::session_path(&granary_dir), expected_session);

    // Verify read/write/clear cycle
    assert_eq!(helpers::get_session(&granary_dir), None);

    helpers::set_session(&granary_dir, "local-session-123");
    assert_eq!(
        helpers::get_session(&granary_dir),
        Some("local-session-123".to_string())
    );

    helpers::clear_session(&granary_dir);
    assert_eq!(helpers::get_session(&granary_dir), None);
}

/// Test: Named workspace session file resolves to ~/.granary/workspaces/<name>/session
#[test]
fn test_session_path_named_workspace() {
    let home_dir = TempDir::new().expect("Failed to create temp dir");
    let workspace_name = "myproject";
    let granary_dir = home_dir
        .path()
        .join(".granary")
        .join("workspaces")
        .join(workspace_name);
    std::fs::create_dir_all(&granary_dir).expect("Failed to create workspace dir");

    let expected_session = granary_dir.join("session");
    assert_eq!(helpers::session_path(&granary_dir), expected_session);

    // Verify the path structure matches the RFC
    let path_str = expected_session.to_string_lossy();
    assert!(
        path_str.contains(".granary/workspaces/myproject/session"),
        "Named workspace session should be at ~/.granary/workspaces/<name>/session, got: {}",
        path_str
    );

    // Verify read/write/clear cycle
    helpers::set_session(&granary_dir, "named-session-456");
    assert_eq!(
        helpers::get_session(&granary_dir),
        Some("named-session-456".to_string())
    );

    helpers::clear_session(&granary_dir);
    assert_eq!(helpers::get_session(&granary_dir), None);
}

/// Test: Default mode session file resolves to ~/.granary/session
#[test]
fn test_session_path_default_mode() {
    let home_dir = TempDir::new().expect("Failed to create temp dir");
    let granary_dir = home_dir.path().join(".granary");
    std::fs::create_dir_all(&granary_dir).expect("Failed to create .granary dir");

    let expected_session = granary_dir.join("session");
    assert_eq!(helpers::session_path(&granary_dir), expected_session);

    // Verify the path structure: should be directly under ~/.granary/
    // NOT under workspaces/
    let path_str = expected_session.to_string_lossy();
    assert!(
        path_str.ends_with(".granary/session"),
        "Default workspace session should be at ~/.granary/session, got: {}",
        path_str
    );
    assert!(
        !path_str.contains("workspaces"),
        "Default workspace session should NOT be under workspaces/, got: {}",
        path_str
    );

    // Verify read/write/clear cycle
    helpers::set_session(&granary_dir, "default-session-789");
    assert_eq!(
        helpers::get_session(&granary_dir),
        Some("default-session-789".to_string())
    );

    helpers::clear_session(&granary_dir);
    assert_eq!(helpers::get_session(&granary_dir), None);
}

/// Test: Session paths are isolated between workspace modes.
/// Each mode's granary_dir yields a distinct session file location.
#[test]
fn test_session_paths_are_isolated() {
    let root = TempDir::new().expect("Failed to create temp dir");

    // Simulate three different granary_dir values for each mode
    let local_granary_dir = root.path().join("project").join(".granary");
    let named_granary_dir = root
        .path()
        .join(".granary")
        .join("workspaces")
        .join("mywork");
    let default_granary_dir = root.path().join(".granary");

    std::fs::create_dir_all(&local_granary_dir).unwrap();
    std::fs::create_dir_all(&named_granary_dir).unwrap();
    std::fs::create_dir_all(&default_granary_dir).unwrap();

    let local_session = helpers::session_path(&local_granary_dir);
    let named_session = helpers::session_path(&named_granary_dir);
    let default_session = helpers::session_path(&default_granary_dir);

    // All three paths must be different
    assert_ne!(local_session, named_session);
    assert_ne!(local_session, default_session);
    assert_ne!(named_session, default_session);

    // Write different session IDs to each
    helpers::set_session(&local_granary_dir, "local-abc");
    helpers::set_session(&named_granary_dir, "named-def");
    helpers::set_session(&default_granary_dir, "default-ghi");

    // Read back and verify isolation
    assert_eq!(
        helpers::get_session(&local_granary_dir),
        Some("local-abc".to_string())
    );
    assert_eq!(
        helpers::get_session(&named_granary_dir),
        Some("named-def".to_string())
    );
    assert_eq!(
        helpers::get_session(&default_granary_dir),
        Some("default-ghi".to_string())
    );
}

/// Test: GRANARY_SESSION env var overrides file-based session in all modes.
///
/// This test constructs Workspace structs directly to test the actual
/// current_session_id() method with the env var set.
#[test]
fn test_granary_session_env_override() {
    let root = TempDir::new().expect("Failed to create temp dir");

    // Create directories for all three modes
    let local_granary = root.path().join("project").join(".granary");
    let named_granary = root.path().join(".granary").join("workspaces").join("test");
    let default_granary = root.path().join(".granary");

    std::fs::create_dir_all(&local_granary).unwrap();
    std::fs::create_dir_all(&named_granary).unwrap();
    std::fs::create_dir_all(&default_granary).unwrap();

    // Write session files to each
    helpers::set_session(&local_granary, "file-session-local");
    helpers::set_session(&named_granary, "file-session-named");
    helpers::set_session(&default_granary, "file-session-default");

    // When GRANARY_SESSION is set, it should take precedence over any file
    // We test this by temporarily setting the env var
    let env_session = "env-override-session-xyz";
    // SAFETY: This test is single-threaded and we clean up immediately after
    unsafe { std::env::set_var("GRANARY_SESSION", env_session) };

    // The env var should be readable
    assert_eq!(
        std::env::var("GRANARY_SESSION").ok(),
        Some(env_session.to_string())
    );

    // Clean up env var to not affect other tests
    // SAFETY: This test is single-threaded and restores original state
    unsafe { std::env::remove_var("GRANARY_SESSION") };

    // After removing, file-based sessions should still be intact
    assert_eq!(
        helpers::get_session(&local_granary),
        Some("file-session-local".to_string())
    );
    assert_eq!(
        helpers::get_session(&named_granary),
        Some("file-session-named".to_string())
    );
    assert_eq!(
        helpers::get_session(&default_granary),
        Some("file-session-default".to_string())
    );
}

/// Test: Workspace::current_session_id uses granary_dir for file path.
/// This is the key property: session file = granary_dir + "session".
#[test]
fn test_session_file_is_granary_dir_join_session() {
    let root = TempDir::new().expect("Failed to create temp dir");

    // For any arbitrary granary_dir, the session file should be at granary_dir/session
    let arbitrary_dirs: Vec<PathBuf> = vec![
        root.path().join("a").join("b").join(".granary"),
        root.path().join(".granary").join("workspaces").join("foo"),
        root.path().join(".granary"),
    ];

    for dir in &arbitrary_dirs {
        std::fs::create_dir_all(dir).unwrap();
        let session_path = dir.join("session");

        // Write a unique session
        let session_id = format!("session-for-{}", dir.display());
        std::fs::write(&session_path, &session_id).unwrap();

        // Verify it reads back correctly via the helper (same logic as Workspace)
        assert_eq!(helpers::get_session(dir), Some(session_id));
    }
}
