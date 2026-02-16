### Automated release pipeline

Releases are now fully automated using [boop](https://github.com/danielkov/boop). When a PR with changelog fragments merges to `main`, CI automatically:

- Resolves the next semver version from pending fragments
- Updates `Cargo.toml` and `Cargo.lock`
- Commits, tags, and pushes the release
- Builds cross-platform binaries (macOS, Linux, Windows)
- Publishes a GitHub Release with structured release notes

This replaces the previous tag-push workflow, which required manual version bumping and tag creation — a process that was error-prone and led to version drift between `Cargo.toml` and GitHub releases.

### New `/shipit` developer command

Contributors can now run `/shipit` in Claude Code to prepare a release PR. It analyzes your branch's changes, determines the appropriate semver bump, records a changelog fragment via boop, and opens a PR — all in one step.

### Updated finalize workflow

The finalize skill now delegates to `/shipit` instead of requiring manual version bumps in `Cargo.toml`. Version management is fully handled by CI going forward.