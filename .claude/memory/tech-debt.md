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
- **Status**: Resolved (2026-04-29) -- W0.10 / DEC-014 introduced GameState (Loading/MainMenu/Playing/Paused) with Building as a sub-state of Playing

## DEBT-004: Hardcoded world seed
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: world
- **What**: Perlin seed is hardcoded to 42
- **Why**: MVP scaffold
- **Fix when**: When adding save/load or world selection
- **Effort**: S
- **Status**: Resolved (2026-04-29) -- W0.13 added `seed: u32` to SaveData; ChunkManager.seed is restored from the save on load. The "new world dialog" UI itself is deferred to W0.6 (egui main menu)

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
- **Status**: Resolved (2026-04-29) -- W0.10 / DEC-014 introduced GameState. Esc pauses, sub-state Building exists for build-mode scoping

## DEBT-012: WorldNoise reconstructed every frame
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: world/biome, player, animals
- **What**: WorldNoise::new(seed) called in multiple systems every frame instead of being cached
- **Why**: Quick implementation, Perlin::new is cheap but still wasteful
- **Fix when**: When optimizing frame time
- **Effort**: S
- **Status**: Resolved (2026-04-29) -- W0.2 made WorldNoise a `Resource` built once via `FromWorld` from `ChunkManager.seed`. All per-frame consumers now take `Res<WorldNoise>`

## DEBT-015: Save format still hand-maintained (moonshine-save deferred)
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: save
- **What**: W0.11 (replace save.rs with moonshine-save) was tried and deferred. The current serde-JSON save in `save.rs` works and was extended in this same session for OS-aware paths (W0.12) and seed persistence (W0.13). Moonshine's main payoff is "new persistable components auto-serialize via reflection" — useful when many entities carry state, marginal when state is dominated by a few resources (Inventory/WorldMemory/Journal/ChunkManager.seed) and a small number of building entities
- **Why**: Two extra costs surfaced. (1) Inventory/recipes use `ItemId` indices into a registry that rebuilds each session, so the save must persist by stable `save_key` strings — moonshine's reflection serializer would need a custom mapper, which is the bulk of what `save.rs` already does. (2) moonshine-save's on-disk format is RON, not JSON; landing it would amend DEC-015 (which committed to JSON for human-readability)
- **Fix when**: Start of Phase 5 (NPC cats). Phase 5 adds ~10 persistable components per NPC and several archetypes — that's where the "new component → just `#[reflect(Save)]`" payoff actually starts compounding
- **Effort**: M
- **Status**: Open

## DEBT-014: Procedural atmosphere art-direction mismatch
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: world/daynight, render
- **What**: The Bevy 0.18 first-party procedural atmosphere was trialled per W0.7 and reverted — its physically-based Earth-scale model (lux::RAW_SUNLIGHT illuminance, km-scale geometry, HDR + AcesFitted tonemapping + Bloom) produces a harsher, more contrasty look than the warm pastel low-poly palette the game is built on. Sky still uses the hand-tuned 6-phase ClearColor gradient in `daynight::update_sky_color`
- **Why**: Spec assumed the first-party atmosphere would be a clean drop-in. Visually it isn't, and tuning it into a soft palette fights every dial in the model
- **Fix when**: When polishing visuals (Phase 7) or when a phase introduces something the manual sky can't represent. Options: custom shader skybox that preserves the gradient look; hold the line and treat the manual gradient as the canonical look (amend the spec)
- **Effort**: M (custom shader skybox) or S (spec amendment)
- **Status**: Open

## DEBT-013: Phase 0 pending crate adoptions (W0.3, W0.4, W0.5, W0.6, W0.9, W0.11)
- **Added**: 2026-04-29
- **Severity**: High
- **Area**: core
- **What**: Six of the 14 work items in Phase 0 ship infrastructure crates that each touch many files: `bevy_rapier3d` (W0.3), `bevy_tnua` (W0.4), `leafwing-input-manager` (W0.5), `bevy_egui` (W0.6), `bevy_asset_loader` (W0.9), and `moonshine-save` (W0.11). They were not landed in the Bevy 0.18 upgrade session because each warrants its own focused integration pass with playtesting
- **Why**: Bundling them into the same session as the engine bump risked subtle regressions in player movement, input, UI, and save -- areas the user actively plays. The eight smaller items (W0.1, W0.2, W0.7, W0.8, W0.10, W0.12, W0.13, W0.14) shipped together because they are mostly mechanical and verifiable in one pass
- **Fix when**: Before any Phase 1+ work begins -- Phase 0 is a hard prerequisite for the rest of the spec. Recommended sequence: W0.5 (leafwing) → W0.3 + W0.4 (rapier + tnua paired) → W0.9 + W0.6 (asset_loader and egui together for the loading screen) → W0.11 (moonshine-save)
- **Effort**: L per item; ~3-4 focused sessions total
- **Status**: Open
