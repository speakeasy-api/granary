#!/usr/bin/env bash
set -euo pipefail

# End-to-end test script for Granary CLI
# Assumes `granary` is installed and available in PATH

TEST_DIR=$(mktemp -d)
ORIGINAL_DIR=$(pwd)

cleanup() {
    echo ""
    echo "=== Cleaning up ==="
    cd "$ORIGINAL_DIR"
    rm -rf "$TEST_DIR"
    echo "Removed test directory: $TEST_DIR"
    echo "Cleanup complete."
}

trap cleanup EXIT

echo "=== Granary E2E Test ==="
echo "Test directory: $TEST_DIR"
echo ""

cd "$TEST_DIR"

# =============================================================================
# 1. Initialize workspace
# =============================================================================
echo "=== 1. Initialize Workspace ==="
granary init
echo "Workspace initialized."

echo ""
echo "=== 2. Doctor Check ==="
granary doctor
echo "Doctor check passed."

# =============================================================================
# 3. Create projects
# =============================================================================
echo ""
echo "=== 3. Create Projects ==="

PROJECT1_OUTPUT=$(granary projects create "Backend API" --description "REST API implementation" --owner "Team A" --tags "backend,api" --json)
PROJECT1_ID=$(echo "$PROJECT1_OUTPUT" | tr -d ' \n' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "Created project: $PROJECT1_ID"

PROJECT2_OUTPUT=$(granary projects create "Frontend App" --description "React frontend" --owner "Team B" --tags "frontend,react" --json)
PROJECT2_ID=$(echo "$PROJECT2_OUTPUT" | tr -d ' \n' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "Created project: $PROJECT2_ID"

echo ""
echo "=== 4. List Projects ==="
granary projects

# =============================================================================
# 5. Create tasks
# =============================================================================
echo ""
echo "=== 5. Create Tasks ==="

TASK1_OUTPUT=$(granary project "$PROJECT1_ID" tasks create "Set up database schema" --priority P0 --description "Design and implement PostgreSQL schema" --json)
TASK1_ID=$(echo "$TASK1_OUTPUT" | tr -d ' \n' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "Created task: $TASK1_ID"

TASK2_OUTPUT=$(granary project "$PROJECT1_ID" tasks create "Implement authentication" --priority P0 --description "JWT-based auth system" --owner "Alice" --json)
TASK2_ID=$(echo "$TASK2_OUTPUT" | tr -d ' \n' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "Created task: $TASK2_ID"

TASK3_OUTPUT=$(granary project "$PROJECT1_ID" tasks create "Build user endpoints" --priority P1 --description "CRUD operations for users" --json)
TASK3_ID=$(echo "$TASK3_OUTPUT" | tr -d ' \n' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "Created task: $TASK3_ID"

TASK4_OUTPUT=$(granary project "$PROJECT2_ID" tasks create "Set up React project" --priority P0 --description "Initialize with Vite" --json)
TASK4_ID=$(echo "$TASK4_OUTPUT" | tr -d ' \n' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "Created task: $TASK4_ID"

# =============================================================================
# 6. Add dependencies
# =============================================================================
echo ""
echo "=== 6. Add Task Dependencies ==="
granary task "$TASK2_ID" deps add "$TASK1_ID"
granary task "$TASK3_ID" deps add "$TASK1_ID" "$TASK2_ID"
echo "Dependencies added."

echo ""
echo "Dependency graph for $TASK3_ID:"
granary task "$TASK3_ID" deps graph

# =============================================================================
# 7. Get next task
# =============================================================================
echo ""
echo "=== 7. Get Next Task ==="
granary next --include-reason

# =============================================================================
# 8. Start and work on tasks
# =============================================================================
echo ""
echo "=== 8. Start Working on Tasks ==="
granary start "$TASK1_ID" --owner "Bob"
echo "Started task: $TASK1_ID"

echo ""
echo "=== 9. Add Comments ==="
granary task "$TASK1_ID" comments create --content "Starting schema design" --kind progress --author "Bob"
granary task "$TASK1_ID" comments create --content "Decided to use UUID for primary keys" --kind decision --author "Bob"
echo "Comments added."

echo ""
echo "List comments:"
granary task "$TASK1_ID" comments

# =============================================================================
# 10. Add artifacts
# =============================================================================
echo ""
echo "=== 10. Add Artifacts ==="
granary task "$TASK1_ID" artifacts add file "schema.sql" --description "Database schema file"
granary task "$TASK1_ID" artifacts add url "https://docs.example.com/db-design" --description "Design document"
echo "Artifacts added."

echo ""
echo "List artifacts:"
granary task "$TASK1_ID" artifacts

# =============================================================================
# 11. Complete first task
# =============================================================================
echo ""
echo "=== 11. Complete Task ==="
granary task "$TASK1_ID" done --comment "Schema implemented and tested"
echo "Task completed: $TASK1_ID"

# =============================================================================
# 12. List all tasks
# =============================================================================
echo ""
echo "=== 12. List All Tasks ==="
granary tasks --all

# =============================================================================
# 13. Session management
# =============================================================================
echo ""
echo "=== 13. Session Management ==="

SESSION_OUTPUT=$(granary session start "Sprint 1" --owner "Claude" --json)
SESSION_ID=$(echo "$SESSION_OUTPUT" | tr -d ' \n' | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
echo "Started session: $SESSION_ID"

granary session add project "$PROJECT1_ID"
granary session add task "$TASK2_ID"
echo "Added items to session scope."

echo ""
echo "Current session:"
granary session current

# =============================================================================
# 14. Focus and pin
# =============================================================================
echo ""
echo "=== 14. Focus and Pin ==="
granary focus "$TASK2_ID"
granary pin "$TASK4_ID"
echo "Set focus and pinned tasks."

# =============================================================================
# 15. Summary and context
# =============================================================================
echo ""
echo "=== 15. Generate Summary ==="
granary summary

echo ""
echo "=== 16. Export Context Pack ==="
granary context --include projects,tasks,comments

# =============================================================================
# 17. Checkpoints
# =============================================================================
echo ""
echo "=== 17. Checkpoints ==="
granary checkpoint create "mid-sprint"
echo "Checkpoint created."

granary checkpoint list

# =============================================================================
# 18. Block and unblock
# =============================================================================
echo ""
echo "=== 18. Block and Unblock Tasks ==="
granary task "$TASK2_ID" block --reason "Waiting for security review"
echo "Task blocked."

granary task "$TASK2_ID" unblock
echo "Task unblocked."

# =============================================================================
# 19. Create subtasks
# =============================================================================
echo ""
echo "=== 19. Create Subtasks ==="
granary task "$TASK2_ID" tasks create "Implement login endpoint" --priority P0
granary task "$TASK2_ID" tasks create "Implement logout endpoint" --priority P1
echo "Subtasks created."

echo ""
echo "List subtasks:"
granary task "$TASK2_ID" tasks

# =============================================================================
# 20. Configuration
# =============================================================================
echo ""
echo "=== 20. Configuration ==="
granary config set default_owner "Claude"
granary config set default_priority "P2"
granary config list

# =============================================================================
# 21. Steering files
# =============================================================================
echo ""
echo "=== 21. Steering Files ==="
granary steering add "CLAUDE.md" --mode always
granary steering add "docs/guidelines.md" --mode on-demand
granary steering list

# =============================================================================
# 22. Different output formats
# =============================================================================
echo ""
echo "=== 22. Output Formats ==="

echo "JSON format:"
granary tasks --all --json | head -20

echo ""
echo "YAML format:"
granary project "$PROJECT1_ID" --format yaml

echo ""
echo "Markdown format:"
granary tasks --all --format md

# =============================================================================
# 23. Update and archive
# =============================================================================
echo ""
echo "=== 23. Update Project ==="
granary project "$PROJECT1_ID" update --description "REST API with GraphQL support"
echo "Project updated."

echo ""
echo "=== 24. Archive Project ==="
granary project "$PROJECT2_ID" archive
echo "Project archived."

echo ""
echo "List projects (including archived):"
granary projects --all

# =============================================================================
# 24. Handoff generation
# =============================================================================
echo ""
echo "=== 25. Generate Handoff ==="
granary handoff --to "Review Agent" --tasks "$TASK2_ID,$TASK3_ID" --constraints "Focus on security" --acceptance-criteria "All tests pass"

# =============================================================================
# 25. Close session
# =============================================================================
echo ""
echo "=== 26. Close Session ==="
granary session close --summary "Completed database schema, auth in progress"
echo "Session closed."

echo ""
echo "List all sessions:"
granary sessions --all

# =============================================================================
# 26. Verify final state
# =============================================================================
echo ""
echo "=== 27. Final State ==="
echo "Projects:"
granary projects --all

echo ""
echo "Tasks:"
granary tasks --all

echo ""
echo "=== E2E Test Complete ==="
echo "All tests passed successfully!"
