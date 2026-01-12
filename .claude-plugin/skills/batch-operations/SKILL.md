---
name: granary-batch-operations
description: Perform reliable batch updates using structured operations in granary. Use for bulk task updates, importing data, or programmatic state changes.
---

# Batch Operations

Granary provides two modes for executing multiple operations: **apply** (atomic/transactional) and **batch** (streaming/independent).

## Apply (Atomic Batch)

Use `granary apply` when all operations must succeed together or fail together. This is transactional - if any operation fails, none are applied.

```bash
cat <<'JSON' | granary apply --stdin
{"ops": [
  {"op": "task.update", "id": "TASK-001", "status": "done"},
  {"op": "task.update", "id": "TASK-002", "status": "in_progress"},
  {"op": "comment.create", "task_id": "TASK-001", "body": "Completed as part of batch update"}
]}
JSON
```

Key characteristics:

- All operations succeed or all fail (rollback on error)
- Single JSON object with `ops` array
- Best for related changes that must be consistent

## Batch (Streaming)

Use `granary batch` when operations are independent and you want to process them individually. Each operation runs independently - failures don't affect other operations.

```bash
cat operations.jsonl | granary batch --stdin
```

Where `operations.jsonl` contains one operation per line:

```jsonl
{"op": "task.update", "id": "TASK-001", "status": "done"}
{"op": "task.update", "id": "TASK-002", "status": "done"}
{"op": "task.create", "title": "New task from import", "project": "my-project"}
```

Key characteristics:

- Each operation independent (continues on error)
- JSONL format (one JSON object per line)
- Best for bulk imports or independent updates

## Operation Types

### Task Operations

**task.create** - Create a new task

```json
{
  "op": "task.create",
  "title": "Task title",
  "project": "project-id",
  "status": "todo",
  "priority": "medium"
}
```

**task.update** - Update an existing task

```json
{ "op": "task.update", "id": "TASK-001", "status": "done", "priority": "high" }
```

**task.delete** - Delete a task

```json
{ "op": "task.delete", "id": "TASK-001" }
```

### Comment Operations

**comment.create** - Add a comment to a task

```json
{ "op": "comment.create", "task_id": "TASK-001", "body": "Comment text here" }
```

### Session Operations

**session.add_scope** - Add a task to the current session scope

```json
{ "op": "session.add_scope", "task_id": "TASK-001" }
```

**session.rm_scope** - Remove a task from the current session scope

```json
{ "op": "session.rm_scope", "task_id": "TASK-001" }
```

### Project Operations

**project.update** - Update project settings

```json
{
  "op": "project.update",
  "id": "my-project",
  "description": "Updated description"
}
```

## Use Cases

### Bulk Status Updates

Mark multiple tasks as complete after a release:

```bash
cat <<'JSON' | granary apply --stdin
{"ops": [
  {"op": "task.update", "id": "TASK-101", "status": "done"},
  {"op": "task.update", "id": "TASK-102", "status": "done"},
  {"op": "task.update", "id": "TASK-103", "status": "done"}
]}
JSON
```

### Import from External Systems

Import tasks from a CSV or external tracker:

```bash
# Generate JSONL from your source data
cat imported_tasks.jsonl | granary batch --stdin
```

### CI/CD Integration

Automatically update task status from CI pipelines:

```bash
# In your CI script
cat <<'JSON' | granary apply --stdin
{"ops": [
  {"op": "task.update", "id": "${TASK_ID}", "status": "done"},
  {"op": "comment.create", "task_id": "${TASK_ID}", "body": "Deployed in build #${BUILD_NUMBER}"}
]}
JSON
```

### Session Scope Management

Set up a working session with multiple tasks:

```bash
cat <<'JSON' | granary apply --stdin
{"ops": [
  {"op": "session.add_scope", "task_id": "TASK-001"},
  {"op": "session.add_scope", "task_id": "TASK-002"},
  {"op": "session.add_scope", "task_id": "TASK-003"}
]}
JSON
```

## Error Handling

### Apply Mode (All-or-Nothing)

- If any operation fails validation or execution, the entire batch is rolled back
- No partial state changes occur
- Error message indicates which operation failed

### Batch Mode (Continue on Error)

- Each operation runs independently
- Failed operations are logged but don't stop processing
- Results include success/failure status for each operation
- Useful for large imports where some failures are acceptable

## Best Practices

1. **Use apply for related changes** - When changes must be consistent (e.g., updating a task and adding a completion comment)

2. **Use batch for imports** - When processing large volumes of independent operations

3. **Validate before applying** - For critical operations, consider a dry-run or validation step

4. **Keep operations small** - Large atomic batches increase rollback risk; break into logical groups when possible
