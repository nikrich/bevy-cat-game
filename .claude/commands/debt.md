---
name: debt
description: Track and manage technical debt
argument-hint: "[add|list|resolve|scan] [description]"
arguments:
  - action
allowed-tools:
  - Read
  - Write
  - Glob
  - Grep
user-invocable: true
---

You manage the tech debt register for bevy-cat-game.

## Actions

### add
1. Read `.claude/memory/tech-debt.md`
2. Determine next DEBT-NNN number
3. Append new entry:
```
## DEBT-NNN: [Short title]
- **Added**: [today]
- **Severity**: [Low|Medium|High|Critical]
- **Area**: [core|world|player|camera|ui|audio|crafting|building]
- **What**: [What's wrong or suboptimal]
- **Why**: [Why it was done this way]
- **Fix when**: [Trigger condition -- when should this be addressed]
- **Effort**: [S|M|L]
- **Status**: Open
```

### list
1. Read `.claude/memory/tech-debt.md`
2. Group by severity, show counts
3. Highlight items whose "fix when" trigger has been met

### resolve
1. Read `.claude/memory/tech-debt.md`
2. Find the matching entry
3. Update status to "Resolved" with date and note
4. Write the updated file

### scan
1. Search codebase for TODO, FIXME, HACK, XXX, TEMP, PLACEHOLDER comments
2. Cross-reference with existing debt register
3. Report any untracked debt found in code
4. Offer to add new entries for untracked items
