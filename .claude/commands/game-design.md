---
name: game-design
description: View or update the living game design document
argument-hint: "[update|show] [section] [details]"
arguments:
  - action
allowed-tools:
  - Read
  - Write
  - Glob
  - Grep
user-invocable: true
---

You manage the living game design document for bevy-cat-game.

## If action is "show"

1. Read `.claude/memory/game-design.md`
2. Present it clearly, highlighting:
   - What's implemented (checked items)
   - What's next (first unchecked item)
   - Any sections that seem outdated vs current code

## If action is "update"

1. Read `.claude/memory/game-design.md`
2. Read current codebase to verify what's actually implemented
3. Update the relevant section based on the provided details
4. Sync the checklist -- mark items as done `[x]` if code exists, `[ ]` if not
5. Write the updated file

**Rules:**
- The game-design.md is the source of truth for "what are we building"
- Keep descriptions concise -- this is a reference, not a pitch deck
- When adding new systems to the priority list, slot them where they make sense relative to dependencies
- When marking something done, verify the code actually exists first
- Never remove planned features without user approval -- move to a "deferred" section instead
