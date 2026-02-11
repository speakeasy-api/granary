//! Integration tests for workspace modes: global default, named workspaces,
//! local workspaces, and the resolution order.
//!
//! These tests verify the workspace resolution logic described in the
//! Global-First Workspaces RFC. Each test uses temp directories and sets
//! HOME/GRANARY_HOME env vars to isolate from the real user environment.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tempfile::TempDir;

// ── Helpers ───────────────────────────────────────────────────────────

/// Find the granary CLI binary in the target directory.
fn granary_bin() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let target_dir = PathBuf::from(manifest_dir).join("target");

    let debug_path = target_dir.join("debug").join("granary");
    if debug_path.exists() {
        return debug_path;
    }

    let release_path = target_dir.join("release").join("granary");
    if release_path.exists() {
        return release_path;
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling_path = dir.join("granary");
        if sibling_path.exists() {
            return sibling_path;
        }
    }

    panic!(
        "granary binary not found. Build it first with 'cargo build'. Searched in: {:?}",
        target_dir
    );
}

/// Get the canonical (symlink-resolved) path for a temp directory.
/// On macOS, /var/folders is a symlink to /private/var/folders, which causes
/// path comparison failures between HOME env var and env::current_dir().
fn canonical(path: &std::path::Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Run a granary command in a controlled environment.
/// Returns (success, stdout, stderr).
fn run_granary(
    args: &[&str],
    home: &std::path::Path,
    cwd: &std::path::Path,
    extra_env: &[(&str, &str)],
) -> (bool, String, String) {
    let mut cmd = Command::new(granary_bin());
    cmd.args(args)
        .env("HOME", home)
        .env_remove("GRANARY_HOME")
        .env_remove("GRANARY_SESSION")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, val) in extra_env {
        cmd.env(key, val);
    }

    let output = cmd.output().expect("Failed to run granary");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

/// Run granary and assert success.
fn run_granary_ok(
    args: &[&str],
    home: &std::path::Path,
    cwd: &std::path::Path,
    extra_env: &[(&str, &str)],
) -> String {
    let (ok, stdout, stderr) = run_granary(args, home, cwd, extra_env);
    assert!(
        ok,
        "granary {} failed.\nstdout: {}\nstderr: {}",
        args.join(" "),
        stdout,
        stderr
    );
    stdout
}

/// Run granary and assert failure.
fn run_granary_err(
    args: &[&str],
    home: &std::path::Path,
    cwd: &std::path::Path,
    extra_env: &[(&str, &str)],
) -> String {
    let (ok, stdout, stderr) = run_granary(args, home, cwd, extra_env);
    assert!(
        !ok,
        "granary {} should have failed.\nstdout: {}\nstderr: {}",
        args.join(" "),
        stdout,
        stderr
    );
    stderr
}

// ── 1. Default workspace (no init) ───────────────────────────────────

#[test]
fn test_default_workspace_no_init() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("myapp");
    fs::create_dir_all(&work_dir).unwrap();

    // Without any init, `granary workspace --json` should resolve to default mode
    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);

    // Should report default mode
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("Invalid JSON: {}\nstdout: {}", e, stdout));
    assert_eq!(json["mode"], "default");
    assert_eq!(json["name"], "default");

    // Database path should be ~/.granary/granary.db
    let db_path = json["database"].as_str().unwrap();
    assert!(
        db_path.ends_with(".granary/granary.db"),
        "Database should be at ~/.granary/granary.db, got: {}",
        db_path
    );
}

#[test]
fn test_default_workspace_auto_creates_config_dir() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("code");
    fs::create_dir_all(&work_dir).unwrap();

    // ~/.granary should not exist yet
    assert!(!home.join(".granary").exists());

    // Running any command that needs a workspace should auto-create ~/.granary/
    let _stdout = run_granary_ok(&["workspace"], &home, &work_dir, &[]);

    // ~/.granary/ should now exist
    assert!(home.join(".granary").exists());
}

// ── 2. Named workspace ───────────────────────────────────────────────

#[test]
fn test_named_workspace_init() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Initialize a named workspace
    let stdout = run_granary_ok(
        &["workspace", "init", "--name", "myapp"],
        &home,
        &work_dir,
        &[],
    );
    assert!(
        stdout.contains("Initialized workspace \"myapp\""),
        "Expected init message, got: {}",
        stdout
    );

    // Verify database was created at ~/.granary/workspaces/myapp/granary.db
    let db_path = home
        .join(".granary")
        .join("workspaces")
        .join("myapp")
        .join("granary.db");
    assert!(db_path.exists(), "Named workspace database should exist");

    // Verify registry was created
    let registry_path = home
        .join(".granary")
        .join("workspaces")
        .join("registry.json");
    assert!(registry_path.exists(), "Registry file should exist");

    let registry_content = fs::read_to_string(&registry_path).unwrap();
    let registry: serde_json::Value = serde_json::from_str(&registry_content).unwrap();
    assert!(
        registry["workspaces"]["myapp"].is_object(),
        "Registry should contain workspace 'myapp'"
    );
}

#[test]
fn test_named_workspace_resolution() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Init named workspace
    run_granary_ok(
        &["workspace", "init", "--name", "myapp"],
        &home,
        &work_dir,
        &[],
    );

    // Verify workspace info shows named mode
    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["mode"], "named");
    assert_eq!(json["name"], "myapp");
}

#[test]
fn test_named_workspace_derives_name_from_dir() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("cool-project");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Init without --name; should derive from directory name
    let stdout = run_granary_ok(&["workspace", "init"], &home, &work_dir, &[]);
    assert!(
        stdout.contains("cool-project"),
        "Should derive workspace name from dir, got: {}",
        stdout
    );
}

// ── 3. Local workspace ───────────────────────────────────────────────

#[test]
fn test_local_workspace_init() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("localapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Initialize a local workspace
    let stdout = run_granary_ok(&["workspace", "init", "--local"], &home, &work_dir, &[]);
    assert!(
        stdout.contains("local workspace") || stdout.contains("Initialized local"),
        "Expected local init message, got: {}",
        stdout
    );

    // Verify .granary/ was created in cwd
    let local_granary = work_dir.join(".granary");
    assert!(local_granary.exists(), ".granary/ should exist in cwd");

    // Verify database exists
    let db_path = local_granary.join("granary.db");
    assert!(db_path.exists(), "Local database should exist");
}

#[test]
fn test_local_workspace_resolution() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("localapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    run_granary_ok(&["workspace", "init", "--local"], &home, &work_dir, &[]);

    // Verify workspace info shows local mode
    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["mode"], "local");
}

#[test]
fn test_local_workspace_not_in_registry() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("localapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    run_granary_ok(&["workspace", "init", "--local"], &home, &work_dir, &[]);

    // Registry should not exist (no named workspaces created)
    // or should not contain this workspace
    let registry_path = home
        .join(".granary")
        .join("workspaces")
        .join("registry.json");

    if registry_path.exists() {
        let content = fs::read_to_string(&registry_path).unwrap();
        let registry: serde_json::Value = serde_json::from_str(&content).unwrap();
        let roots = registry["roots"].as_object();
        if let Some(roots) = roots {
            for (path_key, _ws) in roots {
                let root_path = PathBuf::from(path_key);
                assert_ne!(
                    root_path, work_dir,
                    "Local workspace should NOT be in the registry"
                );
            }
        }
    }
}

// ── 4. Resolution order (local > registry > default) ─────────────────

#[test]
fn test_local_takes_precedence_over_registry() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // First create a named workspace
    run_granary_ok(
        &["workspace", "init", "--name", "myapp"],
        &home,
        &work_dir,
        &[],
    );

    // Verify it's named
    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["mode"], "named");

    // Now also create a local .granary/ in the same directory
    let local_granary = work_dir.join(".granary");
    fs::create_dir_all(&local_granary).unwrap();
    // Create a DB file so it looks like a valid local workspace
    fs::write(local_granary.join("granary.db"), b"").unwrap();

    // Resolution should now find local first
    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json["mode"], "local",
        "Local .granary/ should take precedence over registry"
    );
}

#[test]
fn test_registry_takes_precedence_over_default() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Without init, should resolve to default
    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["mode"], "default");

    // Init a named workspace -> should now resolve to named
    run_granary_ok(
        &["workspace", "init", "--name", "myapp"],
        &home,
        &work_dir,
        &[],
    );

    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json["mode"], "named",
        "Registry entry should take precedence over default"
    );
}

#[test]
fn test_granary_home_takes_precedence_over_everything() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("projects").join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Set up a named workspace
    run_granary_ok(
        &["workspace", "init", "--name", "myapp"],
        &home,
        &work_dir,
        &[],
    );

    // Also set up a local workspace
    run_granary_ok(
        &["workspace", "init", "--local", "--force"],
        &home,
        &work_dir,
        &[],
    );

    // Create a custom GRANARY_HOME directory
    let custom_home = home.join("custom-granary");
    fs::create_dir_all(custom_home.join(".granary")).unwrap();
    fs::write(custom_home.join(".granary").join("granary.db"), b"").unwrap();

    // GRANARY_HOME should override everything
    let stdout = run_granary_ok(
        &["workspace", "--json"],
        &home,
        &work_dir,
        &[("GRANARY_HOME", custom_home.to_str().unwrap())],
    );
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // GRANARY_HOME uses the path directly, should not resolve to "named" or "default"
    let db_path = json["database"].as_str().unwrap();
    assert!(
        db_path.contains("custom-granary"),
        "GRANARY_HOME should be used, got db_path: {}",
        db_path
    );
}

// ── 5. Registry ancestor matching ────────────────────────────────────

#[test]
fn test_registry_ancestor_matching() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("work");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Register /work as a named workspace
    run_granary_ok(
        &["workspace", "init", "--name", "work"],
        &home,
        &work_dir,
        &[],
    );

    // Create a subdirectory and check from there
    let sub_dir = work_dir.join("project-a").join("src");
    fs::create_dir_all(&sub_dir).unwrap();

    let stdout = run_granary_ok(&["workspace", "--json"], &home, &sub_dir, &[]);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json["mode"], "named",
        "Subdirectory should resolve to ancestor's workspace"
    );
    assert_eq!(
        json["name"], "work",
        "Subdirectory should resolve to 'work' workspace"
    );
}

#[test]
fn test_registry_deepest_ancestor_wins() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());

    // Create two nested workspace directories
    let parent_dir = home.join("work");
    let child_dir = parent_dir.join("special-project");
    fs::create_dir_all(&parent_dir).unwrap();
    fs::create_dir_all(parent_dir.join(".git")).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::create_dir_all(child_dir.join(".git")).unwrap();

    // Register parent as "work"
    run_granary_ok(
        &["workspace", "init", "--name", "work"],
        &home,
        &parent_dir,
        &[],
    );

    // Register child as "special"
    run_granary_ok(
        &["workspace", "init", "--name", "special"],
        &home,
        &child_dir,
        &[],
    );

    // From within the child, should resolve to "special" (deepest ancestor)
    let deep_dir = child_dir.join("src").join("lib");
    fs::create_dir_all(&deep_dir).unwrap();

    let stdout = run_granary_ok(&["workspace", "--json"], &home, &deep_dir, &[]);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json["name"], "special",
        "Deepest registered ancestor should win"
    );

    // From a sibling of child, should resolve to "work" (parent)
    let sibling_dir = parent_dir.join("other-project");
    fs::create_dir_all(&sibling_dir).unwrap();

    let stdout = run_granary_ok(&["workspace", "--json"], &home, &sibling_dir, &[]);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json["name"], "work",
        "Sibling should resolve to parent workspace"
    );
}

// ── 6. Workspace list ────────────────────────────────────────────────

#[test]
fn test_workspace_list() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());

    // Create two named workspaces in separate directories
    let dir_a = home.join("project-a");
    let dir_b = home.join("project-b");
    fs::create_dir_all(&dir_a).unwrap();
    fs::create_dir_all(dir_a.join(".git")).unwrap();
    fs::create_dir_all(&dir_b).unwrap();
    fs::create_dir_all(dir_b.join(".git")).unwrap();

    run_granary_ok(
        &["workspace", "init", "--name", "alpha"],
        &home,
        &dir_a,
        &[],
    );
    run_granary_ok(&["workspace", "init", "--name", "beta"], &home, &dir_b, &[]);

    // List workspaces
    let stdout = run_granary_ok(&["workspace", "list", "--json"], &home, &dir_a, &[]);

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let workspaces = json.as_array().expect("Expected JSON array");

    // Should have at least: default, alpha, beta
    let names: Vec<&str> = workspaces
        .iter()
        .filter_map(|w| w["name"].as_str())
        .collect();
    assert!(
        names.contains(&"default"),
        "Should include default workspace"
    );
    assert!(names.contains(&"alpha"), "Should include 'alpha' workspace");
    assert!(names.contains(&"beta"), "Should include 'beta' workspace");

    // Verify modes
    for ws in workspaces {
        let name = ws["name"].as_str().unwrap();
        let mode = ws["mode"].as_str().unwrap();
        match name {
            "default" => assert_eq!(mode, "default"),
            "alpha" | "beta" => assert_eq!(mode, "named"),
            _ => {} // may include local workspace
        }
    }
}

// ── 7. Workspace info ────────────────────────────────────────────────

#[test]
fn test_workspace_info_default() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("somewhere");
    fs::create_dir_all(&work_dir).unwrap();

    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["name"], "default");
    assert_eq!(json["mode"], "default");
    assert!(json["database"].as_str().unwrap().contains("granary.db"));
}

#[test]
fn test_workspace_info_named() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("myproject");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    run_granary_ok(
        &["workspace", "init", "--name", "mywork"],
        &home,
        &work_dir,
        &[],
    );

    let stdout = run_granary_ok(&["workspace", "--json"], &home, &work_dir, &[]);

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["name"], "mywork");
    assert_eq!(json["mode"], "named");
    assert!(
        json["database"]
            .as_str()
            .unwrap()
            .contains("workspaces/mywork/granary.db")
    );
}

// ── 8. Init validation checks ────────────────────────────────────────

#[test]
fn test_init_already_initialized_locally() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Init local workspace
    run_granary_ok(&["workspace", "init", "--local"], &home, &work_dir, &[]);

    // Trying to init again (local) should fail
    let stderr = run_granary_err(&["workspace", "init", "--local"], &home, &work_dir, &[]);
    assert!(
        stderr.contains("already initialized") || stderr.contains("already exists"),
        "Should report already initialized, got: {}",
        stderr
    );

    // Trying to init global should also fail (local .granary/ exists)
    let stderr = run_granary_err(
        &["workspace", "init", "--name", "myapp"],
        &home,
        &work_dir,
        &[],
    );
    assert!(
        stderr.contains("already initialized") || stderr.contains("already exists"),
        "Should report already initialized for global init too, got: {}",
        stderr
    );
}

#[test]
fn test_init_force_overrides_existing() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // Init local workspace
    run_granary_ok(&["workspace", "init", "--local"], &home, &work_dir, &[]);

    // With --force, should succeed
    run_granary_ok(
        &["workspace", "init", "--local", "--force"],
        &home,
        &work_dir,
        &[],
    );
}

#[test]
fn test_init_nested_workspace_rejected() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let parent_dir = home.join("parent");
    let child_dir = parent_dir.join("child");
    fs::create_dir_all(&parent_dir).unwrap();
    fs::create_dir_all(parent_dir.join(".git")).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::create_dir_all(child_dir.join(".git")).unwrap();

    // Init local workspace in parent
    run_granary_ok(&["workspace", "init", "--local"], &home, &parent_dir, &[]);

    // Trying to init in child should fail (nested workspace)
    let stderr = run_granary_err(&["workspace", "init", "--local"], &home, &child_dir, &[]);
    assert!(
        stderr.contains("inside workspace") || stderr.contains("nested"),
        "Should reject nested workspace, got: {}",
        stderr
    );
}

#[test]
fn test_init_nested_workspace_force() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let parent_dir = home.join("parent");
    let child_dir = parent_dir.join("child");
    fs::create_dir_all(&parent_dir).unwrap();
    fs::create_dir_all(parent_dir.join(".git")).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::create_dir_all(child_dir.join(".git")).unwrap();

    run_granary_ok(&["workspace", "init", "--local"], &home, &parent_dir, &[]);

    // With --force, nested init should succeed
    run_granary_ok(
        &["workspace", "init", "--local", "--force"],
        &home,
        &child_dir,
        &[],
    );
}

#[test]
fn test_init_not_git_root_rejected() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());

    // Create a git repo in parent, but init from a subdirectory
    let git_root = home.join("repo");
    let sub_dir = git_root.join("packages").join("sub");
    fs::create_dir_all(&git_root).unwrap();
    fs::create_dir_all(git_root.join(".git")).unwrap();
    fs::create_dir_all(&sub_dir).unwrap();

    // Init from subdirectory (not the git root) should fail
    let stderr = run_granary_err(&["workspace", "init", "--local"], &home, &sub_dir, &[]);
    assert!(
        stderr.contains("git") || stderr.contains("root"),
        "Should reject init not at git root, got: {}",
        stderr
    );
}

#[test]
fn test_init_skip_git_check() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let git_root = home.join("repo");
    let sub_dir = git_root.join("packages").join("sub");
    fs::create_dir_all(&git_root).unwrap();
    fs::create_dir_all(git_root.join(".git")).unwrap();
    fs::create_dir_all(&sub_dir).unwrap();

    // With --skip-git-check, should succeed
    run_granary_ok(
        &["workspace", "init", "--local", "--skip-git-check"],
        &home,
        &sub_dir,
        &[],
    );
}

#[test]
fn test_init_no_git_is_fine() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("no-git-project");
    fs::create_dir_all(&work_dir).unwrap();

    // A directory with no .git at all should be fine (not a git project)
    run_granary_ok(&["workspace", "init", "--local"], &home, &work_dir, &[]);
}

// ── Alias: `granary init` ────────────────────────────────────────────

#[test]
fn test_init_alias_local() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // `granary init --local` should work like `granary workspace init --local`
    run_granary_ok(&["init", "--local"], &home, &work_dir, &[]);

    assert!(
        work_dir.join(".granary").exists(),
        "granary init --local should create .granary/ in cwd"
    );
}

#[test]
fn test_init_alias_global() {
    let temp = TempDir::new().unwrap();
    let home = canonical(temp.path());
    let work_dir = home.join("myapp");
    fs::create_dir_all(&work_dir).unwrap();
    fs::create_dir_all(work_dir.join(".git")).unwrap();

    // `granary init` (without --local) creates a named workspace
    let stdout = run_granary_ok(&["init"], &home, &work_dir, &[]);
    assert!(
        stdout.contains("Initialized workspace"),
        "granary init should create a named workspace, got: {}",
        stdout
    );
}
