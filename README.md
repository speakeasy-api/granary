## Granary design v0.2: a CLI "context hub" for agentic work

Granary's differentiator is not that it can represent projects and tasks (lots of tools can), but that it can represent **an ongoing agentic loop** as a first-class object, and then generate **machine-consumable context packs** (summary, next actions, blockers, handoff packets) on demand.

This mirrors what "plan → execute" workflows do in practice (separating planning artifacts from execution focus) while keeping everything addressable and updatable via CLI.

---

# 1) Product principles

### Goals

- **LLM-first I/O**: every command has stable, strict, machine-readable output (`--json`, `--format prompt`).
- **Local-first**: state stored locally in the repo (or user home), no network dependency.
- **Concurrency-tolerant**: multiple agents/sub-agents can write without corrupting state.
- **Session-centric**: "what's in context" is explicit and queryable.
- **Human legible by default**: readable tables/trees, but never at the expense of structured output.

### Non-goals

- Compete with Linear/Jira as the system of record.
- Be an LLM runner. Granary should be usable by Claude Code/Kiro/Cursor/your own orchestrator rather than replacing them.

---

# 2) Core objects and why they exist

You already have Projects, Tasks, Comments. The key missing primitives for agentic loops are **Session**, **Event log**, and **Artifacts/Attachments**.

## Workspace

A workspace is a directory boundary (typically a repo) that contains `.granary/`.

- Default resolution: walk up from CWD to find `.granary/`, similar to how Git finds `.git/`.
- Explicit override: `GRANARY_HOME`, `--workspace`.

## Project

Long-lived initiative.

Add two fields that matter a lot for agentic orchestration:

- `default_session_policy`: how new sessions should include this project (auto-pin open P0/P1 tasks, etc.)
- `steering_refs`: references to "rules / standards / conventions" (see **Steering** below; inspired by Kiro steering files).

## Task

Unit of work. Must support:

- Hierarchy (subtasks)
- Dependencies
- Ownership/claiming (important for multi-agent)
- Status transitions with auditability

Recommended additional fields for agent workflows:

- `blocked_reason` (string)
- `started_at`, `completed_at`
- `claim`: `{owner, claimed_at, lease_expires_at}` (optional but very helpful for parallel agents)
- `attention`: `{pinned: bool, focus_weight: int}` (drives summaries/context export)

## Comment

Comments shouldn't just be "chatty notes"—they're the backbone of durable context.

Add:

- `kind`: `note | progress | decision | blocker | handoff | incident | context`
- `meta` (JSON): structured breadcrumbs (e.g., `{ "pr": 123, "commit": "abc", "test": "pytest", "result": "fail" }`)

## Artifact

Anything you want to reference without stuffing it into a description/comment:

- File path
- URL
- Git commit / PR link
- Build log snippet location
- Dataset, output file, etc.

Artifacts matter because agentic loops produce _stuff_ continuously.

## Session (the centerpiece)

A **Session** is the container for _what is in context for a run_.

Why: agent frameworks regularly need a persistent "thread" with resumability and "state at time t". LangGraph, for example, formalizes persistence via checkpoints and thread state that can be inspected/resumed later.

Session fields:

- `id` (stable, URL-safe)
- `name`
- `created_at`, `updated_at`, `closed_at`
- `owner` (human or orchestrator, e.g. "Claude Code", "CI Agent", "You")
- `mode`: `plan | execute | review` (optional but powerful)
- `scope`:

  - `pinned_projects: []`
  - `pinned_tasks: []`
  - `pinned_comments: []`
  - `pinned_artifacts: []`

- `focus_task_id` (single "current" task)
- `variables` (key/value): for loop metadata (branch, target env, constraints)
- `checkpoints`: list of snapshots (see below)

### Session inheritance

Sub-agents inherit the session by reading:

- `GRANARY_SESSION=<id>` env var, or
- `.granary/session` pointer file (like Git HEAD).

This makes "global context" consistent across an agent-subagent tree.

---

# 3) ID scheme (make it boring and deterministic)

Your examples are fine, but tighten the rules so agents don't invent IDs.

## Project ID

`<slug>-<suffix>` where suffix is short and collision-resistant (base32/base36, 4–6 chars).

Example:

- `my-big-project-5h18`

Rule: slug is user-chosen/normalized; suffix is generated.

## Task ID

`<project_id>-task-<n>` where `<n>` is monotonically increasing per project.

Example:

- `my-big-project-5h18-task-321`

## Subtask ID

Two good options:

**Option A (recommended):** treat subtasks as normal tasks with `parent_task_id`

- ID: `my-big-project-5h18-task-322`
- `parent_task_id = ...-task-321`

**Option B:** explicit suffix

- `my-big-project-5h18-task-321-subtask-3`

Option A is simpler for dependencies and referencing.

## Comment ID

`<parent_id>-comment-<n>` (monotonic per parent)

Example:

- `my-big-project-5h18-task-321-comment-2`

---

# 4) Storage model (local, concurrent, auditable)

## Recommended backend: SQLite + event log

- Store canonical state in tables (projects/tasks/comments/sessions/artifacts).
- Record every mutation in an `events` table (append-only).
- Enable SQLite WAL to support concurrent reads/writes safely.

This gives you:

- "What changed?" timelines
- Better summaries (derive from events since last summary)
- Checkpointing and "time travel-ish" debugging (restore session snapshot)

This aligns with the "persistent state + checkpoints" philosophy used by agent workflow tooling.

---

# 5) CLI design: nouns, subresources, plus a few high-frequency verbs

You're already close. I'd standardize on:

- **Plural** for collections: `projects`, `tasks`, `sessions`
- **Singular** for addressing one: `project <id>`, `task <id>`, `session <id>`
- **Subresources** for containment: `project <id> tasks`, `task <id> comments`

Also: every command supports `--json` and stable exit codes.

## Global flags

```
granary --help
granary --json ...
granary --format table|json|yaml|md|prompt
granary --workspace /path/to/repo
granary --session <session_id>  # override current
```

---

## 5.1 Workspace/bootstrap

```
granary init                    # create .granary/
granary doctor                  # sanity checks (db readable, locks, schema)
granary config set key value
granary config get key
```

---

## 5.2 Projects

```
granary projects
granary projects create "My big project" --description "..." --owner "Claude Code" --tags web backend
granary project my-big-project-5h18
granary project my-big-project-5h18 update --description "..." --tags +mobile -backend
granary project my-big-project-5h18 archive
```

---

## 5.3 Tasks

Collections:

```
granary tasks                                            # in current session scope by default
granary tasks --all                                      # across workspace
granary tasks --status todo --owner "Claude Code" --tag web
```

Per project:

```
granary project my-big-project-5h18 tasks
granary project my-big-project-5h18 tasks create "Implement auth" --priority P0 --status todo
```

Per task:

```
granary task my-big-project-5h18-task-321
granary task my-big-project-5h18-task-321 update --status in_progress --owner "Agent A"
granary task my-big-project-5h18-task-321 done --comment "Merged PR #123"
granary task my-big-project-5h18-task-321 block --reason "Waiting on API keys"
granary task my-big-project-5h18-task-321 unblock
```

Dependencies:

```
granary task ... deps add my-big-project-5h18-task-100 my-big-project-5h18-task-101
granary task ... deps rm my-big-project-5h18-task-100
granary task ... graph --format md  # show dependency graph
```

Subtasks (Option A style):

```
granary task my-big-project-5h18-task-321 tasks create "Add unit tests" --priority P1
```

---

## 5.4 Comments (typed, structured context)

```
granary task ... comments
granary task ... comments create --kind progress --content "Implemented controller + tests pending"
granary task ... comments create --kind decision --content "Use Redis for rate limiting"
granary comment <comment_id> update --content "..."
```

---

# 6) Session commands (your missing cornerstone)

This is where Granary becomes "agent-loop native".

## 6.1 Start / use / end

```
granary sessions
granary session start "auth-implementation" --owner "Claude Code"
granary session current
granary session use sess-20260111-7f2c
granary session close --summary "Shipped auth MVP"
```

## 6.2 Scope management (what's in context)

```
granary session add project my-big-project-5h18
granary session add task my-big-project-5h18-task-321
granary session rm task my-big-project-5h18-task-999

granary focus my-big-project-5h18-task-321   # sets focus_task_id
granary pin my-big-project-5h18-task-321     # boosts inclusion in summaries
granary unpin my-big-project-5h18-task-321
```

## 6.3 Session environment export (for agents/subagents)

```
granary session env
# prints:
# export GRANARY_SESSION=sess-20260111-7f2c
# export GRANARY_WORKSPACE=/repo/path
```

---

# 7) Summary and "context pack" commands

## 7.1 `summary` (human + orchestrator friendly)

Your requirement is spot-on.

```
granary summary
granary summary --format prompt --token-budget 1200
granary session sess-... summary --since checkpoint:s1
```

### Recommended summary structure (stable)

For `--format prompt`, output something like:

- Session header (id, name, mode, focus)
- "State of work" (counts by status/priority)
- Focus task detail (description + latest progress + blockers)
- Next actionable tasks (dependency-aware)
- Open questions / decisions pending
- Recent artifacts/links
- "Do next" suggestions (optional, but deterministic—no LLM required)

**Important:** this should be derived from _state + recent events_, not freeform text. That keeps it consistent and tool-like.

## 7.2 `context` (export exactly what an LLM should see)

```
granary context --format prompt
granary context --format json
granary context --include decisions,blockers,artifacts --max-items 50
```

This is the mechanism that replaces "plan files" as the durable global context.

Kiro emphasizes persistent context via steering/spec artifacts included into the conversation; Granary can do the same, but in a tool-agnostic way.

---

# 8) Start/next/claim: the agentic loop primitives

## 8.1 `start` (your requested command)

Make it an alias to `task … start`, but keep the noun form too.

```
granary start my-big-project-5h18-task-321
# equivalent to:
granary task my-big-project-5h18-task-321 start
```

**What `start` should do atomically:**

- Set `status = in_progress` (unless already terminal)
- Set `started_at` if unset
- Optionally set/refresh a claim lease (`--owner`, `--lease 30m`)
- Append a progress comment: "started by X" (structured, not chatty)

This mirrors the "intelligent task lifecycle + cross-session persistence" ideas that have emerged around Claude Code task tooling.

## 8.2 `next` (critical for orchestration)

A `next` command is one of the highest leverage features for loops:

```
granary next
granary next --json
granary next --include-reason
```

Algorithm (deterministic):

- Consider tasks in current session scope
- Filter to `status in {todo, in_progress?}` and not blocked
- Exclude tasks with incomplete dependencies
- Order by priority (P0 → P4), then due date, then creation time
- Return top task + rationale (e.g., "deps satisfied: …")

This is validated by existing CLI task tooling patterns (e.g., "show the next task based on dependencies and status").

## 8.3 Claiming/leases (multi-agent safety)

```
granary task ... claim --owner "Agent A" --lease 45m
granary task ... heartbeat
granary task ... release
```

This prevents two subagents from duplicating work.

---

# 9) Checkpoints (pause/resume, diffable state)

Agentic loops benefit hugely from being able to snapshot state and later compare.

```
granary checkpoint create "before-refactor"
granary checkpoint list
granary checkpoint diff before-refactor now --format md
granary checkpoint restore before-refactor
```

This is directly analogous to checkpointing/persistent thread state in agent workflow systems.

---

# 10) Handoffs (first-class, not ad hoc)

You want "agent → subagent" to be seamless. Model this explicitly:

```
granary handoff --to "Review Agent" \
  --tasks my-big-project-5h18-task-321,my-big-project-5h18-task-322 \
  --format prompt
```

Output includes:

- Role ("Review Agent")
- Task(s) with context
- Constraints
- Acceptance criteria (if present)
- "Report back" instructions
- Required output schema (e.g., JSON with fields `findings`, `changes_needed`, `risk`)

This aligns with the "handoff" pattern used in multi-agent orchestration guidance (routines + handoffs).

---

# 11) Batch updates: how agents should write to Granary reliably

Agents are much more reliable when they can emit structured deltas.

## 11.1 `apply` (single patch)

```
cat <<'JSON' | granary apply --stdin
{ "ops": [
    {"op":"task.update","id":"my-big-project-5h18-task-321","status":"done"},
    {"op":"comment.create","parent":"my-big-project-5h18-task-321","kind":"progress","content":"Merged PR #123"}
  ]
}
JSON
```

## 11.2 `batch` (JSONL stream)

```
granary batch --stdin < operations.jsonl
```

This is the "glue" that makes agent loops robust: they can compute changes in memory, then commit them atomically.

---

# 12) "Specs" and "Steering" as optional, tool-agnostic analogs to Kiro

Kiro's approach formalizes:

- **Steering** = persistent workspace guidance
- **Specs** = structured artifacts (requirements/design/tasks) with tracking

Granary can support the same concepts without becoming an IDE:

## 12.1 Steering

```
granary steering list
granary steering add .granary/steering/coding-standards.md --mode always
granary steering add docs/architecture.md --mode on-demand
```

Then:

```
granary context --include steering --format prompt
```

## 12.2 Specs (optional)

```
granary specs create "Auth"  # scaffolds requirements.md/design.md/tasks.md
granary spec auth show --format md
granary spec auth import tasks.md  # converts checklist into tasks
granary spec auth sync  # keeps spec tasks.md and granary tasks in sync (optional)
```

This provides a migration path for teams used to spec/plan files.

---

# 13) A concrete end-to-end workflow (agentic loop)

### Planning phase

```
granary session start "auth" --owner "Claude Code" --mode plan
granary projects create "Authentication"
granary session add project authentication-9k2d

# Agent creates tasks:
granary project authentication-9k2d tasks create "Add login endpoint" --priority P0
granary project authentication-9k2d tasks create "Add session cookie" --priority P0 --dependencies authentication-9k2d-task-1
granary project authentication-9k2d tasks create "Write tests" --priority P1 --dependencies authentication-9k2d-task-1
```

### Execution loop (orchestrator)

```
granary session use sess-...
granary next --json  # pick next actionable task
granary start authentication-9k2d-task-1 --owner "Agent A"
granary task authentication-9k2d-task-1 comments create --kind progress --content "Implemented endpoint; tests pending"
granary task authentication-9k2d-task-1 done --comment "Merged PR #123"
granary summary --format prompt  # feed back into orchestrator
```

### Parallelism

- Orchestrator calls `granary next` repeatedly and spawns subagents.
- Each subagent `claim`s tasks with leases to avoid collisions.

### Human-in-the-loop review

- Use checkpoints before risky refactors.
- Use `handoff` to delegate review to a specialized agent.

---

# 14) Command set (proposed final list)

High-frequency, agent-loop critical:

- `session start|use|current|close|add|rm|env`
- `summary`
- `context`
- `next`
- `start` (alias)
- `task done|block|unblock|claim|release`
- `checkpoint create|list|diff|restore`
- `handoff`
- `apply` / `batch`

Core CRUD:

- `projects`, `project`
- `tasks`, `task`
- `comments`, `comment`
- `artifacts`, `artifact` (attach/link)

Ops/integration:

- `init`, `doctor`, `config`
- `import` / `export` (JSON/YAML/Markdown; later: Linear/Jira/GitHub Issues)

---

# 15) Implementation details that matter for agents

- **Deterministic output**: never reorder lists without documented sorting rules.
- **Exit codes**:

  - `0` success
  - `2` user error (bad args)
  - `3` not found
  - `4` conflict (claim/lease, optimistic concurrency)
  - `5` blocked/deps unmet (for `start`/`next` when strict)

- **Optimistic concurrency**: include `version` on tasks; allow `--if-match <version>`.
- **Redaction**: `--redact secrets` (basic patterns) for context exports.
- **Token budgeting** (approximate) for `summary/context` to fit into model context windows.
