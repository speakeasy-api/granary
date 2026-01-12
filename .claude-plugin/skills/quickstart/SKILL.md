---
name: granary-quickstart
description: Quick start guide for using granary to manage tasks and sessions. Use when setting up granary in a new project, starting a new work session, or learning granary basics.
---

# Granary Quick Start Guide

This guide walks through setting up granary for task and session management in a new project.

## 1. Initialize Granary

First, initialize granary in your project directory:

```bash
granary init
```

Example output:

```json
{
  "success": true,
  "message": "Initialized granary in /path/to/project/.granary",
  "config": {
    "version": "0.1",
    "created_at": "2024-01-15T10:00:00Z"
  }
}
```

## 2. Create Your First Project

Create a project to organize related tasks:

```bash
granary projects create "API Refactoring" --description "Refactor REST API to use new patterns" --owner "Agent"
```

Example output:

```json
{
  "id": "proj_01",
  "name": "API Refactoring",
  "description": "Refactor REST API to use new patterns",
  "owner": "Agent",
  "status": "active",
  "created_at": "2024-01-15T10:05:00Z",
  "tasks": []
}
```

## 3. Create Your First Task

Add a task to your project using the project ID:

```bash
granary project proj_01 tasks create "Update authentication middleware" --description "Migrate auth middleware to new token format" --priority P1
```

Example output:

```json
{
  "id": "task_01",
  "project_id": "proj_01",
  "title": "Update authentication middleware",
  "description": "Migrate auth middleware to new token format",
  "priority": "P1",
  "status": "pending",
  "created_at": "2024-01-15T10:10:00Z"
}
```

## 4. Start a Session

Start a work session to track your progress:

```bash
granary session start "api-refactor-session" --owner "Agent" --mode plan
```

Example output:

```json
{
  "id": "sess_01",
  "name": "api-refactor-session",
  "owner": "Agent",
  "mode": "plan",
  "status": "active",
  "started_at": "2024-01-15T10:15:00Z",
  "scope": []
}
```

Session modes:

- `plan` - Planning and organizing work
- `execute` - Actively working on tasks
- `review` - Reviewing completed work

## 5. Add Project to Session Scope

Add the project to your session's scope to focus your work:

```bash
granary session add project proj_01
```

Example output:

```json
{
  "session_id": "sess_01",
  "scope": [
    {
      "type": "project",
      "id": "proj_01",
      "name": "API Refactoring"
    }
  ],
  "message": "Added project proj_01 to session scope"
}
```

## 6. Get Next Task

Find the next task to work on based on priority:

```bash
granary next
```

Example output:

```json
{
  "task": {
    "id": "task_01",
    "project_id": "proj_01",
    "title": "Update authentication middleware",
    "description": "Migrate auth middleware to new token format",
    "priority": "P1",
    "status": "pending"
  },
  "reason": "Highest priority task in session scope"
}
```

## 7. Basic Workflow Loop

### Start Working on a Task

```bash
granary task task_01 start
```

Example output:

```json
{
  "id": "task_01",
  "status": "in_progress",
  "started_at": "2024-01-15T10:20:00Z",
  "message": "Task started"
}
```

### Do Your Work

Perform the actual work for the task (coding, writing, etc.)

### Mark Task as Done

```bash
granary task task_01 done
```

Example output:

```json
{
  "id": "task_01",
  "status": "completed",
  "completed_at": "2024-01-15T11:00:00Z",
  "message": "Task completed"
}
```

### Get Next Task and Repeat

```bash
granary next
```

Continue the loop until all tasks are complete.

## Summary

The basic granary workflow is:

1. `granary init` - Initialize in your project
2. `granary projects create` - Create a project
3. `granary project <id> tasks create` - Add tasks
4. `granary session start` - Start a work session
5. `granary session add project <id>` - Focus your scope
6. `granary next` - Get next task
7. `granary task <id> start` - Begin work
8. `granary task <id> done` - Complete task
9. Repeat steps 6-8
