### Entity metadata

Tasks, projects, and initiatives now support an optional free-form JSON `metadata` field. This lets you attach arbitrary structured information — environment config, tracking IDs, custom fields — to any entity without being limited to text-only tags.

**CLI usage:**

```sh
granary project my-proj tasks create "deploy service" \
  --metadata '{"env": "production", "region": "us-east-1"}'

granary tasks update proj-task-1 \
  --metadata '{"env": "staging", "retries": 3}'
```

Metadata works the same way on projects (`project create/update --metadata`) and initiatives (`initiative create/update --metadata`).

**Output behavior:**

Metadata is included in `--output json` / `--json` output but is intentionally excluded from prompt and text formats, keeping those outputs clean for human and LLM consumption.

**Event template access:**

Metadata is embedded as a nested JSON object in all event payloads (task.created, task.updated, task.next, project.created, etc.), so event templates can reference individual metadata fields:

```
{metadata.env}
{metadata.region}
{metadata.config.timeout}
```

This enables metadata-driven automation — for example, a `task.next` handler that routes work based on `{metadata.env}` or `{metadata.worktree}`.