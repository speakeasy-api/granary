# RFC 0001: Automated Releases with Boop

## Status

Draft

## Problem

Our current release process is manual and error-prone:

1. **Forgotten tags** — A change can be merged to `main` without a corresponding tag push, so no release is created.
2. **Version drift** — `Cargo.toml` version can fall out of sync with the GitHub release tag, causing the CLI update checker to permanently show "new version available" even when the user is up to date.
3. **Manual release notes** — Writing release notes by hand is tedious and easy to forget, leading to empty or auto-generated notes that don't describe what actually changed.

All three issues stem from the same root cause: humans are in the loop for steps that should be automated.

## Proposal

Replace the tag-push-driven release workflow with an automated pipeline powered by [boop](https://github.com/danielkov/boop).

### How boop works

Boop stores changelog fragments as individual files in `.boop/changelogs/`. Each fragment is created with a CLI command:

```sh
boop major "description"   # breaking change
boop minor "description"   # new feature
boop patch "description"   # bug fix
```

At release time, `boop apply` resolves the next semver version from the highest pending bump and records the release in `.boop/releases.toml`. It does **not** modify project manifests — updating `Cargo.toml` is the workflow's responsibility using boop's `version` output. After apply, `boop version` returns the new version and `boop changelog` returns the assembled release notes (grouped by severity: major > minor > patch).

Because fragments are individual files with ULID names, parallel branches never conflict.

### New workflow

```
Developer                           CI
─────────                           ──
1. Make changes on feature branch
2. Run `/shipit` (or do it manually)
   - Determines semver bump type
   - Runs `boop <major|minor|patch>`
   - Commits with conventional commit
   - Pushes & opens PR
3.                                  CI runs tests (existing workflow)
4. Merge PR to main
5.                                  CI: boop apply → get version &
                                        changelog → sed Cargo.toml →
                                        cargo update → commit → tag →
                                        build matrix → GH release
```

### CI changes

We adopt the same pattern boop itself uses for its own releases — a single workflow on `main` that checks for pending fragments, applies them, updates the manifest, commits, tags, builds, and publishes. See [boop's release workflow](https://github.com/danielkov/boop/blob/main/.github/workflows/release.yml) as the reference implementation to adapt from.

#### Updated workflow: `release.yml`

Replaces the current tag-triggered workflow. Now triggered on push to `main`.

**Job 1: Check for release**

1. Checkout
2. Run the boop action (`danielkov/boop@v1`) — this installs boop and runs `boop apply`. Outputs: `released` (bool), `version`, `changelog`
3. If `released == 'true'`:
   - Install Rust toolchain
   - Update `Cargo.toml` workspace version via `sed` and run `cargo update --workspace` to sync `Cargo.lock`
   - Configure git as `github-actions[bot]`
   - Commit `.boop/releases.toml`, `Cargo.toml`, `Cargo.lock` with a `ci(boop): release <version> [skip ci]` message containing the changelog
   - Tag `v<version>` and push to `main` with tags

**Job 2: Build** (unchanged — same cross-platform matrix)

Runs if `released == 'true'`. Checks out at the new `v<version>` tag to ensure the build uses the updated `Cargo.toml` version. Same matrix as today: macOS (x86_64 + aarch64), Linux (x86_64 + aarch64 via cross), Windows (x86_64). Includes the Silo.app bundle step for macOS.

**Job 3: Create GitHub Release**

Uses `softprops/action-gh-release@v2` with:

- `tag_name` and `name` from the boop version output
- `body` from the boop changelog output (replaces `generate_release_notes: true`)
- `prerelease` based on whether the version contains a hyphen (pre-release indicator)

#### `ci.yml` — No changes needed.

### Local developer experience: `/shipit` command

A new Claude Code skill that automates the "create PR" side:

1. Analyze staged/unstaged changes and recent commits on the branch
2. Determine the appropriate semver bump (major/minor/patch)
3. Generate release notes and run `boop <bump> "<notes>"`
4. Verify branch state — if on an unexpected branch, prompt the user
5. Commit using conventional commit format (e.g., `feat: add X`, `fix: Y`)
6. Push and open a PR with the release notes as the description

### Responsibility split

| Concern                    | Who                                   | How                                                       |
| -------------------------- | ------------------------------------- | --------------------------------------------------------- |
| Record changelog fragments | Developer (via `/shipit` or manually) | `boop major\|minor\|patch "..."`                          |
| Resolve next version       | CI (boop action)                      | `boop apply` → outputs `version` and `changelog`          |
| Update `Cargo.toml`        | CI (workflow step)                    | `sed` + `cargo update --workspace`                        |
| Commit version bump        | CI (workflow step)                    | `git add` + `git commit` + `git tag` + `git push`         |
| Build release binaries     | CI (existing matrix)                  | `cargo build --release --target ...`                      |
| Publish GitHub Release     | CI (workflow step)                    | `softprops/action-gh-release` with boop changelog as body |

### What gets committed to the repo

- `.boop/` directory (created by `boop init`)
  - `releases.toml` — version ledger (updated by CI on release)
  - `changelogs/` — fragment files added by developers, consumed by `boop apply`
- `Cargo.toml` / `Cargo.lock` — version updated by CI on `main`, **not** by developers

### Migration

1. Run `boop init` in the repo root — it auto-detects version `1.2.0` from `Cargo.toml`
2. Add `.boop/` to version control
3. Add `auto-release.yml` workflow
4. Update `release.yml` to use boop changelog
5. Update the `finalize` skill to use `/shipit` instead of manual version bumping
6. Remove any manual version-bump steps from contributor docs

## Risks

- **CI needs write access to push commits/tags** — Already the case with our current setup (release workflow has `contents: write`).
- **Merge conflicts in `.boop/`** — By design, boop uses ULID-named files so parallel branches don't conflict. `releases.toml` only changes on `main`.
