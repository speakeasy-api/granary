---
name: granary-task-design
description: Write effective task descriptions that enable autonomous sub-agent execution. Use when creating tasks, subtasks, or breaking down work for delegation.
---

# Task Design Guide

This skill helps you write effective task descriptions that enable autonomous sub-agent execution.

## Core Principle: Task as Context Transfer

**The task description is the ONLY context a sub-agent receives when delegated work.**

Every task description must answer three questions:

1. **WHY** - Why does this task exist? What problem does it solve?
2. **WHAT** - What specific deliverable is expected?
3. **HOW** (or constraints) - Technical approach, patterns to follow, or boundaries

If any of these are missing, the sub-agent will either ask clarifying questions (wasting time) or make assumptions (risking incorrect work).

## Task Description Template

Use this template for all task descriptions:

```markdown
**Goal:** One-sentence summary of what this task accomplishes

**Context:** Why this task exists, relevant background, how it fits into the larger picture

**Requirements:**

- Specific deliverable 1
- Specific deliverable 2
- Specific deliverable 3

**Technical Notes:**

- Relevant code locations (files, modules, functions)
- Patterns to follow from existing code
- Things to explicitly avoid
- Dependencies or libraries to use

**Acceptance Criteria:**

- [ ] Criterion 1 - specific, measurable
- [ ] Criterion 2 - specific, measurable
- [ ] Criterion 3 - specific, measurable

**References:**

- Links to relevant documentation
- Related issues or PRs
- Design documents
```

## Task Granularity

**One task = one reviewable unit of work.**

Guidelines:

- A task should be completable in a single focused session
- If the description exceeds 500 words, the task is too large - break it down
- Each task should produce a single, coherent change that can be reviewed independently
- Avoid tasks that require multiple unrelated changes

Signs a task is too large:

- Multiple unrelated acceptance criteria
- "And also..." appearing in requirements
- Touching more than 3-4 files in unrelated parts of the codebase

## Priority Assignment

| Priority | Meaning                                      | Examples                                                          |
| -------- | -------------------------------------------- | ----------------------------------------------------------------- |
| **P0**   | Blocking/Critical - Must be done immediately | Production outage, security vulnerability, blocking other work    |
| **P1**   | Important - Should be done soon              | Key feature for upcoming release, significant bug affecting users |
| **P2**   | Normal - Standard priority                   | Regular feature work, non-critical bugs, improvements             |
| **P3**   | Nice to have - Do when time permits          | Minor enhancements, code cleanup, small optimizations             |
| **P4**   | Backlog - Maybe someday                      | Ideas, future considerations, low-impact improvements             |

Default to P2 unless there's a specific reason for higher or lower priority.

## Dependencies

Add a dependency when a task **CANNOT start** until another task finishes.

**Good reasons for dependencies:**

- Task B modifies code that Task A will create
- Task B requires APIs or interfaces defined in Task A
- Task B tests functionality implemented in Task A

**Do NOT add dependencies for:**

- Tasks that could theoretically be done in parallel
- "It would be nice to do A before B" preferences
- Related but independent work

Over-constraining with unnecessary dependencies creates bottlenecks and reduces parallelism.

## Subtasks vs Dependencies

Use **subtasks** for hierarchical breakdown of a single piece of work:

- Parent task: "Implement user authentication"
  - Subtask: "Create login form component"
  - Subtask: "Implement JWT token handling"
  - Subtask: "Add session persistence"

Use **dependencies** for sequencing different work streams:

- Task: "Add user profile page" depends on "Implement user authentication"
- Task: "Write integration tests" depends on "Implement API endpoints"

Key difference: Subtasks are parts of a whole; dependencies are separate wholes that must be sequenced.

## Anti-Patterns to Avoid

### Vague descriptions

**Bad:** "Fix the bug"

- No context about what bug
- No information about where it occurs
- No acceptance criteria

**Bad:** "Implement feature X"

- No context about why
- No specific requirements
- No acceptance criteria

### Missing context

**Bad:** "Update the handler to use the new format"

- Which handler?
- What new format?
- Where is the format defined?

### Implicit knowledge

**Bad:** "Do it like we did for the other service"

- Which service?
- What pattern specifically?
- Sub-agent has no memory of previous work

### Kitchen sink tasks

**Bad:** "Implement auth, add tests, update docs, and fix the login bug"

- Multiple unrelated deliverables
- Impossible to review as a unit
- Should be 4 separate tasks

## Good Task Example

Here is a concrete example of a well-structured task:

```markdown
**Goal:** Add rate limiting to the API authentication endpoint

**Context:** Our `/api/auth/login` endpoint is vulnerable to brute force attacks.
We've seen suspicious traffic patterns in logs suggesting automated login attempts.
This is part of our Q1 security hardening initiative.

**Requirements:**

- Implement rate limiting on POST /api/auth/login
- Limit to 5 attempts per IP address per 15-minute window
- Return 429 Too Many Requests when limit exceeded
- Include Retry-After header in 429 responses

**Technical Notes:**

- Use the existing RateLimiter class in `/src/middleware/rate-limiter.ts`
- Follow the pattern used in `/src/routes/api/password-reset.ts`
- Store rate limit state in Redis (connection already configured)
- Do NOT use in-memory storage (won't work with multiple server instances)

**Acceptance Criteria:**

- [ ] Rate limiting applied to /api/auth/login endpoint
- [ ] 6th request within 15 minutes returns 429 status
- [ ] 429 response includes accurate Retry-After header
- [ ] Rate limit state persists in Redis
- [ ] Unit tests cover rate limit logic
- [ ] Integration test verifies 429 response

**References:**

- Security audit findings: https://internal.docs/security-audit-q1
- RateLimiter class docs: /docs/middleware.md#rate-limiter
- Related PR with similar implementation: #234
```

This example works because:

- Goal is clear and specific
- Context explains why this matters (security concern, audit findings)
- Requirements are concrete and measurable
- Technical notes point to existing patterns and warn about pitfalls
- Acceptance criteria are checkboxes that can be verified
- References provide additional context without cluttering the description
