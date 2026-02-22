### Fix task.next events not firing on project dependency completion

Workers subscribing to `task.next` events were never notified when a project dependency was satisfied through auto-completion. This caused tasks in dependent projects to sit idle even though `granary next --all` correctly showed them as actionable.

**Root cause:** The `trg_task_next_on_project_dep_completed` trigger fired when a project's status changed to `'done'` or `'archived'`, but the auto-complete system sets projects to `'completed'` — a status the trigger didn't recognize. Since no code path ever sets a project to `'done'`, this cascade trigger has never fired.

The trigger now recognizes `'completed'` as a valid completion status, so it fires when auto-complete transitions a project. All other `task.next` triggers and Rust-side `next` queries updated to include `'completed'` in project dependency status checks for consistency.

### Fix task.next event loop on todo task updates

The `trg_task_next_on_status_todo` trigger was missing an `OLD.status != 'todo'` guard, causing it to fire on **any** update to a task with `'todo'` status — not just transitions **to** `'todo'`. When a worker set `owner` or `worker_ids` on a still-todo task, this re-emitted `task.next`, which the worker consumed, updated the task again, and created a ~1-second event loop that only broke when the runner finally set the task to `in_progress`.

The trigger now only fires when the status actually changes to `'todo'` from a different status. This was a pre-existing bug that became visible once the project dependency cascade fix started emitting events that previously never fired.