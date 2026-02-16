# RFC: FTS5 Full-Text Search

## Problem

Search uses `LIKE '%query%'` on a single column per entity type (project
`name`, task `title`, initiative `name`). This means:

- Descriptions, tags, and comments are never searched.
- No relevance ranking - results come back in `created_at DESC` order.
- `LIKE '%…%'` can't use indexes; it's a full table scan every time.
- Multi-word queries don't work intuitively ("auth login" matches the literal
  substring, not documents containing both words).

`granary plan` inherits these limits via `find_prior_art`, so prior-art
discovery is weaker than it should be.

## Design

### FTS5 Virtual Table

One shared FTS5 table indexes all searchable entities:

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
    entity_type,    -- 'project', 'task', 'initiative'
    entity_id,      -- ID in the source table (logical ref, not a real FK)
    title,          -- name/title (high weight)
    body,           -- description + other text (normal weight)
    tags,           -- tags JSON flattened to space-separated tokens
    tokenize='unicode61 remove_diacritics 2'
);
```

This is a regular (not contentless) FTS5 table. We considered `content=''`
to avoid duplicating storage, but contentless FTS5 tables cannot return
**any** column values on SELECT - including `entity_type` and `entity_id`,
which we need to hydrate results from source tables. External content mode
(`content='table'`) only supports a single backing table, so it doesn't
work for a unified index across three entity types. The storage overhead is
negligible since the indexed text already lives in the inverted index.

### What Gets Indexed

| Entity     | `title` column | `body` column | `tags` column |
| ---------- | -------------- | ------------- | ------------- |
| project    | `name`         | `description` | `tags` JSON   |
| task       | `title`        | `description` | `tags` JSON   |
| initiative | `name`         | `description` | `tags` JSON   |

### Column Weights

FTS5's `bm25()` ranking function accepts per-column weights. `title` gets a
higher weight so name/title matches outrank body matches:

```sql
SELECT entity_type, entity_id, rank
FROM search_index
WHERE search_index MATCH ?
ORDER BY bm25(search_index, 0.0, 0.0, 10.0, 1.0, 2.0)
```

Weights: `entity_type=0, entity_id=0, title=10, body=1, tags=2`.

### Keeping the Index in Sync

Use SQLite triggers on `INSERT`, `UPDATE`, `DELETE` for each source table.
Updates delete the old row and insert the new one; deletes remove by
`entity_type` + `entity_id`:

```sql
-- Example for projects
CREATE TRIGGER IF NOT EXISTS search_index_projects_insert AFTER INSERT ON projects BEGIN
    INSERT INTO search_index(entity_type, entity_id, title, body, tags)
    VALUES ('project', NEW.id, NEW.name, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS search_index_projects_update AFTER UPDATE ON projects BEGIN
    DELETE FROM search_index WHERE entity_type = 'project' AND entity_id = OLD.id;
    INSERT INTO search_index(entity_type, entity_id, title, body, tags)
    VALUES ('project', NEW.id, NEW.name, NEW.description, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS search_index_projects_delete AFTER DELETE ON projects BEGIN
    DELETE FROM search_index WHERE entity_type = 'project' AND entity_id = OLD.id;
END;
```

Same pattern for `tasks` and `initiatives`.

### Migration

A single new migration file:

1. Create the `search_index` FTS5 virtual table.
2. Create the six triggers (insert/update/delete x 3 entity types).
3. Backfill existing rows:
   ```sql
   INSERT INTO search_index(entity_type, entity_id, title, body, tags)
   SELECT 'project', id, name, description, tags FROM projects;
   -- ... same for tasks and initiatives
   ```

### Rust Changes

**`db::search`** - Replace the three `LIKE`-based functions with a single
`search_all` function:

```rust
pub async fn search_all(pool: &SqlitePool, query: &str, limit: i32) -> Result<Vec<FtsMatch>> {
    let rows = sqlx::query_as::<_, FtsMatch>(
        r#"
        SELECT entity_type, entity_id, rank
        FROM search_index
        WHERE search_index MATCH ?
        ORDER BY bm25(search_index, 0.0, 0.0, 10.0, 1.0, 2.0)
        LIMIT ?
        "#,
    )
    .bind(query)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

The caller (`search_service.rs`) takes the `(entity_type, entity_id)` pairs,
batch-fetches full rows from source tables, and assembles `SearchResult`
variants as before.

Keep the existing per-type functions as thin wrappers that filter by
`entity_type` (the `plan` command only wants projects).

**`services::search_service`** - Update to call `search_all`, then hydrate
results by fetching full rows. Group entity IDs by type, batch-fetch, merge.

**`cli::plan::find_prior_art`** - Switches to `db::search::search_projects`
which now uses FTS5 internally. No signature change needed.

### Query Syntax

FTS5 gives us these for free, no extra code:

- `auth login` - implicit AND (matches rows containing both terms)
- `"auth login"` - phrase match
- `auth OR login` - explicit OR
- `auth*` - prefix match

Expose these as-is. The existing single-string query parameter maps directly
to an FTS5 MATCH expression.

### `--since` flag on `granary search`

No change. FTS5 results join back to source tables, so any post-filter
(status, date) is applied on the hydrated rows.

## Rollout

One migration, one PR. Existing `granary search` and `granary plan` calls
get better results with no CLI changes. The only user-visible difference is
that search now considers descriptions/tags and returns relevance-ranked
results.
