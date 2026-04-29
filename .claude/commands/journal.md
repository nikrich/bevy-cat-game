---
name: journal
description: Summarize what was accomplished in this session
allowed-tools:
  - Read
  - Write
  - Bash
  - Glob
user-invocable: true
---

You are writing a session journal entry for bevy-cat-game.

## Process

1. Read `.claude/memory/journal.md` to see prior entries and format
2. Review the conversation to identify what was accomplished
3. Run `git diff --stat` and `git log --oneline -10` to see concrete changes
4. Scan for any new files created: `find src/ -name '*.rs' -newer .claude/memory/journal.md`

## Write the entry

Append a new entry at the TOP of journal.md (newest first), under the `# Session Journal` header:

```
## [date] -- [short theme]

**What was done:**
- [concrete accomplishment 1]
- [concrete accomplishment 2]

**Key decisions:**
- [DEC-NNN reference if any decisions were made]

**Files changed:**
- [created/modified/deleted files]

**Open threads:**
- [unfinished work or next steps]

**Blockers:**
- [issues hit and how they were resolved, or still open]
```

## Rules
- Be factual, not narrative. List what happened, not how you felt about it.
- Under 20 lines per entry.
- Only record what actually happened -- don't record planned work as done.
- Reference decision log entries by DEC-NNN when relevant.
- If tech debt was created, note the DEBT-NNN reference.
