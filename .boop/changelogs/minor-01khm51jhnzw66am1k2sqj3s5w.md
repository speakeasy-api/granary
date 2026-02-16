### Task–worker/run association

When a worker picks up a task event, granary now automatically records which workers and runs have operated on it. Each task gains two new JSON-array columns — `worker_ids` and `run_ids` — that accumulate IDs as work is performed. The first run to claim an unowned task also sets itself as the task's `owner`, giving downstream automation a single ID to query for the "current" run.

This means you can trace exactly which workers touched a task and in what order, without having to join across run tables manually.

### Environment variables for spawned processes

Every process spawned by a worker now receives two new environment variables:

- `GRANARY_WORKER_ID` — the ID of the worker that spawned the process
- `GRANARY_RUN_ID` — the ID of the current run

Scripts and actions can use these to call back into granary, tag artifacts, or correlate logs without needing to parse context from the event payload.

### Clickable worker/run links in Silo

Task detail views across all three screens (initiative detail, project detail, and task list) now display worker and run IDs as clickable links that navigate directly to the corresponding worker or run detail screen.