---
name: granary-setup
description: Set up granary in a new project. Use when asked to initialize granary, install it, or get started with task management.
---

# Setting Up Granary

Use this skill when the user asks you to set up granary in their project.

## 1. Check Installation

First, verify granary is installed:

```bash
which granary
```

If not installed, direct the user to install it:

```bash
# macOS/Linux
curl -fsSL https://raw.githubusercontent.com/danielkov/granary/main/install.sh | sh

# Windows PowerShell
irm https://raw.githubusercontent.com/danielkov/granary/main/install.ps1 | iex
```

## 2. Initialize Workspace

Initialize granary in the project directory:

```bash
granary init
```

This creates a `.granary/` directory with the SQLite database.

## 3. Verify Setup

Check that granary is working:

```bash
granary doctor
```

## Done

Granary is now ready. The user can:

- Create projects with `granary projects create "Project Name" --description "..."`
- Create tasks with `granary project <id> tasks create "Task title" --description "..."`
- Start sessions with `granary session start "session-name"`
