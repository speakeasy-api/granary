---
name: granary-context-packing
description: Generate and consume context packs for LLM agents. Use when preparing context for sub-agents, creating summaries, or exporting session state.
---

# Context Packing Skill

Generate and consume context packs for LLM agents. Use this skill when preparing context for sub-agents, creating summaries, or exporting session state.

## Summary Command

Generate a compact summary of the current session state:

```bash
granary summary
```

### With Options

```bash
granary summary --token-budget 1200 --format prompt
```

### What's Included in Summaries

- **Session info:** Current session ID, start time, duration
- **Task counts:** Total tasks, completed, in-progress, blocked
- **Focus:** Current active task or objective
- **Blockers:** Outstanding blockers preventing progress
- **Decisions:** Key decisions made during the session

## Context Export

Export full context for handoffs or sub-agent preparation:

```bash
granary context --format prompt
```

### Selective Export

Include only specific context types:

```bash
granary context --include decisions,blockers,artifacts --max-items 50
```

### Available Include Options

- `decisions` - Key decisions made
- `blockers` - Current blockers
- `artifacts` - Created or modified files
- `tasks` - Task list and status
- `history` - Action history
- `notes` - Session notes

## Format Options

Both `summary` and `context` commands support multiple output formats:

| Format   | Flag              | Use Case                       |
| -------- | ----------------- | ------------------------------ |
| Table    | `--format table`  | Human-readable terminal output |
| JSON     | `--format json`   | Programmatic consumption       |
| YAML     | `--format yaml`   | Configuration files            |
| Markdown | `--format md`     | Documentation                  |
| Prompt   | `--format prompt` | LLM context injection          |

## Token Budget Management

Cap output size to fit within LLM context windows:

```bash
# Limit to 1200 tokens
granary summary --token-budget 1200

# Limit context export to 4000 tokens
granary context --token-budget 4000 --format prompt
```

When token budget is exceeded, content is prioritized:

1. Current focus and blockers (highest priority)
2. Recent decisions
3. In-progress tasks
4. Completed tasks (truncated first)

## Using Context in Prompts

Capture context for injection into prompts or sub-agent calls:

```bash
CONTEXT=$(granary context --format prompt --max-items 30)
```

### Example: Sub-agent Handoff

```bash
# Export context for a sub-agent
CONTEXT=$(granary context --format prompt --token-budget 2000)

# Use in a prompt
echo "Given this context:
$CONTEXT

Please continue with the next task."
```

### Example: Loop Iteration Summary

```bash
# Get compact summary for agentic loop
SUMMARY=$(granary summary --token-budget 500 --format prompt)
```

## Best Practices

### Use Summaries for Loops

When running in agentic loops with repeated LLM calls, use `summary` with a tight token budget to preserve context window space:

```bash
granary summary --token-budget 500 --format prompt
```

### Use Full Context for Handoffs

When handing off to a sub-agent or new session, use `context` with comprehensive includes:

```bash
granary context --format prompt --include decisions,blockers,tasks,artifacts
```

### Match Token Budget to Model

- **Small context models (4K-8K):** Use `--token-budget 500-1000`
- **Medium context models (32K):** Use `--token-budget 2000-4000`
- **Large context models (128K+):** Use `--token-budget 8000-16000`

### Prefer Prompt Format for LLMs

The `--format prompt` output is optimized for LLM consumption with clear structure and minimal noise.

### Cache Context for Multiple Uses

If making multiple sub-agent calls, capture context once:

```bash
CONTEXT=$(granary context --format prompt --token-budget 3000)

# Reuse for multiple calls
agent_call_1 "$CONTEXT"
agent_call_2 "$CONTEXT"
```
