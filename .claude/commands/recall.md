---
name: recall
description: Search all memory for relevant information
argument-hint: "[search query]"
arguments:
  - query
allowed-tools:
  - Read
  - Grep
user-invocable: true
---

You are searching the bevy-cat-game memory palace for information.

## Process

1. Search across ALL memory files:
   - `.claude/memory/palace.md` -- general knowledge
   - `.claude/memory/decisions.md` -- architectural decisions
   - `.claude/memory/game-design.md` -- game design doc
   - `.claude/memory/tech-debt.md` -- tech debt register
   - `.claude/memory/journal.md` -- session history
2. Match broadly: if query is "terrain", also check for "world", "ground", "noise", "biome", "chunk"
3. Group results by source file
4. Include dates where available

## Output format

For each match:
```
**[Source file]** ([date if available])
> [relevant content]
```

## Rules
- Search broadly -- "camera" should also find "isometric", "follow", "zoom", "orthographic"
- Flag entries older than 2 weeks as potentially stale
- If nothing found, say so clearly and suggest what to search instead
- If a memory conflicts with current code state, flag the discrepancy
