### FTS5 search no longer fails on special characters

Queries containing FTS5 operator characters like `-`, `+`, `*`, or keywords like `NOT`/`OR`/`AND` are now properly escaped before being passed to SQLite's FTS5 `MATCH` clause.

Previously, running something like `granary plan "Fix TypeScript v2 build failures - 11 root causes from SDK battery tests"` would fail because the `-` was interpreted as FTS5's NOT operator. Now, each token is individually quoted and purely-punctuation tokens (like a bare `-`) are dropped, so the search works regardless of what characters appear in the plan name.

Search queries also now use `OR` semantics instead of implicit `AND`, which makes prior-art matching more lenient — a project only needs to match *some* of the query terms to appear in results, rather than requiring all of them.