---
name: brainstorm
description: Generate game feature and mechanic ideas for a given area
argument-hint: "[area] e.g. 'crafting', 'biomes', 'water', 'NPCs', 'UI'"
arguments:
  - area
allowed-tools:
  - Read
  - Write
  - Glob
  - Grep
user-invocable: true
---

You are brainstorming game features and mechanics for the bevy-cat-game project.

## Process

1. Read `.claude/memory/game-design.md` to understand current game vision and what exists
2. Read `.claude/memory/palace.md` for art direction and preferences
3. Read `.claude/memory/decisions.md` for relevant prior decisions
4. Scan the current codebase (`src/`) to understand what's implemented

## Generate ideas

For the area "$ARGUMENTS", generate 5-8 ideas. Each idea must include:

- **What**: One-sentence description
- **Why it fits**: How it serves the peaceful/discovery/crafting identity
- **How**: Brief technical approach (Bevy systems, components, resources needed)
- **Effort**: S (hours) / M (day) / L (days)
- **Impact**: Low / Medium / High (on player experience)
- **Dependencies**: What must exist first

## Output format

1. Rank ideas by impact/effort ratio (best first)
2. Deep dive on the top 2: describe the ECS architecture (components, systems, events, states) and the player experience
3. Flag any ideas that conflict with existing decisions

## Constraints

- Align with peaceful, no-pressure game identity -- no combat, no death, no timers
- Respect the low-poly earthy aesthetic
- Consider Bevy 0.16 capabilities and limitations
- Think about what makes discovery feel rewarding without extrinsic rewards
- Keep scope realistic for an AI-built game
