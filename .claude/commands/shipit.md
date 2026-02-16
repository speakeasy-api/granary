---
name: shipit
description: Analyze changes, determine semver bump, write release notes with boop, commit, push, and open a PR
user_invocable: true
---

# Ship It

Automate the release preparation workflow: analyze changes, determine the semver bump, record release notes via boop, commit, push, and open a PR.

## Prerequisites

Check that `boop` is installed:

```sh
which boop
```

If boop is not found, prompt the user they need to install it. Suggest running the install for them:

MacOS / Linux:

```sh
curl -sSfL https://raw.githubusercontent.com/danielkov/boop/main/scripts/install.sh | sh
```

Windows:

```powershell
irm https://raw.githubusercontent.com/danielkov/boop/main/scripts/install.ps1 | iex
```

## Steps

### 1. Analyze changes

Examine the current branch's changes relative to `main`:

- Run `git diff main...HEAD --stat` to see changed files
- Run `git log main..HEAD --oneline` to see commits
- Read the changed files to understand what was modified

### 2. Determine semver bump

Based on the changes, classify the release type:

| Type      | Criteria                                                                     | Examples                                               |
| --------- | ---------------------------------------------------------------------------- | ------------------------------------------------------ |
| **major** | Breaking changes to CLI flags, config format, public API, or data migrations | Renamed commands, removed flags, changed config schema |
| **minor** | New features, new commands, new flags, backwards-compatible additions        | Added `--since` flag, new `actions` system             |
| **patch** | Bug fixes, performance improvements, internal refactors, doc updates         | Fixed clippy warnings, dependency bumps                |

If ambiguous, present your assessment to the user and confirm they agree with the bump type before proceeding.

### 3. Write release notes

Write **real release notes** — not a compressed summary or commit message restatement. These end up in the GitHub Release and are what users and contributors actually read.

Structure them with markdown headings and bullet points. For each significant change:

- Give it a descriptive `###` heading
- Explain what changed and **why it matters**
- Call out behavior changes, new workflows, or migration steps
- Use enough detail that someone who wasn't involved understands the change

**Example of good release notes:**

```markdown
### Automated release pipeline

Releases are now fully automated using [boop](https://github.com/danielkov/boop). When a PR with changelog fragments merges to `main`, CI automatically resolves the next semver version, updates `Cargo.toml`, tags the release, builds binaries, and publishes a GitHub Release with structured notes.

This replaces the previous tag-push workflow, which required manual version bumping and tag creation.

### New `/shipit` developer command

Contributors can now run `/shipit` in Claude Code to prepare a release PR. It analyzes your branch's changes, determines the appropriate semver bump, records a changelog fragment, and opens a PR.
```

**Example of bad release notes:**

```markdown
Automated the release pipeline using boop. Added /shipit command for streamlined release preparation.
```

Record them with boop:

```sh
boop <major|minor|patch> "<release notes in markdown>"
```

### 4. Verify branch state

Check the current branch:

```sh
git branch --show-current
```

- If on `main` or `master`: **stop** and ask the user to create a feature branch first.
- If the branch name doesn't seem to match the changes (e.g., branch is `fix/typo` but changes are a new feature): prompt the user with an AskUserQuestion — ask if they want to continue on this branch or create a new one.
- Otherwise: proceed.

### 5. Commit

Stage all changes including the new `.boop/changelogs/` entry. Write a conventional commit message:

- `feat: <description>` for minor bumps
- `fix: <description>` for patch bumps
- `feat!: <description>` or `BREAKING CHANGE: <description>` for major bumps

The commit message should be a concise summary of the changes (not the full release notes).

### 6. Push and open PR

```sh
git push -u origin <branch-name>
```

Open a PR targeting `main`. Use the release notes as the PR body:

```sh
gh pr create --title "<conventional commit style title>" --body "<release notes>"
```
