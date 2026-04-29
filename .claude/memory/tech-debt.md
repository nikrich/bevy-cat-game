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
- **Status**: Resolved (2026-04-29) -- Phase 1 foundation slice (DEC-017 / DEC-018) replaced per-tile cuboids with one triangle mesh + one heightfield collider per chunk. Chunk size went 16 -> 32 cells, render distance 3 -> 2

## DEBT-008: Material/mesh duplication per chunk
- **Added**: 2026-04-29
- **Severity**: Medium
- **Area**: world/terrain, world/props
- **What**: Each chunk creates its own materials and meshes instead of sharing cached handles
- **Why**: Quick implementation path
- **Fix when**: When optimizing memory/GPU usage
- **Effort**: M
- **Status**: Resolved (2026-04-29) -- Phase 1 foundation slice. Terrain now shares one `TerrainMaterial` resource across every chunk (per-vertex colors carry biome tint). Water shares one `WaterAssets` resource (one mesh + one material). Props were already on shared `PropAssets`

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

## DEBT-017: AppExit deadlocks under rapier + egui
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: state
- **What**: `MessageWriter<AppExit>` from the egui Quit buttons closes the window but the process never exits — some thread/resource teardown order in Bevy 0.18 + bevy_rapier3d 0.33 + bevy_egui 0.39 deadlocks. As a workaround, `state::quit_now()` calls `std::process::exit(0)` directly. The save loop already auto-saves every 30s and on F5, so the worst case is losing 30s of unsaved play
- **Why**: Identified during W0.3 + W0.4 playtest. Hand-shutting the process down with `std::process::exit` skips Drop on every resource in the world — fine for our small game, would be a problem if any subsystem needs to flush state on exit (none currently do)
- **Fix when**: Bevy / rapier / egui versions move forward — bisect to find which one's teardown is the culprit. Or convert Quit-from-pause into "auto-save + return to MainMenu" and reserve the actual process exit for window-close, where Bevy seems to handle it cleanly
- **Effort**: M (bisect upstream) or S (UX-flow rework)
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

## DEBT-016: Player movement physics (W0.3 rapier + W0.4 tnua)
- **Added**: 2026-04-29
- **Severity**: -
- **Area**: player, world/terrain, world/props
- **What**: Originally tracked the deferral of W0.3 (`bevy_rapier3d`) and W0.4 (`bevy_tnua`); both shipped in a follow-up session the same day
- **Status**: Resolved (2026-04-29) -- rapier 0.33 + tnua 0.31 + tnua-rapier3d 0.16 wired up. Per-tile cuboid colliders on terrain (intentionally throwaway, Phase 1 swaps for the vertex-height trimesh). Capsule rigid body + LockedAxes::ROTATION_LOCKED + TnuaController on the player. `TnuaBuiltinJump` action bound to `Action::Jump` with height=1.6 (clears one beach step + a stump). `snap_to_terrain` removed; gravity + the float spring drive Y. `pose_player` still scales the visual capsule for the verb cues. Wading depth tuned (`floor_y = step_height(SEA_LEVEL) * 0.5 - 1.05`) so the cat half-submerges in water mesh. Hand-rolled `push_player_out_of_walls` retired in favour of rapier-resolved wall colliders attached in `building::collision::attach_for_form`. Known follow-ups: tall trees act as climb-blocking walls (the float spring's cling_distance is intentionally short to avoid floating); water-tile collider race exposed by physics-driven chunk churn — `init_water_ripples` now uses `try_insert` to swallow despawn-races. The Quit button hard-exits via `std::process::exit(0)` because Bevy 0.18 + rapier + egui deadlock on `AppExit` shutdown — see DEBT-017

## DEBT-018: Warm-cell tile tint disabled by terrain rewrite
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: memory/tile_tint
- **What**: The Phase C "warm tile glow" visual is parked. The original `tile_tint` system queried `Tile` entities and cloned their `StandardMaterial` so it could fade an emissive amber tint while a cell was warm. Phase 1 (DEC-017 / DEC-018) replaced per-tile entities with one mesh per chunk, so the per-cell material trick no longer applies. `WorldMemory.warmth` itself still ticks (track_player_cell + decay_warmth) so verbs/journal/save are unaffected — only the visible glow is missing
- **Why**: Reimplementing the visual on the new mesh needs either a per-vertex emissive attribute mutated on warm-cell change (and a regen on every change) or a small overlay decal/light spawned per warm cell. Neither belongs in the Phase 1 foundation slice
- **Fix when**: Phase 1 follow-up after the slope shader (W1.3) lands — the same custom material extension is the natural home for an emissive overlay. Or, if warmth-as-glow turns out to be rare, spawn a small unlit decal/quad at the warm cell instead of touching the chunk material
- **Effort**: M
- **Status**: Open

## DEBT-019: Per-tile water swell parked by water-mesh-per-chunk migration
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: world/water
- **What**: The previous water "wave" visual was a per-tile Y modulation in `update_water_ripples`. With Phase 1's water-mesh-per-chunk (W1.6) the water is one flat plane per chunk, so the per-tile bobbing doesn't apply. The plane sits at sea level with no animation
- **Why**: Re-implementing the swell on a single mesh requires either per-frame mesh-vertex mutation (allocates) or a real water shader (significant work). Both belong in a later Phase 1 slice or Phase 7 polish
- **Fix when**: When a phase needs the "alive water" feel back, or as part of Phase 7 visual polish. Cheapest route is a custom material that displaces verts in the vertex stage from the same `wave_height` function
- **Effort**: M
- **Status**: Open

## DEBT-020: W1.4 per-biome top-face texture atlas
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: world/terrain, render
- **What**: W1.4's full vision — one texture tile *per biome* on cell tops (so Forest reads as leaf litter, Desert as sand grain, Snow as crystals, etc.) — is not shipped. The current `TerrainMaterial.base_color_texture` is one shared procedural noise tile that multiplies with the per-vertex biome tint, which gets us most of the visual lift but doesn't differentiate biomes by texture, only by tint
- **Why**: A real atlas needs (1) a texture asset per biome (or one combined atlas with per-cell UV offset), (2) a custom material extension to sample the right tile per cell, and (3) the spec amendment to W1.4's "smooth blending across vertex boundaries" wording — the cuboid topology means tile-aligned biome edges, not gradient blending (DEC-018). The procedural-noise version was the cheapest visible win; a full atlas is polish that doesn't gate any other phase
- **Fix when**: Phase 7 polish, or earlier if biome distinguishability becomes a player-feedback issue. Lowest cost-benefit win is per-biome procedural noise tiles (different scales/octaves per biome) rather than authored art
- **Effort**: M
- **Status**: Open

## DEBT-021: W1.12 / W1.13 navmesh + override deferred to Phase 5
- **Added**: 2026-04-29
- **Severity**: Low
- **Area**: world/navmesh
- **What**: `bevy_landmass` integration (W1.12) and the `NavmeshOverride` data path for stairs/cart paths (W1.13) are not implemented. Without a navmesh, NPC AI is limited to the wander-and-flee pattern the existing animal system uses
- **Why**: Phase 1's player-facing payoff doesn't depend on a navmesh. Phase 5 is when NPC cats need to actually pathfind around terrain — that's the right cost-benefit moment, especially since `bevy_landmass`'s API surface differs from the spec-assumed `oxidized_navigation` and budget is non-trivial (1-2 sessions for the integration spike per the original DEC-021 risk note)
- **Fix when**: Start of Phase 5 (NPC cats). Path will need updating because `bevy_landmass` has a different tile model than what DEC-021 / W1.12 originally assumed; treat the integration as fresh research at that point. Fallbacks if it doesn't fit: `vleue_navigator` or a hand-rolled grid navmesh on the existing 32×32 cell grid
- **Effort**: L (1-2 sessions)
- **Status**: Open

## DEBT-013: Phase 0 pending crate adoptions — superseded
- **Added**: 2026-04-29
- **Severity**: -
- **Area**: -
- **What**: Originally tracked all six pending crate adoptions. Most landed this session: leafwing (W0.5), egui (W0.6), asset_loader (W0.9). Atmosphere (W0.7) is its own DEBT-014. moonshine-save (W0.11) is its own DEBT-015. Rapier + tnua (W0.3 + W0.4) is its own DEBT-016
- **Status**: Resolved (2026-04-29) — split into per-area debts; nothing left under this catch-all
