### Fix task.next events not firing on project dependency completion

Workers subscribing to `task.next` events were never notified when a project dependency was satisfied through auto-completion. This caused tasks in dependent projects to sit idle even though `granary next --all` correctly showed them as actionable.

**Root cause:** The `trg_task_next_on_project_dep_completed` trigger fired when a project's status changed to `'done'` or `'archived'`, but the auto-complete system sets projects to `'completed'` — a status the trigger didn't recognize. Since no code path ever sets a project to `'done'`, this cascade trigger has never fired.

**What changed:**

- The `trg_task_next_on_project_dep_completed` trigger now recognizes `'completed'` as a valid completion status, so it fires when auto-complete transitions a project
- All other `task.next` triggers updated to include `'completed'` in project dependency status checks for consistency
- Rust-side `next` queries updated to treat `'completed'` projects as satisfied dependencies (defensive, previously handled by the task-existence subquery)

**Impact:** Workers will now correctly pick up tasks as soon as all tasks in a dependency project are done, without needing the dependency project to be manually archived.