---
name: playtest
description: Compile, validate, and report on the current game state
allowed-tools:
  - Bash
  - Read
  - Glob
  - Grep
  - Write
user-invocable: true
---

You are running a playtest validation on bevy-cat-game.

## Process

1. **Compile check**
   ```
   cd /Users/jannik/development/nikrich/bevy-cat-game && cargo check 2>&1
   ```
   If this fails, stop and report the errors.

2. **Lint check**
   ```
   cargo clippy -- -W clippy::all 2>&1
   ```
   Report any warnings (don't fail on them, but note them).

3. **Test suite**
   ```
   cargo test 2>&1
   ```
   Report pass/fail.

4. **Code health scan**
   - Search for `unwrap()` calls in game code (not tests) -- these should be `?` in Bevy 0.16
   - Search for TODO/FIXME/HACK comments
   - Check that all systems return `Result` where they use queries

5. **Architecture check**
   - Verify every module has a Plugin struct registered in main.rs
   - Verify no circular dependencies between modules
   - Check that components are data-only (no impl blocks with game logic)

6. **State report**
   Read `.claude/memory/game-design.md` and report:
   - Features implemented vs planned (percentage)
   - Next 3 items on the priority list
   - Any blockers

## Output

A structured report with sections: Build, Lint, Tests, Code Health, Architecture, Progress.
Mark each section PASS/WARN/FAIL.

## After playtest

If issues were found, update `.claude/memory/tech-debt.md` with any new debt items discovered.
