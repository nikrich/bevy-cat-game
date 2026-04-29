---
name: challenge
description: Stress-test a game design or architecture decision
argument-hint: "[approach] e.g. 'chunk system using HashMap', 'inventory as Resource'"
arguments:
  - approach
allowed-tools:
  - Read
  - Glob
  - Grep
  - WebSearch
user-invocable: true
---

You are stress-testing a game design or architectural approach for bevy-cat-game.

## Process

1. Read the current codebase to understand existing architecture
2. Read `.claude/memory/decisions.md` for prior decisions that may be affected
3. Read `.claude/memory/game-design.md` for game vision constraints

## Analysis

For the approach "$ARGUMENTS":

### 5 "What breaks when..." questions
Ask and answer five probing questions specific to:
- Bevy ECS constraints (system ordering, query conflicts, change detection)
- Performance at scale (10K+ entities, 100+ chunks, many spawns per frame)
- Player experience impact (latency, visual glitches, save file size)
- Future extensibility (will this block planned features?)
- Platform constraints (desktop + console memory, input differences)

### Edge cases
Identify 3 concrete scenarios where this approach fails or degrades.

### Alternatives
For each weakness found, propose a specific alternative approach with trade-offs.

## Verdict

Deliver one of:
- **PROCEED** -- approach is sound, weaknesses are minor
- **MODIFY** -- good direction but needs specific adjustments (list them)
- **RECONSIDER** -- fundamental issues, recommend alternative

Include a one-sentence summary of the strongest argument for and against.
