---
name: granary-project-design
description: Design effective granary projects with clear descriptions, appropriate tags, and steering references. Use when creating new projects or restructuring existing ones.
---

# Project Design Guide

This skill helps you design effective granary projects with clear structure, meaningful metadata, and proper steering references.

## Project Naming Conventions

### Use Descriptive Slugs

- Names should clearly communicate the project's purpose
- Use action-oriented verbs when applicable
- Keep names concise but meaningful

**Good examples:**

- `implement-oauth2-authentication`
- `migrate-postgres-to-cockroachdb`
- `refactor-payment-processing`
- `add-user-analytics-dashboard`

**Avoid:**

- `auth-stuff` (too vague)
- `new-feature` (not descriptive)
- `fix-things` (unclear scope)
- `project-1` (meaningless)

### Naming Patterns

| Pattern                  | When to Use            | Example                     |
| ------------------------ | ---------------------- | --------------------------- |
| `implement-{feature}`    | New functionality      | `implement-two-factor-auth` |
| `migrate-{from}-to-{to}` | Data/system migrations | `migrate-mysql-to-postgres` |
| `refactor-{component}`   | Code restructuring     | `refactor-api-layer`        |
| `fix-{issue}`            | Bug fixes or repairs   | `fix-memory-leak-in-worker` |
| `add-{capability}`       | Extensions             | `add-export-to-csv`         |
| `upgrade-{dependency}`   | Dependency updates     | `upgrade-react-to-v19`      |

## Description Best Practices

A well-written description follows this structure:

### 1. Start with WHY

Begin with the motivation or business value. Why does this project exist?

### 2. Define Success Criteria

What does "done" look like? Be specific and measurable.

### 3. List Constraints

What limitations or requirements must be respected?

### Description Template

```
{Why this project matters - 1-2 sentences}

Success criteria:
- {Measurable outcome 1}
- {Measurable outcome 2}

Constraints:
- {Limitation or requirement 1}
- {Limitation or requirement 2}
```

### Example Descriptions

**Good:**

```
Add OAuth2 support for Google and GitHub login to reduce friction for new user signups.

Success criteria:
- Users can sign in with Google accounts
- Users can sign in with GitHub accounts
- Existing session auth continues to work
- Login flow completes in under 3 seconds

Constraints:
- No breaking changes to existing session auth
- Must comply with OAuth2 security best practices
- Support both web and mobile clients
```

**Avoid:**

```
Add OAuth login.
```

(Too brief - no context, success criteria, or constraints)

## Tags for Organization

Use tags to categorize and filter projects effectively.

### Category Tags

| Tag              | Purpose                  |
| ---------------- | ------------------------ |
| `feature`        | New functionality        |
| `bugfix`         | Bug repairs              |
| `refactor`       | Code improvements        |
| `infrastructure` | DevOps/platform work     |
| `documentation`  | Docs updates             |
| `security`       | Security-related work    |
| `performance`    | Optimization work        |
| `tech-debt`      | Technical debt reduction |

### Priority Tags

| Tag  | Meaning                    |
| ---- | -------------------------- |
| `P0` | Critical - drop everything |
| `P1` | High priority - next up    |
| `P2` | Medium priority - planned  |
| `P3` | Low priority - backlog     |

### Team Tags

Use team identifiers for ownership:

- `team-backend`
- `team-frontend`
- `team-platform`
- `team-mobile`

### Combining Tags

Apply multiple relevant tags:

```bash
--tags "feature,security,P1,team-backend"
```

## Owner Assignment

### When to Assign an Owner

- Project has a clear responsible team or individual
- Accountability is needed for tracking
- Project is actively being worked on

### When to Leave Unassigned

- Project is in ideation/proposal phase
- Ownership needs to be discussed
- Cross-functional project without clear lead

### Owner Formats

```bash
--owner "Backend Team"
--owner "alice@example.com"
--owner "Platform Engineering"
```

## Steering References

Link projects to relevant standards and guidelines using steering references.

### What to Reference

- **Coding standards** - Language-specific style guides
- **ADRs** - Architecture Decision Records
- **Style guides** - UI/UX guidelines
- **Security policies** - Compliance requirements
- **API standards** - REST/GraphQL conventions

### Adding Steering References

```bash
granary projects update {project-id} \
  --add-steering ".granary/steering/coding-standards.md"

granary projects update {project-id} \
  --add-steering ".granary/steering/security-policy.md"
```

### Creating Project-Specific Steering

For project-specific guidance, create a steering file:

```bash
# Create steering file first
echo "# OAuth2 Implementation Standards
- Use PKCE for all OAuth flows
- Store tokens in httpOnly cookies
- Implement token refresh logic" > .granary/steering/oauth2-standards.md

# Then reference it
granary projects update {project-id} \
  --add-steering ".granary/steering/oauth2-standards.md"
```

## Default Session Policy

Configure which steering files are automatically included in sessions.

### Auto-Include Rules

Set up default steering for all sessions in a project:

```bash
granary projects update {project-id} \
  --session-policy "auto-include:.granary/steering/coding-standards.md"
```

### Multiple Auto-Includes

```bash
granary projects update {project-id} \
  --session-policy "auto-include:.granary/steering/coding-standards.md" \
  --session-policy "auto-include:.granary/steering/testing-guidelines.md"
```

## Complete Example

Here's a full project creation with all best practices applied:

```bash
granary projects create "Implement OAuth2 Authentication" \
  --description "Add OAuth2 support for Google and GitHub login to reduce signup friction and improve user conversion rates.

Success criteria:
- Users can sign in with Google accounts
- Users can sign in with GitHub accounts
- Existing session auth continues to work unchanged
- Login flow completes in under 3 seconds
- 95% of OAuth attempts succeed

Constraints:
- No breaking changes to existing session auth
- Must comply with OAuth2 security best practices (PKCE, secure token storage)
- Support both web and mobile clients
- Pass security review before launch" \
  --owner "Backend Team" \
  --tags "feature,security,P1,team-backend"
```

Then add steering references:

```bash
granary projects update {project-id} \
  --add-steering ".granary/steering/security-policy.md" \
  --add-steering ".granary/steering/api-standards.md" \
  --session-policy "auto-include:.granary/steering/coding-standards.md"
```

## Checklist for New Projects

Before creating a project, verify:

- [ ] Name is descriptive and action-oriented
- [ ] Description explains WHY the project exists
- [ ] Success criteria are specific and measurable
- [ ] Constraints are documented
- [ ] Appropriate category and priority tags applied
- [ ] Owner assigned (if applicable)
- [ ] Relevant steering references linked
- [ ] Session policy configured for auto-includes
