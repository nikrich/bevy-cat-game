---
name: decision-log
description: Record or query architectural and design decisions
argument-hint: "[record|query] [description]"
arguments:
  - action
allowed-tools:
  - Read
  - Write
  - Grep
user-invocable: true
---

You manage the architectural decision log for bevy-cat-game.

## If action is "record"

1. Read `.claude/memory/decisions.md`
2. Determine the next DEC-NNN number
3. Ask yourself: is this a genuine architectural decision (irreversible or costly to change), or just an implementation detail? Only log the former.
4. Append a new entry:

```
## DEC-NNN: [Short title]
- **Date**: [today]
- **Status**: Accepted
- **Context**: [Why this decision was needed -- the problem or tension]
- **Decision**: [What was chosen and the key reasoning]
- **Alternatives**: [What else was considered and why rejected]
- **Consequences**: [Trade-offs accepted, what this enables/prevents]
```

5. Write the updated file

**Rules:**
- Decisions are immutable once recorded. To reverse, create a new decision that supersedes.
- Focus on WHY, not what. The code shows what; the log explains why.
- Reference related decisions by DEC-NNN when relevant.

## If action is "query"

1. Read `.claude/memory/decisions.md`
2. Search for entries matching the query
3. Return matching decisions with their full context
4. If a decision seems stale (context has changed), flag it
