# Agent Instructions

## SQL Migrations

- Always create migrations via SQLx CLI from the repo root:
  - `sqlx migrate add <name>`
- Do not hand-write timestamp/version prefixes in migration filenames.
- If you hit a duplicate migration version error, regenerate one migration with `sqlx migrate add <name>` and move the SQL into the newly generated file.

## Build Guard

- This repo has a build-time check in `build.rs` that fails compilation when two migration files share the same numeric version prefix.
- Treat that failure as a naming/version collision and regenerate one of the conflicting migrations.
