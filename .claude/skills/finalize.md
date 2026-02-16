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
   - Run the `/shipit` skill to determine the semver bump, record release notes via boop, commit, push, and open a PR
