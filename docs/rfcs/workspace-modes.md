# RFC: Global-First Workspaces

**Status:** Draft
**Author:** Daniel Kovacs
**Created:** 2026-02-08

## Summary

Make granary global-first by default: a single database at `~/.granary/granary.db` serves as the workspace unless the user explicitly opts into local (per-directory) mode. Workspace boundaries are tracked through a lightweight registry that maps directory roots to named workspace databases under `~/.granary/workspaces/`. The existing local `.granary/` directory model is preserved as an opt-in via `granary workspace init --local`.

## Motivation

Today, granary is technically local-first — `granary init` creates a `.granary/` directory in the current folder. However, because `~/.granary/` exists as the global config directory and workspace discovery walks up the tree, users whose cwd is anywhere under `$HOME` already get a working granary without ever running `init`. This is the happy accident we want to formalize.

The current model has friction:

1. **Init ceremony is unnecessary for most users.** A single developer working across multiple repos doesn't want a separate `.granary/granary.db` per project — they want one place for all their tasks and projects.
2. **Nested `.granary/` directories are confusing.** The traversal stops at the first `.granary/` it finds, which means a user who ran `init` in both `~/projects/` and `~/projects/foo/` gets different databases depending on their cwd.
3. **No workspace isolation model.** When users *do* want isolation (e.g., work vs personal), there's no first-class concept of a named workspace — just wherever `.granary/` happens to live on disk.

## Design

### Core Concepts

**Global mode** (default): All data lives under `~/.granary/`. The "workspace" is `$HOME` itself. No `granary init` required — the first command that needs a database creates `~/.granary/granary.db` automatically. This is how most users should interact with granary.

**Workspace mode**: A named, isolated database stored at `~/.granary/workspaces/<name>/granary.db`. Created via `granary workspace init`. The workspace is associated with one or more directory roots via the registry. Directories map to workspaces, not the other way around — each directory can belong to at most one workspace.

**Local mode** (legacy): A `.granary/` directory in the project root, just like today. Created via `granary workspace init --local`. This is the escape hatch for users who want data co-located with their repo (e.g., for version control or offline portability).

### Directory Layout

```
~/.granary/
├── config.toml                    # Global user config (runners, preferences)
├── granary.db                     # Default workspace database
├── workers.db                     # Daemon/worker state (unchanged)
├── workspaces/
│   ├── registry.json              # Workspace registry (see below)
│   ├── work/
│   │   └── granary.db             # "work" workspace database
│   └── personal/
│       └── granary.db             # "personal" workspace database
├── daemon/                        # Daemon files (unchanged)
└── logs/                          # Worker logs (unchanged)
```

For local mode, the existing structure is preserved:

```
/path/to/project/.granary/
├── granary.db
└── session
```

### Workspace Registry

A JSON file at `~/.granary/workspaces/registry.json` maps directory roots to workspace names. Directory paths are the keys, ensuring each directory belongs to at most one workspace:

```json
{
  "roots": {
    "/Users/daniel/work": "work",
    "/Users/daniel/contracts": "work",
    "/Users/daniel/personal": "personal"
  },
  "workspaces": {
    "work": {
      "created_at": "2026-02-08T10:00:00Z"
    },
    "personal": {
      "created_at": "2026-02-08T10:00:00Z"
    }
  }
}
```

The `roots` map provides O(1) lookup from a directory path to its workspace name, and structurally prevents a directory from being assigned to multiple workspaces.

Workspace databases are always at `~/.granary/workspaces/<name>/granary.db` — the name in the `workspaces` map is the directory name under `~/.granary/workspaces/`.

**Resolution order** (most specific wins):

1. `GRANARY_HOME` env var or `--workspace` CLI flag — explicit override, skip all discovery.
2. Local `.granary/` — walk up from cwd, stop before `$HOME`. If found, use it (local mode).
3. Registry lookup — check if cwd (or any ancestor up to but not including `$HOME`) matches a key in `roots`. Most specific (deepest path) match wins.
4. Default — use `~/.granary/granary.db`.

### Commands

`granary workspace init` is the canonical entrypoint for workspace creation. `granary init` is a convenience alias.

#### `granary workspace init`

```
granary workspace init [--local] [--force] [--skip-git-check] [--name <workspace-name>]
```

Creates a named workspace associated with the current directory.

**Validation checks (apply to both `--local` and default global mode):**

1. **Already initialized locally?** Check if `.granary/` exists in the current directory.
   - If yes and global mode: error → `Workspace already initialized locally at ./.granary. Use --force to overwrite, or run 'granary workspace migrate --global' to migrate to a named workspace.`
   - If yes and local mode: error → `Workspace already initialized locally at ./.granary. Use --force to overwrite.`

2. **Parent workspace exists?** Walk up from cwd to (but not including) `$HOME`, looking for `.granary/`.
   - If found: error → `Already inside workspace at /path/to/parent/.granary. Use --force to initialize a nested workspace.`

3. **Git directory check.** Look for `.git/` or `.git` file in cwd.
   - If `.git/` exists in a parent but not in cwd: error → `Not in git repository root (git root is /path/to/root). Use --skip-git-check if this is intentional.`
   - If no `.git/` found anywhere: proceed (not a git project, that's fine).

**Default (global) mode** (`granary workspace init` without `--local`):

After validation passes:
1. Derive workspace name from `--name` flag, or from the directory name (e.g., `/Users/daniel/projects/myapp` → `myapp`). If that name already exists in the registry, append a short random suffix (e.g., `myapp-a3f`).
2. Create `~/.granary/workspaces/<name>/granary.db` and run migrations.
3. Add cwd → name entry to `registry.json` `roots` map, and name → metadata to `workspaces` map.
4. Inject agent instructions (existing behavior).
5. Output: `Initialized workspace "myapp" at ~/.granary/workspaces/myapp/`

**Local mode** (`granary workspace init --local`):

After validation passes:
1. Create `.granary/` in cwd (existing behavior).
2. Create `granary.db` and run migrations.
3. Inject agent instructions.
4. Output: `Initialized local workspace at /path/to/project/.granary/`

The local workspace is NOT registered in the registry — it's discovered by directory traversal, same as today.

#### `granary init` (Alias)

```
granary init [--local] [--force] [--skip-git-check]
```

Alias for `granary workspace init`. When invoked without `--local`, it is equivalent to:

```
granary workspace init --name <generated_name>
```

Where `<generated_name>` is a random identifier like `workspace_1dg29`. This provides a quick "just give me isolation" experience without requiring the user to think of a name. In contrast, `granary workspace init` derives a name from the directory.

When invoked with `--local`, it is equivalent to `granary workspace init --local`.

#### `granary workspace` (No Args)

Shows workspace info for cwd — which workspace the current directory belongs to, the resolved database path, and the workspace mode (default/named/local).

```
$ granary workspace
Workspace: work
Mode:      named
Database:  ~/.granary/workspaces/work/granary.db
Root:      /Users/daniel/work (matched from registry)
```

```
$ granary workspace
Workspace: default
Mode:      default
Database:  ~/.granary/granary.db
```

#### `granary workspace list` / `granary workspaces`

Lists all workspaces from the registry, plus the default workspace, plus any local workspaces if cwd is inside one.

```
$ granary workspaces
NAME       MODE     DATABASE                                    ROOTS
default    default  ~/.granary/granary.db                       (all unmatched)
work       named    ~/.granary/workspaces/work/granary.db       /Users/daniel/work, /Users/daniel/contracts
personal   named    ~/.granary/workspaces/personal/granary.db   /Users/daniel/personal
```

Stale root paths (directories that no longer exist) are shown but not automatically cleaned up. They remain in the registry until explicitly removed — there's no negative consequence of orphaned paths, and keeping them helps debug accidentally deleted workspaces.

#### `granary workspace <name> add`

Adds cwd to the named workspace. Fails if cwd is already registered to any workspace.

```
$ cd ~/contracts
$ granary workspace work add
Added /Users/daniel/contracts to workspace "work".
```

Error if already registered:
```
$ granary workspace work add
Error: /Users/daniel/contracts is already part of workspace "personal". Remove it first with 'granary workspace personal remove'.
```

#### `granary workspace <name> remove`

Removes cwd from the named workspace.

```
$ cd ~/contracts
$ granary workspace work remove
Removed /Users/daniel/contracts from workspace "work".
```

Error if cwd is not a root of the named workspace:
```
Error: /Users/daniel/contracts is not a root of workspace "work".
```

#### `granary workspace <name> move`

Rewrites the workspace directory root for the current workspace. Used *before* moving workspace directories on disk.

```
$ cd ~/work
$ granary workspace work move ~/new-location/work
Updated workspace "work": /Users/daniel/work → /Users/daniel/new-location/work
# Now move the directory:
# mv ~/work ~/new-location/work
```

This updates the `roots` map key from the old path to the new path. It fails if cwd is not a root of the named workspace, or if the target path is already registered.

#### `granary workspace <name> migrate`

Migrates between local and named workspace modes.

```
granary workspace <name> migrate --global [--name <workspace-name>]
granary workspace <name> migrate --local
```

**Local → Global** (`granary workspace <name> migrate --global`):

1. Find local `.granary/granary.db` in cwd.
2. Derive workspace name from `--name` flag, or from the directory name.
3. Copy database to `~/.granary/workspaces/<name>/granary.db`.
4. Add cwd → name entry to `registry.json`.
5. Remove `.granary/` from cwd (with confirmation prompt).
6. Output: `Migrated local workspace to "myapp". Local .granary/ removed.`

**Global → Local** (`granary workspace <name> migrate --local`):

1. Determine which workspace cwd belongs to (from registry).
2. Copy database to `./.granary/granary.db`.
3. Remove cwd from the `roots` map in the registry.
4. If workspace has no remaining roots, optionally clean up the workspace directory under `~/.granary/workspaces/`.
5. Output: `Migrated workspace "myapp" to local .granary/. Removed from registry.`

Both operations copy rather than move, then clean up the source. This is safer than a move — if the process is interrupted, no data is lost.

### Session File Handling

The session pointer file behavior changes slightly per mode:

- **Local mode**: `.granary/session` in the project directory (unchanged).
- **Global/workspace mode**: `~/.granary/workspaces/<name>/session` for named workspaces, `~/.granary/session` for the default workspace.

The `GRANARY_SESSION` env var continues to override in all modes.

### Workspace Struct Changes

```rust
pub struct Workspace {
    /// Workspace name (None for local-only workspaces)
    pub name: Option<String>,
    /// Root directory (cwd for global, project root for local)
    pub root: PathBuf,
    /// Path to the directory containing granary.db
    pub granary_dir: PathBuf,
    /// Path to the database file
    pub db_path: PathBuf,
    /// How this workspace was resolved
    pub mode: WorkspaceMode,
}

pub enum WorkspaceMode {
    /// Default global workspace (~/.granary/granary.db)
    Default,
    /// Named workspace under ~/.granary/workspaces/<name>/
    Named(String),
    /// Local .granary/ directory in the project tree
    Local,
}
```

### Why Database-per-Workspace (Not Workspace-ID Column)

We use separate databases per workspace rather than a single database with a `workspace_id` column. The reasons:

1. **No query tax.** Every query in the codebase today works without a workspace filter. Adding `WHERE workspace_id = ?` to every query across 2,400+ lines of database code is error-prone and a maintenance burden.
2. **Natural isolation.** Deleting a workspace means deleting a file. No surgical `DELETE FROM` across 10+ tables.
3. **No contention.** Multiple concurrent granary processes (one per terminal) can write to different workspace databases without competing for a single write lock.
4. **Simple migration.** Copying a database file between global and local is a file copy. No data extraction/insertion.
5. **Schema stays clean.** The same migrations work identically regardless of mode. No conditional schema.

The tradeoff is that cross-workspace queries (e.g., "all tasks across all workspaces") require opening multiple databases. This is acceptable — Silo (the GUI) can use SQLite's `ATTACH DATABASE` for read-only cross-workspace views, and the CLI rarely needs cross-workspace queries.

### Output

All new workspace commands implement the existing `Output` trait, supporting `--format` / `--json` / `--prompt` / `--text` flags. Each command chooses its own sensible default — `workspace list` defaults to `table`, while single-entity commands like `workspace` (info) default to `md` for readable key-value output. `prompt` and `json` modes are available for agent and programmatic consumption respectively.

### Impact on Existing Code

**Workspace::find()** — Updated to implement the new resolution order. The core change is that it now falls through to the default global database instead of returning `WorkspaceNotFound`. The first command that ever runs auto-creates `~/.granary/granary.db`.

**Workspace::find_or_create()** — Simplified: `find()` now always succeeds (falling through to default), so `find_or_create` is only needed for explicit `init`.

**global_config_service** — The `global_pool()` singleton for `workers.db` is unchanged. The default workspace database (`~/.granary/granary.db`) is a separate pool.

**Migrations** — The same `migrations/` directory is used for all workspace databases. The `workers.db` global database continues to use the same migrations (since it also needs the same schema for event consumers). If schema divergence is needed in the future, separate migration directories can be introduced.

**Agent file injection** — `granary init` continues to inject instructions into workspace agent files. For global mode, it injects into files found at the workspace root (cwd during init).

**`granary doctor`** — Updated to display workspace mode info: whether the current workspace is default/named/local, the resolved database path, and the workspace name if applicable.

### Backward Compatibility

- Existing local `.granary/` directories continue to work. The traversal logic finds them before falling through to global, so behavior is unchanged for users who have already run `granary init`.
- The default workspace (`~/.granary/granary.db`) is the same database that "accidentally" works today via traversal to `~/.granary/`. Data is preserved.
- `GRANARY_HOME` env var and `--workspace` CLI flag continue to work as explicit overrides.
- `granary init` continues to work as an alias, so existing documentation and muscle memory are preserved.

## Alternatives Considered

### UUID-based Workspace Identity

Generate a UUID at init time and store it in each workspace database. A global registry maps UUIDs to paths. This handles directory renames gracefully — the UUID survives a move.

**Rejected because:** Granary workspaces are anchored to filesystem locations. If you move `~/projects/myapp` to `~/archived/myapp`, the workspace *should* update its association. A path-based registry makes this explicit and human-readable. UUIDs add indirection without clear benefit for a CLI tool.

### Single Database with Workspace Column

One `~/.granary/granary.db` containing all data, with a `workspace_id TEXT` column on every table.

**Rejected because:** Every query needs a workspace filter. The existing 2,400+ lines of SQL queries would all need modification. A missing `WHERE` clause leaks data across workspaces. Single point of failure. Write contention across workspaces. See "Why Database-per-Workspace" above.

### Registry in SQLite Instead of JSON

Store the workspace registry in a SQLite database instead of a JSON file.

**Rejected because:** The registry is small (handful of entries), read-heavy, and human-editable. JSON is simpler, debuggable with a text editor, and doesn't require a database connection. If the registry grows to need indexing or transactions, it can be migrated to SQLite later.

### No Registry (Purely Convention-Based)

Use directory name as workspace name with no registry. `~/.granary/workspaces/myapp/granary.db` is the workspace for any directory named `myapp`.

**Rejected because:** Directory names aren't unique. Two projects named `myapp` in different locations would collide. The registry maps specific directory roots to workspaces, avoiding ambiguity.

