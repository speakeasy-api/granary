---
name: granary-initiative-planner
description: Plan and organize multi-project initiatives or significant, substantial projects. Use when work spans multiple services, repositories, or has natural project boundaries.
---

# Planning Multi-Project Initiatives

Use this skill when work naturally spans multiple projects or services. **Do NOT use for single-feature work**—use `/granary:plan-work` instead.

**Your role:** High-level architecture, separation of concerns, dependency analysis, and spawning sub-agents for project planning. You do NOT plan individual project tasks yourself—you coordinate and synthesize.

## Your Focus as Initiative Planner

You are a **high-level coordinator**. Your job is to:

1. **Collect broad context** before any planning begins
2. **Design the project structure** with clear boundaries
3. **Spawn sub-agents** to handle detailed project planning
4. **Synthesize cross-project dependencies** from sub-agent reports

**You do NOT:**

- Plan detailed tasks within projects (sub-agents do this)
- Implement anything directly
- Make low-level technical decisions

## When to Use Initiatives

**Use initiatives when:**

- Work spans multiple services (API + frontend + workers)
- Implementation has clear phases with dependencies (schema -> API -> UI)
- Multiple teams or agents will work in parallel on different components
- You need to track cross-project dependencies

**Do NOT use initiatives when:**

- Work fits in a single project
- There are no cross-project dependencies
- It's a single feature in one service

## Step 1: Comprehensive Context Collection

**This is your most important step.** Before creating an initiative, gather broad context to inform the entire planning process. You are building the foundation that sub-agents will rely on.

### 1.1 Search for Prior Art

```bash
# Search granary for related existing work
granary search "feature keywords"
granary initiatives  # Check existing initiatives
granary projects     # See all projects
```

### 1.2 Understand Current Implementation

Research the codebase at a high level:

- What services/modules exist that will be affected?
- What boundaries already exist in the architecture?
- What shared dependencies or interfaces might be needed?
- What are the current **limitations** of the system?

```bash
# Example: understand existing architecture
ls -la src/
grep -r "mod " src/lib.rs
cat README.md
```

### 1.3 Gather External Context

**Use web search and documentation lookup** to understand:

- Industry best practices for similar features
- Framework/library documentation for relevant technologies
- Potential pitfalls or known issues others have encountered

This external research provides valuable context that sub-agents will benefit from when planning their projects.

### 1.4 Document Limitations and Constraints

Before proceeding, document:

- Technical limitations of the current system
- Resource or time constraints
- Dependencies on external systems or teams
- Known risks or uncertainties

### 1.5 Decision Tree

| Situation                                         | Action                              |
| ------------------------------------------------- | ----------------------------------- |
| Work fits in one project                          | Use `/granary:plan-work` instead    |
| Related initiative exists                         | Add projects to existing initiative |
| Work spans 2+ distinct projects with dependencies | Create new initiative               |

## Step 2: Create the Initiative

```bash
granary initiatives create "Initiative Name" \
  --description "High-level goal spanning multiple projects"
```

Example:

```bash
granary initiatives create "User Authentication System" \
  --description "Implement auth across API, web app, and mobile app with shared token service"
```

## Step 3: Design Separation of Concerns

Break the initiative into logical projects. Each project should:

- Be independently implementable (given its dependencies)
- Have a clear boundary (service, module, or phase)
- Be completable by a single agent

Think carefully about:

- **What depends on what?** Which components need to exist before others can start?
- **What can run in parallel?** Which projects have no dependencies on each other?
- **Where are the interfaces?** What contracts need to be defined between projects?

Example breakdown:

```
Initiative: User Authentication System
├── Project: Auth Token Service (shared backend) — no deps, can start first
├── Project: API Auth Integration — depends on token service
├── Project: Web App Login — depends on API auth
└── Project: Mobile App Login — depends on API auth (parallel with Web App)
```

## Step 4: Create Projects and Add to Initiative

```bash
# Create each project with clear descriptions for sub-agents
granary projects create "Auth Token Service" \
  --description "JWT token generation and validation service"
# Output: auth-token-service-abc1

granary projects create "API Auth Integration" \
  --description "Add authentication middleware to API endpoints"
# Output: api-auth-integration-def2

# Add projects to initiative
granary initiative <initiative-id> add-project auth-token-service-abc1
granary initiative <initiative-id> add-project api-auth-integration-def2
```

## Step 5: Analyze and Set Up Project Dependencies

**This is critical.** Review each project pair and ask:

- Does project A need anything from project B to start?
- Does project B produce interfaces/APIs that project A consumes?
- Are there shared schemas or contracts that must exist first?

```bash
# API Auth depends on Token Service being complete
granary project api-auth-integration-def2 deps add auth-token-service-abc1

# Web App depends on API Auth
granary project web-app-login-ghi3 deps add api-auth-integration-def2
```

**Key principle:** A project is blocked until ALL its dependency projects have ALL tasks done.

## Step 6: Verify the Dependency Structure

Before spawning sub-agents, verify the structure makes sense:

```bash
# View the initiative dependency graph
granary initiative <initiative-id> graph

# View as mermaid diagram (paste into GitHub/VSCode)
granary initiative <initiative-id> graph --format mermaid

# Check overall status
granary initiative <initiative-id> summary
```

Review the graph for:

- Circular dependencies (error)
- Missing dependencies (projects that should depend on each other but don't)
- Overly sequential structure (could more projects run in parallel?)

## Step 7: Spawn Sub-Agents for Project Planning

**Do NOT plan project tasks yourself.** Spawn sub-agents to handle detailed project planning. Each sub-agent focuses deeply on one project while you maintain the high-level view.

### 7.1 Get Projects Ready for Planning

```bash
# Get all projects in the initiative
granary initiative <initiative-id> projects

# Identify which projects are unblocked (no project dependencies)
granary initiative <initiative-id> graph
```

### 7.2 Spawn Sub-Agents with Large Context Models

**Critical:** Use the **largest context model available** (e.g., `claude-opus-4-5-20250101` or `opus`) for sub-agents. These planning tasks benefit from deep reasoning and extensive context windows.

For each unblocked project, spawn a sub-agent using the Task tool:

```
Use Task tool with:
  prompt: [see template below]
  subagent_type: "general-purpose"
  model: "claude-opus-4-5-20250101"  # Use largest available context model
  run_in_background: true  # Spawn in parallel when multiple projects are unblocked
```

### 7.3 Sub-Agent Prompt Template

Provide each sub-agent with:

1. **Project context** from your high-level research
2. **Clear directive** to use `/granary:plan-work`
3. **Instruction to report back** with cross-project dependencies

**Example prompt:**

```
Use /granary:plan-work to plan the project `auth-token-service-abc1`.

## Context
This project is part of the "User Authentication System" initiative.
Goal: Implement JWT token generation and validation service.

## High-Level Context from Initiative Planning
- The API will consume tokens via middleware (future project: api-auth-integration)
- Token format should be JWT with RS256 signing
- Existing auth patterns found in src/middleware/ use Bearer tokens
- Reference: https://jwt.io/introduction for JWT best practices

## Your Task
1. Research the project scope thoroughly
2. Create detailed tasks with file paths, implementation details, and acceptance criteria
3. Set up task dependencies within the project

## Required Report Back
When planning is complete, summarize:
1. **Tasks created** - List of tasks with brief descriptions
2. **Cross-project interfaces** - What APIs, types, or contracts does this project expose that other projects will depend on?
3. **Cross-project dependencies identified** - Did you discover dependencies on other initiative projects not yet captured?
4. **Risks or blockers** - Any concerns that affect the broader initiative
```

### 7.4 Spawn in Parallel

Spawn sub-agents for **all unblocked projects simultaneously**:

```
# If projects A, B, C have no project dependencies, spawn all three at once:

Use Task tool (all in ONE message):
  1. prompt: [Project A planning prompt]
     model: "claude-opus-4-5-20250101"
     run_in_background: true

  2. prompt: [Project B planning prompt]
     model: "claude-opus-4-5-20250101"
     run_in_background: true

  3. prompt: [Project C planning prompt]
     model: "claude-opus-4-5-20250101"
     run_in_background: true
```

### 7.5 Collect and Synthesize Reports

After sub-agents complete, review their reports for:

- **Cross-project dependencies** they identified (update initiative dependency graph if needed)
- **Shared interfaces** that need coordination between projects
- **Risks** that affect multiple projects

```bash
# Update dependencies if sub-agents identified new ones
granary project <project-B> deps add <project-A>

# Verify updated structure
granary initiative <initiative-id> graph --format mermaid
```

## Step 8: Hand Off to Orchestration

Once all projects have been planned by sub-agents, the initiative is ready for `/granary:orchestrate`:

```bash
# Verify all projects have tasks
granary initiative <initiative-id> summary

# Get the next actionable task across the entire initiative
granary initiative <initiative-id> next

# Get ALL unblocked tasks (for parallel execution)
granary initiative <initiative-id> next --all
```

## Example: Full Initiative Planning Flow

```bash
# 1. Research the codebase (understand existing architecture)

# 2. Create initiative
granary initiatives create "Payment Processing" \
  --description "Add payment support: Stripe integration, checkout flow, order management"

# 3. Design separation of concerns and create projects
granary projects create "Stripe Service" --description "Stripe API integration wrapper"
granary projects create "Checkout API" --description "Checkout endpoints and cart management"
granary projects create "Checkout UI" --description "Frontend checkout flow"
granary projects create "Order Service" --description "Order persistence and status tracking"

# 4. Add to initiative
granary initiative payment-processing-xyz1 add-project stripe-service-abc1
granary initiative payment-processing-xyz1 add-project checkout-api-def2
granary initiative payment-processing-xyz1 add-project checkout-ui-ghi3
granary initiative payment-processing-xyz1 add-project order-service-jkl4

# 5. Analyze dependencies and set them up
granary project checkout-api-def2 deps add stripe-service-abc1
granary project checkout-ui-ghi3 deps add checkout-api-def2
granary project order-service-jkl4 deps add checkout-api-def2

# 6. Verify structure
granary initiative payment-processing-xyz1 graph --format mermaid

# 7. Spawn sub-agents to plan unblocked projects
# stripe-service-abc1 is unblocked -> spawn agent with /granary:plan-work

# 8. Once planned, check readiness
granary initiative payment-processing-xyz1 summary
```

## Summary

1. **Collect context broadly** -> Prior art, current implementation, limitations, external docs, web search
2. **Assess** -> Is this truly multi-project work?
3. **Create initiative** -> High-level container with context documentation
4. **Design separation** -> Clear boundaries, identify interfaces
5. **Create projects** -> Add to initiative with good descriptions
6. **Analyze dependencies** -> Review all project pairs, set project-level dependencies
7. **Verify structure** -> Check graph for issues
8. **Spawn sub-agents** -> Use **largest context model** (opus), provide context, request cross-project dependency reports
9. **Synthesize reports** -> Update dependencies based on sub-agent findings
10. **Hand off** -> Orchestrator uses initiative-level commands

**Your output:** A well-structured initiative with projects and dependencies. Sub-agents handle detailed task planning within each project and report back cross-project dependencies they discover.

**Key principle:** You are a high-level coordinator. Gather broad context, design the structure, delegate deep planning to sub-agents, then synthesize their findings.
