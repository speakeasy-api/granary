### Improved agent workflow prompts to prevent initiative planning mistakes

Agents planning multi-project initiatives were incorrectly creating tasks directly instead of delegating to sub-agents. The root cause: `granary plan` showed inviting task-creation templates, and `granary initiate` buried the delegation requirement in Step 3 of 4.

All agent-facing prompts have been redesigned to make the correct workflow path unmistakable.

#### `granary initiate` now front-loads a delegation constraint

The output now begins with a `## CRITICAL: Delegation-Only Workflow` section before any steps:

```
## CRITICAL: Delegation-Only Workflow

You are the initiative coordinator. Your job is to create projects and delegate.
Do NOT create tasks directly. Do NOT use `granary project <id> tasks create`.
Task creation is handled by sub-agents via `granary plan --project <id>`.
```

Step 3 was also renamed from "Launch Sub-Agents for Planning" to "Delegate Planning to Sub-Agents" with reinforcement:

```
Do NOT create tasks yourself — this is the sub-agent's responsibility.
```

#### `granary plan` now warns initiative agents to stop

When an agent runs `granary plan "Feature name"` (without `--project`), the output now includes a scope guard:

```
## Scope: Single-Project Planning

This workflow is for planning ONE project with tasks.
If this is part of a multi-project initiative, stop here.
Use `granary initiate "Initiative name"` instead — it will coordinate
project creation and delegate task planning to sub-agents.
```

#### Entrypoint and help text now distinguish the two paths

Both `granary` (bare command) and `granary --help` now present a decision tree instead of a flat list:

```
Choose ONE entry point based on scope:
- Single project (one feature/fix): `granary plan "Feature name"`
- Multiple projects (cross-cutting work): `granary initiate "Initiative name"`
```

The `--help` text also adds an explicit note:

```
NOTE: Do NOT use `granary plan` for multi-project work.
`initiate` coordinates projects and delegates task planning to sub-agents.
```