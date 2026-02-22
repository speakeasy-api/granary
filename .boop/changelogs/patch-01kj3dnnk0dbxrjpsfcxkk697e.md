### Fix task.next event loop on todo task updates

Workers subscribing to `task.next` events experienced an event loop where the same task would emit repeated `task.next` events approximately every second. This caused unnecessary event spam and wasted runner capacity.

**Root cause:** The `trg_task_next_on_status_todo` trigger fired on any update to a task with `status = 'todo'`, not just on transitions *to* `todo`. When a worker claimed a task (setting `owner`, `worker_ids`, etc.), the update triggered another `task.next` event, which the daemon dispatched back to a worker, creating an infinite loop at the poll interval.

**What changed:**

- Added `AND OLD.status != 'todo'` guard to `trg_task_next_on_status_todo`, so it only fires on actual status transitions to `todo` — not on metadata updates to tasks already in `todo` status
- Added e2e tests verifying that owner/worker updates on todo tasks don't re-emit `task.next`
- Added full lifecycle test covering project dependency cascade through worker-style updates