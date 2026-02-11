# RFC: CLI UX Aliases & Forgiving Argument Resolution

## Problem

The current CLI is precise but unforgiving. A user who types `granary project` gets a different experience than `granary projects`. Someone who types `granary tasks create` gets an error because the create action lives under `project <id> tasks create`. A user who guesses `granary project show <id>` instead of `granary show <id>` gets nothing.

This is fine for power users and LLM agents who read `--help`. It's a brick wall for everyone else.

## Design Principles

1. **Never break existing commands.** Every current invocation continues to work identically.
2. **Guess what the user meant.** If there's an unambiguous interpretation, just do it.
3. **Aliases must be discoverable.** Running `--help` should show all accepted forms. No invisible magic.

---

## Strategy: Clap-Native Over Pre-Parse

Clap supports [`visible_alias`](https://docs.rs/clap/latest/clap/struct.Command.html) and `visible_aliases` on both subcommands and args. When a subcommand has visible aliases, `--help` renders them inline:

```
Commands:
  create [aliases: new, add]  Create a new project
  update [aliases: edit]      Update project
```

This means aliases are self-documenting. Users who run `--help` learn that `new` works. Users who guess `new` without reading help get the right behavior. Both audiences are served.

The entire RFC is implemented through clap derive attributes — no pre-parse argument rewriting, no shadow layer the user can't see.

---

## 1. Singular/Plural Unification

### Current behavior

| Command            | Result                 |
| ------------------ | ---------------------- |
| `granary projects` | Lists projects         |
| `granary project`  | Error: requires `<id>` |
| `granary tasks`    | Lists tasks            |
| `granary task`     | Error: requires `<id>` |

### Proposed behavior

| Command            | Result                                  |
| ------------------ | --------------------------------------- |
| `granary projects` | Lists projects                          |
| `granary project`  | **Lists projects** (same as `projects`) |
| `granary tasks`    | Lists tasks                             |
| `granary task`     | **Lists tasks** (same as `tasks`)       |

### Affected pairs

| Singular (currently requires ID) | Plural (currently lists) |
| -------------------------------- | ------------------------ |
| `project`                        | `projects`               |
| `task`                           | `tasks`                  |
| `initiative`                     | `initiatives`            |
| `session`                        | `sessions`               |
| `worker`                         | `workers`                |
| `run`                            | `runs`                   |

`workspace`/`workspaces` already works this way.

### Implementation: Merge singular and plural into one variant

Eliminate the separate plural variants. Each entity gets a single command with `id: Option<String>`. When `id` is `None`, it lists. When `id` is `Some`, it shows/manages the specific entity.

The plural form becomes a `visible_alias`:

```rust
/// Manage projects
#[command(visible_alias = "projects")]
Project {
    /// Project ID (omit to list all)
    id: Option<String>,

    #[command(subcommand)]
    action: Option<ProjectAction>,

    /// Include archived (for list)
    #[arg(long)]
    all: bool,
},
```

Help output becomes:

```
Commands:
  project [aliases: projects]  Manage projects
  task [aliases: tasks]        Manage tasks
  ...
```

And these all work:

```bash
granary project                     # list projects
granary projects                    # list projects (alias)
granary project my-id               # show project
granary project create "Auth"       # create project
granary projects create "Auth"      # same (alias)
granary project my-id tasks         # list project tasks
```

### Merging the action enums

Currently `ProjectsAction` has only `Create`, while `ProjectAction` has `Update`, `Archive`, `Tasks`, `Deps`, `Summary`, `Ready`, `Steer`. After merging, `ProjectAction` absorbs `Create`:

```rust
#[derive(Subcommand)]
pub enum ProjectAction {
    /// Create a new project
    #[command(visible_aliases = ["new", "add"])]
    Create {
        name: Option<String>,
        #[arg(long = "name", conflicts_with = "name")]
        name_flag: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        tags: Option<String>,
    },

    /// Update project
    #[command(visible_aliases = ["edit", "modify"])]
    Update { /* ... */ },

    /// Archive project
    #[command(visible_alias = "close")]
    Archive,

    // ... rest unchanged
}
```

### Dispatch logic

The handler checks `id` to decide list vs. detail:

```rust
Commands::Project { id: None, action: None, all } => {
    // List projects
    projects::list(all, cli_format, watch, interval).await?;
}
Commands::Project { id: None, action: Some(ProjectAction::Create { .. }), .. } => {
    // Create project (no ID needed)
    projects::create(/* ... */).await?;
}
Commands::Project { id: Some(id), action, .. } => {
    // Show or manage specific project
    projects::project(&id, action, cli_format).await?;
}
```

### The "create" ambiguity

`granary project create "Auth"` — is `create` the project ID or the subcommand?

Clap handles this: `#[command(subcommand)]` takes priority over positional args when the token matches a subcommand name (or alias). So `create` is unambiguously the subcommand. A project whose ID is literally `create` can be accessed via `granary show create`.

In practice this never happens — granary IDs are generated as `<slug>-<4chars>` (e.g., `my-project-abc1`).

### Same pattern for all entity pairs

Apply identically to `Task`/`Tasks`, `Initiative`/`Initiatives`, `Session`/`Sessions`, `Worker`/`Workers`, `Run`/`Runs`.

For `Task`, `id: Option<String>` + `action: Option<TaskAction>`. When `id` is `None` and no action, list tasks. The `Tasks` top-level variant is removed and `task` gets `#[command(visible_alias = "tasks")]`.

---

## 2. Action Aliases

Users will guess action verbs. Every action subcommand gets `visible_aliases` so they show up in `--help`.

### Alias table

| Canonical | Visible Aliases           | Scope                                                         |
| --------- | ------------------------- | ------------------------------------------------------------- |
| `create`  | `new`, `add`              | Projects, tasks, initiatives, sessions, comments, checkpoints |
| `update`  | `edit`, `modify`          | Projects, tasks, initiatives                                  |
| `archive` | `close`                   | Projects, initiatives                                         |
| `start`   | `begin`                   | Tasks, workers, sessions                                      |
| `stop`    | `halt`, `kill`            | Workers, runs                                                 |
| `block`   | `hold`                    | Tasks                                                         |
| `unblock` | `unhold`                  | Tasks                                                         |
| `release` | `drop`, `unclaim`         | Tasks                                                         |
| `graph`   | `tree`                    | Dependencies                                                  |
| `list`    | `ls`                      | Steering, deps, checkpoints, etc.                             |
| `rm`      | `remove`, `del`, `delete` | Steering, deps, artifacts                                     |

### Aliases intentionally NOT added

These cause collisions with existing subcommand names in certain contexts:

| Rejected alias       | Reason                                                        |
| -------------------- | ------------------------------------------------------------- |
| `status` → `summary` | Conflicts with `worker status`, `run status`, `daemon status` |
| `run` → `start`      | Conflicts with `run` entity noun                              |
| `done` → `archive`   | Conflicts with `task done`                                    |
| `pause` → `block`    | Conflicts with `run pause`                                    |
| `resume` → `unblock` | Conflicts with `run resume`                                   |

### Implementation

Pure clap derive attributes. Example for `ProjectAction`:

```rust
#[derive(Subcommand)]
pub enum ProjectAction {
    /// Create a new project
    #[command(visible_aliases = ["new", "add"])]
    Create { /* ... */ },

    /// Update project
    #[command(visible_aliases = ["edit", "modify"])]
    Update { /* ... */ },

    /// Archive project
    #[command(visible_alias = "close")]
    Archive,

    /// Show project summary
    #[command(visible_alias = "overview")]
    Summary,

    // ...
}
```

Example for `TaskAction`:

```rust
#[derive(Subcommand)]
pub enum TaskAction {
    /// Start working on task
    #[command(visible_alias = "begin")]
    Start { /* ... */ },

    /// Block task
    #[command(visible_alias = "hold")]
    Block { /* ... */ },

    /// Unblock task
    #[command(visible_alias = "unhold")]
    Unblock,

    /// Release claim on task
    #[command(visible_aliases = ["drop", "unclaim"])]
    Release,

    // ...
}
```

What `granary project my-id --help` looks like after this:

```
Commands:
  create [aliases: new, add]        Create a new project
  update [aliases: edit, modify]    Update project
  archive [aliases: close]          Archive project
  tasks                             List or create tasks
  deps                              Manage dependencies
  summary [aliases: overview]       Show project summary
  ready                             Mark project as ready
  steer                             Manage steering files
  help                              Print help
```

---

## 3. Top-Level Command Aliases

### `show` aliases

The existing `Show` command gets visible aliases:

```rust
/// Show any entity by ID
#[command(visible_aliases = ["view", "get", "inspect"])]
Show {
    id: String,
},
```

Help output:

```
Commands:
  show [aliases: view, get, inspect]  Show any entity by ID
  ...
```

Now `granary get proj-123`, `granary view proj-123`, `granary inspect proj-123` all work, and `granary --help` tells you so.

### `search` alias

```rust
/// Search projects and tasks by title
#[command(visible_alias = "find")]
Search {
    query: String,
},
```

### `start` alias

Already exists as top-level. Add visible alias:

```rust
/// Start a task
#[command(visible_alias = "begin")]
Start {
    task_id: String,
    // ...
},
```

---

## 4. Positional-to-Flag Coercion

### Problem

```
granary project create "My Project"        # positional
granary project create --name "My Project" # flag
```

Both should work. Users shouldn't have to remember which form is required.

### Current state audit

| Command                               | Positional           | Has `--flag` equivalent? |
| ------------------------------------- | -------------------- | ------------------------ |
| `project create <name>`               | `name`               | No                       |
| `project <id> tasks create <title>`   | `title`              | No                       |
| `task <id> comments create <content>` | `content_positional` | **Yes** (`--content`)    |
| `plan <name>`                         | `name`               | No                       |
| `initiate <name>`                     | `name`               | No                       |
| `work done <task_id> <summary>`       | `summary`            | No                       |
| `work block <task_id> <reason>`       | `reason`             | No                       |
| `session start <name>`                | `name`               | No                       |
| `checkpoint create <name>`            | `name`               | No                       |

### Implementation

Follow the existing `CommentAction::Create` pattern — it already solves this:

```rust
Create {
    /// Project name
    name_positional: Option<String>,

    /// Project name (alternative to positional)
    #[arg(long = "name", conflicts_with = "name_positional")]
    name_flag: Option<String>,

    // ... other fields
}
```

Handler:

```rust
let name = name_positional
    .or(name_flag)
    .ok_or_else(|| GranaryError::InvalidArgument("name is required".into()))?;
```

### Priority targets

Phase into creation commands first — that's where users trip up most:

1. `project create <name>` → also `--name`
2. `project <id> tasks create <title>` → also `--title`
3. `plan <name>` → also `--name`
4. `initiate <name>` → also `--name`
5. `session start <name>` → also `--name`
6. `checkpoint create <name>` → also `--name`
7. `work done <task_id> <summary>` → also `--summary`
8. `work block <task_id> <reason>` → also `--reason`

Skip `show <id>`, `search <query>`, `start <task_id>` — the positional is obvious enough that a `--id`/`--query` flag adds noise.

---

## 5. Ambiguity & Edge Cases

### Subcommand vs. ID precedence

When a token after `project` matches both a subcommand name and could be an ID:

```
granary project create "Auth"   # "create" = subcommand (clap picks this)
granary project my-proj-abc1    # "my-proj-abc1" = ID (no subcommand match)
```

Clap resolves this naturally: subcommands match first. Since granary IDs are machine-generated (`<slug>-<4chars>`), they'll never collide with action names like `create`, `update`, `new`, `edit`.

### Hidden aliases for typos (optional, low priority)

Clap also supports non-visible `alias` for common typos without cluttering help:

```rust
#[command(
    visible_aliases = ["new", "add"],
    alias = "crate"  // common typo, hidden
)]
Create { /* ... */ }
```

### `--help` on the merged entity command

`granary project --help` shows:

```
Manage projects

Usage: granary project [ID] [COMMAND]

Commands:
  create [aliases: new, add]        Create a new project
  update [aliases: edit, modify]    Update project
  archive [aliases: close]          Archive project
  tasks                             List or create tasks in project
  deps                              Manage project dependencies
  summary [aliases: overview]       Show project summary
  ready                             Mark project as ready for work
  steer                             Manage project steering files
  help                              Print this message

Arguments:
  [ID]  Project ID (omit to list all)

Options:
      --all   Include archived projects
  -h, --help  Print help
```

Everything is visible. No surprises.

---

## 6. Rollout

### Phase 1: Action aliases (low risk, high value)

Add `visible_alias`/`visible_aliases` to all action subcommands across every enum. This is purely additive — zero behavior change for existing commands, just new accepted synonyms.

Affected enums:

- `ProjectAction` — create, update, archive, summary
- `TaskAction` — start, block, unblock, release
- `WorkCommand` — start, done, block, release
- `WorkerCommand` — start, stop
- `RunCommand` — stop
- `ProjectDepsAction` — list, rm, graph
- `ProjectSteerAction` — list, add, rm
- `SteeringAction` — list, add, rm
- `ArtifactAction` — add, rm
- `DepsAction` — add, rm, graph
- `SubtaskAction` — create
- `CommentAction` — create
- `CheckpointAction` — create, list
- `InitiativeAction` — update, archive, summary
- `InitiativesAction` — create
- `SessionAction` — start, close, rm

Plus top-level: `Show`, `Search`, `Start`.

### Phase 2: Singular/plural merge (medium risk, needs careful dispatch refactor)

Merge `Projects`/`Project`, `Tasks`/`Task`, `Initiatives`/`Initiative`, `Sessions`/`Session`, `Workers`/`Worker`, `Runs`/`Run` into single variants with `id: Option<String>`.

This touches dispatch in `main.rs` and requires adjusting every handler to handle the `id: None` (list) case. But the logic already exists in the plural handlers — it's just moving it.

Potential risk: `#[command(subcommand_negates_reqs = true)]` may be needed on the merged variant so that `granary project create "Auth"` doesn't require `id`. Verify clap behavior here.

### Phase 3: Positional-to-flag coercion (low risk)

Add `--name`/`--title`/`--summary`/`--reason` flag alternatives. Purely additive clap changes.

---

## 7. What This Does NOT Change

- Output formats (table/json/prompt/yaml/md)
- The `work` command workflow (start/done/block/release)
- Error messages for actually invalid input (clap still handles these)
- Programmatic/LLM usage (exact commands continue working)

## 8. Open Questions

1. **How verbose should alias lists get in `--help`?** If a subcommand has 3+ aliases, the `[aliases: ...]` suffix can get wide. Clap renders these inline. We could limit visible aliases to 2-3 most intuitive ones and use hidden `alias` for the rest.

2. **Should `granary task` (no args) show session-scoped or all tasks?** Current `granary tasks` shows session-scoped by default. Merged form should match.

3. **Should we add `delete` as an action?** Currently no entity supports destructive delete through the CLI (only `archive`). If we add `rm`/`remove` aliases, should they map to `archive` or to actual deletion? Recommendation: alias `rm` → `archive` for projects/initiatives, keep it as real `rm` only where it already exists (steering files, deps, artifacts).
