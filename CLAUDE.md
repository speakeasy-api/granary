After implementation is complete, use `cargo fmt` to format files.

IMPORTANT: when user requests to "use granary", run `granary` command before performing any other task.

## Iced GUI Development

The `crates/silo/` crate uses the Iced GUI library. When working on files in this crate, the `.claude/skills/iced-development.md` skill is automatically applied. It contains best practices for:

- Application architecture (Elm Architecture / MVU)
- Message enum patterns
- State management
- Styling and theming
- Component composition
- Async operations with Tasks
- Subscriptions for background events
