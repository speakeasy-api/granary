### Project dependency filtering in next commands

The `next`, `next --all`, `project.next`, and `task.next` mechanisms now correctly respect project-level dependencies. Previously, only task dependencies were checked — so when Project A depended on Project B, tasks from both projects would appear as actionable even if Project B still had incomplete work.

Now, tasks in a project with unmet project dependencies are excluded from all next-task queries and event triggers. A project dependency is considered met when the dependency project is either `done` or `archived`.

This also fixes the `initiative ... next` command, which had a similar issue where archiving a dependency project would leave dependents permanently blocked (it only checked task status, not project status).

### Cascade trigger for project completion

A new SQLite trigger (`trg_task_next_on_project_dep_completed`) automatically emits `task.next` events when a dependency project transitions to `done` or `archived`. This ensures workers subscribed to `task.next` are notified when project-level blockers are resolved, without requiring any manual intervention.

### E2E test coverage for project dependency blocking

Added a dedicated end-to-end test suite (`tests/project_dep_next_e2e.rs`) that runs the actual `granary` binary in an isolated sandbox to verify project dependency filtering works correctly across `next`, `next --all`, and `initiative next`. Tests cover blocking, unblocking via completion, unblocking via archival, and partial dependency satisfaction.