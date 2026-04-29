---
name: remember
description: Save something to the memory palace
argument-hint: "[what to remember]"
arguments:
  - memory
allowed-tools:
  - Read
  - Write
user-invocable: true
---

You are saving information to the bevy-cat-game memory palace.

## Process

1. Read `.claude/memory/palace.md`
2. Categorize the memory into the correct section:
   - **Art direction**: visual style, colors, aesthetic choices
   - **Game vision**: gameplay philosophy, core identity, target audience
   - **Technical notes**: implementation discoveries, gotchas, performance findings
   - **Preferences**: user preferences for how work should be done
   - **External resources**: links, tools, references
   - **Ideas parking lot**: early-stage ideas not yet in game-design.md
3. Format as a date-stamped one-liner: `- [YYYY-MM-DD] [the memory]`
4. Check for duplicates -- update existing entry if it's a refinement
5. Write the updated file

## Rules
- Never store secrets, passwords, API keys, or tokens
- Keep entries concise -- one line per memory
- If the memory is an architectural decision, suggest using `/decision-log record` instead
- If the memory is a game design change, suggest using `/game-design update` instead
- If the memory is tech debt, suggest using `/debt add` instead
