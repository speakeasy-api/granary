---
name: granary-steering
description: Use steering files to apply coding standards and conventions in granary. Use when setting up project standards or ensuring consistent agent behavior.
---

# Granary Steering

Steering files are references to standards, conventions, and style guides that help maintain consistent behavior across AI agent interactions. Inspired by the concept of "specs" in tools like Kiro, steering provides a way to codify project decisions and ensure agents follow established patterns.

## What is Steering?

Steering files are documents that guide AI agent behavior by providing:

- **Coding standards** - Language-specific style guides and conventions
- **Architectural decisions** - ADRs (Architecture Decision Records) that document why certain choices were made
- **API design guidelines** - Patterns for consistent API development
- **Security requirements** - Non-negotiable security practices and constraints
- **Project conventions** - Team-specific patterns and practices

When agents have access to steering files, they can produce output that aligns with your project's established norms rather than relying on generic best practices.

## Managing Steering Files

### List Current Steering References

View all steering files currently configured:

```bash
granary steering list
```

This displays the paths to all documents that agents should reference when working on your project.

### Add a Steering Reference

Register a new document as a steering reference:

```bash
granary steering add ./docs/CODING_STANDARDS.md
```

You can add multiple files:

```bash
granary steering add ./docs/API_GUIDELINES.md
granary steering add ./adrs/0001-use-typescript.md
granary steering add ./security/REQUIREMENTS.md
```

### Remove a Steering Reference

Remove a document from steering:

```bash
granary steering rm ./docs/old-standards.md
```

## Project-Level Steering

For project-specific steering that applies only to a particular project context, use the project update command:

```bash
granary project <project-id> update --steering-refs "./docs/STANDARDS.md,./adrs/*.md"
```

This allows different projects within the same repository to have different steering configurations.

## Types of Steering Documents

### Coding Standards

Language and framework conventions:

```markdown
# TypeScript Standards

- Use strict mode
- Prefer interfaces over type aliases for object shapes
- Use named exports over default exports
- Maximum function length: 50 lines
```

### Architecture Decision Records (ADRs)

Document significant decisions with context:

```markdown
# ADR-0001: Use Event Sourcing for Order Management

## Status

Accepted

## Context

We need reliable audit trails for all order changes...

## Decision

Implement event sourcing for the Order aggregate...

## Consequences

- All order mutations are captured as events
- Agents must append events, never mutate state directly
```

### API Design Guidelines

Consistent API patterns:

```markdown
# API Standards

- Use REST with JSON:API specification
- Version APIs in URL path: /api/v1/...
- Use kebab-case for URL paths
- Return 201 for resource creation with Location header
```

### Security Requirements

Non-negotiable security practices:

```markdown
# Security Requirements

- Never log sensitive data (passwords, tokens, PII)
- All user input must be validated and sanitized
- Use parameterized queries for all database operations
- Secrets must come from environment variables
```

## Using Steering in Workflows

### Read at Session Start

When beginning work on a project, read steering files to establish context:

```bash
# List and review steering files
granary steering list

# Read specific steering documents as needed
```

### Include in Context Packs

Reference steering files when creating context packs for agents:

```bash
granary context pack create --include-steering
```

This ensures agents always have access to current standards.

### Reference During Code Review

When reviewing code or making decisions, check steering files for applicable guidance:

```bash
granary steering list | xargs -I {} echo "Checking: {}"
```

## Best Practices

### Keep Documents Concise

- Focus on actionable rules and patterns
- Avoid lengthy explanations when brief statements suffice
- Use examples to clarify ambiguous points
- Break large documents into focused, single-purpose files

### Update on Decisions

- Add new ADRs when significant decisions are made
- Update standards when conventions evolve
- Remove or archive outdated steering files
- Date and version your steering documents

### Version Control

- Keep steering files in the repository alongside code
- Review steering changes in pull requests
- Use meaningful commit messages for steering updates
- Consider a CHANGELOG for steering document evolution

### Organize by Domain

Structure steering files logically:

```
docs/
  steering/
    coding/
      typescript.md
      testing.md
    api/
      rest-guidelines.md
      error-handling.md
    security/
      authentication.md
      data-handling.md
    adrs/
      0001-event-sourcing.md
      0002-graphql-subscriptions.md
```

### Review Periodically

- Schedule regular reviews of steering documents
- Remove outdated or contradictory guidance
- Consolidate fragmented standards
- Gather team feedback on steering effectiveness
