### Daemon shutdown now cleans up worker and run state

Previously, when the granary daemon shut down (via `granary daemon stop`, signals, or any other reason), workers were signalled to stop but their database status was never updated. This left workers permanently marked as `running` in the global database with their active runs also stuck in `running` state.

On next daemon start, `restore_workers` would find these stale "running" workers and attempt to respawn them — potentially picking up orphaned runs from completely unrelated workspaces. Manually stopping individual workers worked correctly because `stop_worker` properly updated the DB, but the bulk `shutdown_all` path (used on daemon exit) skipped this cleanup entirely.

`shutdown_all` now mirrors the same cleanup that `stop_worker` does: after signalling and waiting for workers to finish, it marks each worker as `stopped` and cancels any active runs in the database.