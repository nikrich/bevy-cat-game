# Tech Debt Register

## DEBT-001: Placeholder player model
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: player
- **What**: Player is a capsule primitive, not the actual cat model
- **Why**: cat.glb exists in assets/models/ but Mixamo animations fail to play visually despite correct data. Likely Blender 5.1 glTF export issue. See feedback_animation_pitfalls.md in memory.
- **Fix when**: Validate GLB in external viewer first, then retry. Consider Blender 4.x or pre-animated asset.
- **Effort**: M
- **Status**: Open

## DEBT-002: No chunk system
- **Added**: 2026-04-29
- **Severity**: High
- **Area**: world
- **What**: Entire 64x64 terrain spawns at startup -- no chunk loading/unloading
- **Why**: MVP scaffold, get something on screen first
- **Fix when**: Before adding more terrain features or props
- **Effort**: L
- **Status**: Resolved (2026-04-29) -- chunk system implemented in chunks.rs

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

## DEBT-005: Systems using let-else instead of Result return
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: player, camera
- **What**: move_player and follow_player use `let Ok(...) else { return }` instead of `-> Result`
- **Why**: Written before adopting the Bevy 0.16 fallible systems pattern
- **Fix when**: Next time these systems are touched
- **Effort**: S
- **Status**: Open

## DEBT-006: No test coverage
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: all
- **What**: Zero tests -- no unit or integration tests
- **Why**: MVP phase, systems are mostly wiring
- **Fix when**: Before systems get complex enough to have subtle bugs
- **Effort**: M
- **Status**: Open

## DEBT-007: Per-tile entities in chunks
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: world/terrain
- **What**: Each 16x16 chunk spawns 256 individual Cuboid entities instead of a single baked mesh
- **Why**: Simpler to implement, enables per-tile interaction (DEC-004)
- **Fix when**: If profiling shows entity count is the bottleneck (likely at render distance > 4)
- **Effort**: L
- **Status**: Open

## DEBT-008: Material/mesh duplication per chunk
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: world/terrain, world/props
- **What**: Each chunk creates its own materials and meshes instead of sharing cached handles
- **Why**: Quick implementation path
- **Fix when**: When optimizing memory/GPU usage
- **Effort**: M
- **Status**: Partially resolved -- props now use PropAssets struct with shared handles. Terrain still duplicates per chunk.

## DEBT-009: Fragile JSON save format
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: save
- **What**: Hand-written JSON serialization and string-based parsing. No schema validation.
- **Why**: Avoided serde dependency (DEC-008)
- **Fix when**: When save format grows beyond current 3 sections (player, inventory, buildings)
- **Effort**: M
- **Status**: Resolved (2026-04-29) -- migrated to serde + serde_json under DEC-011, save now uses registry save_keys with legacy migration shim

## DEBT-010: Crafting UI needs polish
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: ui
- **What**: User feedback that crafting menu "looks bad" and wants "more intricate" crafter
- **Why**: Built quickly with basic Bevy UI nodes
- **Fix when**: Next UI pass
- **Effort**: M
- **Status**: Resolved (2026-04-29) -- ported menu to Spiritfarer-inspired painted panel chrome (SVG-authored 9-slice PNG, Cinzel/Nunito fonts, gold border ornaments, flourish footer); added category tabs (Refining/Furniture/Building/Decor/Food); fixed row alignment and right-edge overflow

## DEBT-011: No game states
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: core
- **What**: No Loading/Menu/Playing/Paused state machine. Game goes straight to playing.
- **Why**: MVP focus
- **Fix when**: Before adding menus, pause screen, or loading screen
- **Effort**: M
- **Status**: Open

## DEBT-012: WorldNoise reconstructed every frame
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: world/biome, player, animals
- **What**: WorldNoise::new(seed) called in multiple systems every frame instead of being cached
- **Why**: Quick implementation, Perlin::new is cheap but still wasteful
- **Fix when**: When optimizing frame time
- **Effort**: S
- **Status**: Open
