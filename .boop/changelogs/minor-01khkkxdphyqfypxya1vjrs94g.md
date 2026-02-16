### FTS5 full-text search

Search now uses SQLite's FTS5 full-text search engine instead of `LIKE '%query%'` pattern matching. This is a significant upgrade to how `granary search` and `granary plan` find relevant results.

#### What changed

A new shared FTS5 virtual table (`search_index`) indexes projects, tasks, and initiatives across their names/titles, descriptions, and tags. SQLite triggers keep the index in sync automatically on every insert, update, and delete.

Search results are now **relevance-ranked** using BM25 scoring, with title matches weighted higher than body or tag matches. Previously, results were returned in `created_at DESC` order with no relevance signal.

#### New capabilities

- **Multi-word queries**: `auth login` matches documents containing both terms (implicit AND), instead of requiring the exact substring
- **Phrase matching**: `"auth login"` finds the exact phrase
- **Prefix matching**: `auth*` matches any term starting with "auth"
- **Boolean operators**: `auth OR login` for explicit OR queries
- **Broader coverage**: descriptions and tags are now searched, not just names/titles

#### Impact on `granary plan`

Prior art discovery in `granary plan` benefits directly — it now finds related projects even when the name doesn't contain the exact query substring, and results are ordered by relevance rather than creation date.

Includes an RFC document at `docs/rfcs/fts5-search.md` with full design rationale.