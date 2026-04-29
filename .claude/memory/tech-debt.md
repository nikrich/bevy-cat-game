# Tech Debt Register

## DEBT-001: Placeholder player model
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: player
- **What**: Player is a capsule primitive, not the actual cat model
- **Why**: Cat .glb not yet provided
- **Fix when**: User provides the cat model asset
- **Effort**: S
- **Status**: Open

## DEBT-002: No chunk system
- **Added**: 2026-04-29
- **Severity**: High
- **Area**: world
- **What**: Entire 64x64 terrain spawns at startup -- no chunk loading/unloading
- **Why**: MVP scaffold, get something on screen first
- **Fix when**: Before adding more terrain features or props
- **Effort**: L
- **Status**: Open

## DEBT-003: No game states
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: core
- **What**: No state machine (Loading/Menu/Playing/Paused) -- goes straight to gameplay
- **Why**: MVP scaffold
- **Fix when**: Before adding menus or save/load
- **Effort**: M
- **Status**: Open

## DEBT-004: Hardcoded world seed
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: world
- **What**: Perlin seed is hardcoded to 42
- **Why**: MVP scaffold
- **Fix when**: When adding save/load or world selection
- **Effort**: S
- **Status**: Open
