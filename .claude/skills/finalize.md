---
name: finalize
description: IMPORTANT! you must use this skill when a granary project is complete
---

# Finalize Project

After a project is complete (no outstanding tasks):

1. **Verify branch**: Ensure you're on a feature branch (not main/master)
2. **Format code**: Run `cargo fmt`
3. **Run tests**: Run `cargo test`
4. **Handle failures**: If tests fail, create new tasks to fix the failing tests and stop
5. **If all tests pass**:
   - Assess the changes made during the project
   - Increment the version in `Cargo.toml` according to semver:
     - **patch**: bug fixes, minor improvements
     - **minor**: new features, backwards-compatible changes
     - **major**: breaking changes
   - Create a PR using `gh pr create` with a summary of changes
