//! End-to-end tests for project dependency filtering in `next` commands.
//!
//! These tests run the actual `granary` binary in an isolated sandbox
//! (HOME set to a temp dir) to verify that `next`, `next --all`, and
//! `initiative ... next` respect project-level dependencies.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

// ── Helpers ───────────────────────────────────────────────────────────

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

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

struct Sandbox {
    _tmp: TempDir,
    home: PathBuf,
    cwd: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let home = canonical(tmp.path());
        let cwd = home.join("workspace");
        fs::create_dir_all(&cwd).unwrap();

        // Initialize the default workspace
        let output = Command::new(granary_bin())
            .arg("init")
            .env("HOME", &home)
            .env_remove("GRANARY_HOME")
            .env_remove("GRANARY_SESSION")
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("failed to run granary init");
        assert!(
            output.status.success(),
            "granary init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        Sandbox {
            _tmp: tmp,
            home,
            cwd,
        }
    }

    fn run(&self, args: &[&str]) -> (bool, String, String) {
        let mut cmd = Command::new(granary_bin());
        cmd.args(args)
            .env("HOME", &self.home)
            .env_remove("GRANARY_HOME")
            .env_remove("GRANARY_SESSION")
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().expect("Failed to run granary");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        (output.status.success(), stdout, stderr)
    }

    fn run_ok(&self, args: &[&str]) -> String {
        let (ok, stdout, stderr) = self.run(args);
        assert!(
            ok,
            "granary {:?} failed.\nstdout: {}\nstderr: {}",
            args, stdout, stderr
        );
        stdout
    }
}

/// Extract an ID from `--json` output (looks for `"id":"<value>"`).
fn extract_id(json_output: &str) -> String {
    let collapsed: String = json_output.chars().filter(|c| !c.is_whitespace()).collect();
    let marker = "\"id\":\"";
    let start = collapsed
        .find(marker)
        .unwrap_or_else(|| panic!("no id found in: {}", json_output))
        + marker.len();
    let end = collapsed[start..]
        .find('"')
        .expect("unterminated id string")
        + start;
    collapsed[start..end].to_string()
}

// ── Tests ─────────────────────────────────────────────────────────────

#[test]
fn test_next_excludes_tasks_blocked_by_project_dependency() {
    let sb = Sandbox::new();

    // Create two projects: Backend (dependency) and Frontend (depends on Backend)
    let backend_id = extract_id(&sb.run_ok(&["projects", "create", "Backend", "--json"]));
    let frontend_id = extract_id(&sb.run_ok(&["projects", "create", "Frontend", "--json"]));

    // Frontend depends on Backend
    sb.run_ok(&["project", &frontend_id, "deps", "add", &backend_id]);

    // Create tasks in both projects
    let backend_task_id = extract_id(&sb.run_ok(&[
        "project",
        &backend_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Setup database",
        "--priority",
        "P0",
        "--json",
    ]));
    let frontend_task_id = extract_id(&sb.run_ok(&[
        "project",
        &frontend_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Setup React",
        "--priority",
        "P0",
        "--json",
    ]));

    // `next` should return the Backend task, NOT the Frontend task
    let next_output = sb.run_ok(&["next", "--json"]);
    assert!(
        next_output.contains(&backend_task_id),
        "next should return the backend task (unblocked), got: {}",
        next_output
    );
    assert!(
        !next_output.contains(&frontend_task_id),
        "next should NOT return the frontend task (blocked by project dep), got: {}",
        next_output
    );

    // `next --all` should only include Backend task
    let next_all = sb.run_ok(&["next", "--all", "--json"]);
    assert!(
        next_all.contains(&backend_task_id),
        "next --all should include the backend task, got: {}",
        next_all
    );
    assert!(
        !next_all.contains(&frontend_task_id),
        "next --all should NOT include the frontend task, got: {}",
        next_all
    );
}

#[test]
fn test_next_unblocks_after_project_dependency_completed() {
    let sb = Sandbox::new();

    let backend_id = extract_id(&sb.run_ok(&["projects", "create", "Backend", "--json"]));
    let frontend_id = extract_id(&sb.run_ok(&["projects", "create", "Frontend", "--json"]));

    sb.run_ok(&["project", &frontend_id, "deps", "add", &backend_id]);

    let backend_task_id = extract_id(&sb.run_ok(&[
        "project",
        &backend_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Only backend task",
        "--json",
    ]));
    let frontend_task_id = extract_id(&sb.run_ok(&[
        "project",
        &frontend_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Frontend task",
        "--json",
    ]));

    // Before: frontend task should NOT be in next --all
    let before = sb.run_ok(&["next", "--all", "--json"]);
    assert!(
        !before.contains(&frontend_task_id),
        "frontend task should be blocked before backend completes"
    );

    // Complete the backend task (auto-completes the backend project)
    sb.run_ok(&["task", &backend_task_id, "done"]);

    // After: frontend task should now appear
    let after = sb.run_ok(&["next", "--all", "--json"]);
    assert!(
        after.contains(&frontend_task_id),
        "frontend task should be unblocked after backend project completed, got: {}",
        after
    );
}

#[test]
fn test_next_unblocks_after_project_dependency_archived() {
    let sb = Sandbox::new();

    let backend_id = extract_id(&sb.run_ok(&["projects", "create", "Backend", "--json"]));
    let frontend_id = extract_id(&sb.run_ok(&["projects", "create", "Frontend", "--json"]));

    sb.run_ok(&["project", &frontend_id, "deps", "add", &backend_id]);

    // Backend has an incomplete task (won't be completed)
    sb.run_ok(&[
        "project",
        &backend_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Incomplete task",
        "--json",
    ]);

    let frontend_task_id = extract_id(&sb.run_ok(&[
        "project",
        &frontend_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Frontend task",
        "--json",
    ]));

    // Before: blocked
    let before = sb.run_ok(&["next", "--all", "--json"]);
    assert!(
        !before.contains(&frontend_task_id),
        "frontend task should be blocked before backend is archived"
    );

    // Archive the backend project (even though it has incomplete tasks)
    sb.run_ok(&["project", &backend_id, "archive"]);

    // After: unblocked (archived satisfies dependency)
    let after = sb.run_ok(&["next", "--all", "--json"]);
    assert!(
        after.contains(&frontend_task_id),
        "frontend task should be unblocked after backend project archived, got: {}",
        after
    );
}

#[test]
fn test_next_stays_blocked_when_only_some_project_deps_completed() {
    let sb = Sandbox::new();

    // Create three projects: A, B (dependencies), and C (depends on both)
    let proj_a_id = extract_id(&sb.run_ok(&["projects", "create", "Dep-A", "--json"]));
    let proj_b_id = extract_id(&sb.run_ok(&["projects", "create", "Dep-B", "--json"]));
    let proj_c_id = extract_id(&sb.run_ok(&["projects", "create", "Dependent-C", "--json"]));

    // C depends on both A and B
    sb.run_ok(&["project", &proj_c_id, "deps", "add", &proj_a_id]);
    sb.run_ok(&["project", &proj_c_id, "deps", "add", &proj_b_id]);

    // Create tasks
    let task_a_id = extract_id(&sb.run_ok(&[
        "project",
        &proj_a_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Task in A",
        "--json",
    ]));
    sb.run_ok(&[
        "project",
        &proj_b_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Task in B",
        "--json",
    ]);
    let task_c_id = extract_id(&sb.run_ok(&[
        "project",
        &proj_c_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Task in C",
        "--json",
    ]));

    // Complete A (only one dep satisfied)
    sb.run_ok(&["task", &task_a_id, "done"]);

    // C should still be blocked (B is not done)
    let output = sb.run_ok(&["next", "--all", "--json"]);
    assert!(
        !output.contains(&task_c_id),
        "task in C should still be blocked when only A is done but B is not, got: {}",
        output
    );
}

#[test]
fn test_initiative_next_respects_project_dependencies() {
    let sb = Sandbox::new();

    // Create initiative
    let init_id = extract_id(&sb.run_ok(&["initiatives", "create", "Q1 Release", "--json"]));

    // Create two projects
    let backend_id = extract_id(&sb.run_ok(&["projects", "create", "Backend", "--json"]));
    let frontend_id = extract_id(&sb.run_ok(&["projects", "create", "Frontend", "--json"]));

    // Frontend depends on Backend
    sb.run_ok(&["project", &frontend_id, "deps", "add", &backend_id]);

    // Add both to initiative
    sb.run_ok(&["initiative", &init_id, "add-project", &backend_id]);
    sb.run_ok(&["initiative", &init_id, "add-project", &frontend_id]);

    // Create tasks
    let backend_task_id = extract_id(&sb.run_ok(&[
        "project",
        &backend_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Backend work",
        "--json",
    ]));
    let frontend_task_id = extract_id(&sb.run_ok(&[
        "project",
        &frontend_id,
        "tasks",
        "create",
        "--status",
        "todo",
        "Frontend work",
        "--json",
    ]));

    // Initiative next --all should only show backend task
    let next_output = sb.run_ok(&["initiative", &init_id, "next", "--all", "--json"]);
    assert!(
        next_output.contains(&backend_task_id),
        "initiative next should include backend task, got: {}",
        next_output
    );
    assert!(
        !next_output.contains(&frontend_task_id),
        "initiative next should NOT include frontend task (project dep unmet), got: {}",
        next_output
    );

    // Complete backend task → backend project auto-completes → frontend unblocked
    sb.run_ok(&["task", &backend_task_id, "done"]);

    let after = sb.run_ok(&["initiative", &init_id, "next", "--all", "--json"]);
    assert!(
        after.contains(&frontend_task_id),
        "initiative next should include frontend task after backend completed, got: {}",
        after
    );
}
