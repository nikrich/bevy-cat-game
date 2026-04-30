# Architectural Decision Log

## DEC-001: Use Bevy over Unity/Godot
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need a game engine where AI can build everything end-to-end via code -- no GUI editor dependency
- **Decision**: Bevy (Rust) -- 100% code-driven, ECS architecture ideal for PCG, no editor required
- **Alternatives**: Unity (editor-dependent, .meta files fragile), Godot 4 (.tscn writable but editor-preferred), React Three Fiber (lower performance ceiling)
- **Consequences**: Steeper Rust learning curve, smaller ecosystem than Unity, but full code control and great performance

## DEC-002: Isometric orthographic camera
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Top-down 3D world crafting game needs a camera style
- **Decision**: Isometric angle (~45 degrees) with orthographic projection -- standard for peaceful crafting genre (Stardew Valley, Cult of the Lamb style)
- **Alternatives**: Fixed top-down 90 degrees (loses depth), perspective iso (parallax complexity), free camera (overwhelming for peaceful game)
- **Consequences**: Movement must be rotated to align with camera, UI placement is predictable, world feels consistent at any zoom

## DEC-003: Perlin noise stepped terrain
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need PCG terrain that matches low-poly art style
- **Decision**: Layered Perlin noise with height quantized to 0.25 steps, color-mapped by elevation (sand/dirt/grass)
- **Alternatives**: Wave Function Collapse (better for structured layouts), Voronoi (better for biome boundaries), flat grid (boring)
- **Consequences**: Natural-looking terrain with chunky aesthetic, easy to extend with biomes, need chunk system for infinite world

## DEC-004: Chunk-based terrain with per-tile entities
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need infinite terrain but spawning the whole world at startup is impossible. Must balance load/unload granularity with entity count and frame hitches (ref DEC-003)
- **Decision**: 16x16 tile chunks, render distance 3 (7x7 = 49 chunks visible), max 4 chunks loaded per frame to spread cost. Each tile is a separate entity (Cuboid) parented to the chunk entity
- **Alternatives**: Single-mesh-per-chunk (better GPU perf, harder to interact with individual tiles), larger chunks (fewer entities but bigger load spikes), ECS-less mesh generation (loses Bevy query benefits)
- **Consequences**: ~12,500 tile entities at steady state -- acceptable for now. Per-tile entities make future gathering/interaction easy (just query tiles). Can migrate to baked meshes later if profiling shows entity count is the bottleneck. Chunk children auto-despawn when chunk is despawned

## DEC-005: Noise-based prop placement
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Props (trees, rocks, flowers) need to feel natural and be deterministic (same seed = same world). Must be biome-aware and despawn with chunks
- **Decision**: Secondary Perlin noise layers (seeds 137, 251) control density and variety. Biome-aware rules: grass gets trees/bushes/flowers/mushrooms, dirt gets rocks/mushrooms, sand gets sparse rocks. Props spawn as children of chunk entities
- **Alternatives**: Random scatter with RNG per chunk (less natural clustering), Wave Function Collapse (overkill for prop placement), hand-placed templates (breaks PCG promise)
- **Consequences**: Deterministic placement -- revisiting an area always looks the same. Child-of-chunk parenting means automatic cleanup on unload. Density threshold (0.55) keeps ~15-20% of tiles decorated. Easy to tune per-biome by adjusting noise thresholds

## DEC-006: Day/night cycle timing and atmosphere
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need time-of-day to create atmosphere without time pressure. Must feel peaceful -- no "you must sleep or die" mechanics. Cycle length affects how often players experience transitions
- **Decision**: Full cycle in 12 real minutes (2 in-game hours per real minute). Start at 8am (pleasant morning). Six phases: dawn 5-7, morning 7-9, day 9-16, dusk 16-18, twilight 18-20, night 20-5. Night uses dim cool-blue moonlight rather than darkness
- **Alternatives**: Longer cycle (30+ min, transitions too rare to notice), real-time clock (locks players into their timezone's time), no night (misses atmosphere opportunity)
- **Consequences**: Players see a full dawn-to-dusk cycle in a short session. Night is still playable (moonlight, not blackout) -- reinforces peaceful identity. Sky color, sun position, and ambient light all transition smoothly. Speed is adjustable via WorldTime resource for future player control

## DEC-007: Unified input abstraction for KB+mouse and gamepad
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Game needs to support both keyboard+mouse and gamepad. Building placement needs mouse cursor-to-world raycasting. Multiple systems (player, crafting, gathering, building) all read input independently
- **Decision**: Single GameInput resource populated in PreUpdate from all input sources. All game systems read GameInput, never raw keyboard/mouse/gamepad. Cursor raycasts to Y=0 plane for world position
- **Alternatives**: leafwing-input-manager crate (adds dependency, heavier), per-system raw input reading (duplicated logic, hard to add gamepad)
- **Consequences**: Clean separation of input reading from game logic. Adding new input sources (touch, rebinding) only requires changes in input/mod.rs. Slight latency (1 frame) between input and action due to PreUpdate timing

## DEC-008: Manual JSON save format without serde
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need save/load for player position, inventory, and placed buildings. Adding serde + serde_json adds compile time and dependency weight
- **Decision**: Hand-written JSON serialization and simple string-based parsing. Save format is human-readable and editable
- **Alternatives**: serde + serde_json (automatic, more robust), bincode (compact, not human-readable), RON (Rust-native but still a dependency)
- **Consequences**: Fragile parser that needs manual updates when save format changes. Works for current scope but should migrate to serde if save format grows complex (more entity types, world state, etc.)

## DEC-010: Registry-based combinatorial item system
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Hardcoded ItemKind enum has 15 items. User wants thousands of items to support beautiful houses with furniture. Authoring thousands of variants by hand is infeasible
- **Decision**: Replace ItemKind enum with `ItemId(u32)` handles into an `ItemRegistry`. Items are generated combinatorially as `(Form, Material)` pairs. Forms are visual archetypes (Chair, Wall, Lamp, ...), Materials are palette/properties (Pine, Oak, Stone, ...). Recipes are templated against MaterialFamily so one "Chair" recipe yields Pine Chair / Oak Chair / Birch Chair from the player's choice of input
- **Alternatives**: Hand-authored item enum (doesn't scale), pure procedural generation (loses authorial control), tag-only system (less data-oriented)
- **Consequences**: Cross-cutting refactor across inventory, crafting, building, gathering, save, ui. Display names generated from registry. Mesh handles cached per Form so spawn cost stays flat. Enables phases 3/4/5 (browser UI, modular houses, dyes). Breaks DEC-008 -- save must move to serde (DEC-011)

## DEC-011: Migrate save format to serde
- **Date**: 2026-04-29
- **Status**: Accepted (supersedes DEC-008)
- **Context**: DEC-008 chose hand-written JSON to avoid the serde dependency. With registry-based items (DEC-010) the save format must serialize ItemId, RecipeId, building grid cells, and per-instance tints. Hand-rolling that is fragile
- **Decision**: Adopt `serde` + `serde_json` for save/load. Keep .json on disk for human-readability. Persist items by stable string key (Form_name + Material_name) rather than numeric ItemId so registry rebuilds across sessions remain compatible
- **Alternatives**: Keep manual parser (breaks under registry), bincode (compact but unreadable), RON (Rust-native but extra dep with no human-readable advantage over JSON)
- **Consequences**: One added dependency. ~50 lines of save.rs become a few derive macros. DEBT-009 closes when migration lands

## DEC-013: Bevy 0.18 + ecosystem crate stack (Phase 0)
- **Date**: 2026-04-29
- **Status**: Accepted (atmosphere clause amended 2026-04-29)
- **Context**: Building Phase 0 (foundations) on top of Bevy 0.16 with hand-rolled abstractions would force every later phase to refactor as it grew. The spec mandates the crates the wider Bevy ecosystem has standardised on
- **Decision**: Bump Bevy 0.16 → 0.18.1. Adopt the official ecosystem stack: `bevy_rapier3d` (physics), `bevy_tnua` (character controller), `leafwing-input-manager` (input), `bevy_egui` (immediate UI), `bevy_asset_loader` (asset gating), `moonshine-save` (reflection-based save/load). **Atmosphere/sky stays on the hand-tuned `daynight.rs` ClearColor gradient** — the Bevy 0.18 first-party procedural atmosphere was trialled and reverted because its Earth-scale lighting model (lux::RAW_SUNLIGHT, km-scale scenes, HDR + AcesFitted + Bloom) produced a visibly harsher look than the warm pastel low-poly palette the game is built on. `bevy_atmosphere` (the third-party crate) is also not adopted — stuck on 0.16 per the 2026-04-29 audit
- **Alternatives**: Stay on 0.16 (defers pain to Phase 1+). Skip individual crates (loses ecosystem integration). Adopt bevy_atmosphere (non-starter on 0.18). Tune the first-party atmosphere into a soft palette (possible but every dial fights the model's intent)
- **Consequences**: Phase 0 is one focused refactor in exchange for stable infrastructure across phases 1-7. Atmosphere is parked: the manual sky-color gradient stays, and a custom shader skybox is the right path if a future phase decides "no manual ClearColor" matters more than the current look. Breaks DEC-007 (input) and DEC-011 (save); both superseded below

## DEC-014: Game state machine (Loading / MainMenu / Playing / Paused, with Building sub-state)
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: No state machine meant the game booted straight into gameplay (DEBT-003 / DEBT-011). Pause, main menu, loading screens, and build-mode scoping all need a state to scope on/off entities and run conditions
- **Decision**: `GameState`: Loading → MainMenu → Playing → Paused. `BuildState`: Idle / Building, sub-state of Playing. Esc toggles Playing↔Paused; pausing freezes `Time<Virtual>` so every consumer of `Res<Time>` halts without per-system gating. Until W0.6 + W0.9 land (egui menu + asset_loader Loading screen) the Loading state immediately auto-transitions to Playing so the structure is in place without a real boot screen
- **Alternatives**: Tag every gameplay system with `in_state(Playing)` (works but fragile, easy to forget on new systems). Pause via `App::set_runner` swap (heavier, less idiomatic). Defer to a later phase (every later phase needs the partition)
- **Consequences**: Closes DEBT-003 and DEBT-011. The placeholder Loading→Playing transition is owned by `state.rs::bootstrap_into_playing` and goes away when W0.9 wires real asset gating. Esc-as-pause cohabits with Esc-as-cancel-build / Esc-as-close-craft — pause is suppressed when build mode or crafting menu is active

## DEC-015: Moonshine-save with reflection-based persistence
- **Date**: 2026-04-29
- **Status**: Accepted (supersedes DEC-011)
- **Context**: DEC-011 adopted serde + serde_json with hand-maintained SaveData structs. As Phase 0+ adds physics state, NPC state, role state, and festival state, hand-mirroring every component into a save struct doesn't scale
- **Decision**: Migrate to `moonshine-save` for save/load. Persistable components are tagged with `#[reflect(...)]` and the `Save` marker; loaded components are restored via reflection. On-disk format remains JSON for human readability and dirt-simple debug inspection
- **Alternatives**: Stay on hand-maintained SaveData (forces a sync pass every time a new persistable component lands). Bevy's built-in scene format (less control over what's persisted, more verbose on disk)
- **Consequences**: New persistable components need only `#[derive(Reflect)] #[reflect(Save)]` to participate in save/load — no save struct edits. Closes the old "save struct grew" pain. Future phases can add components to NPCs, buildings, etc. without touching save.rs

## DEC-016: 24-minute day cycle (supersedes DEC-006)
- **Date**: 2026-04-29
- **Status**: Accepted (supersedes DEC-006)
- **Context**: DEC-006 picked 12-min day. Per design call: 24 in-game hours = 24 real minutes (1 in-game hour per real minute) feels more like a session-friendly Stardew/Animal-Crossing pace than the previous 30-second hour
- **Decision**: `WorldTime.speed` drops from 2.0 to 1.0 (1 real minute = 1 in-game hour). Phase boundaries (dawn 5-7, morning 7-9, day 9-16, dusk 16-18, twilight 18-20, night 20-5) unchanged
- **Consequences**: Players see ~one full cycle per ~24-minute session instead of two. Transitions feel less hectic; a full day arc maps to a typical "evening of play". DEC-006 retired

## DEC-017: Vertex-height grid replaces per-tile cuboids (supersedes DEC-003 elevation rendering)
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Phase 1 deliverable. Per-tile cuboid terrain (DEC-004) couldn't host the brushes, auto-flatten, navmesh, and slope blending the spec calls for without retrofitting them onto a model that fights every operation. Vertex grids are the canonical shape for editable terrain
- **Decision**: Each chunk owns a 33×33 height array (32 cells = 33 verts, with the last row/column shared with the neighbour chunk). Per-vertex biome IDs ride alongside heights. The PCG samples `WorldNoise` per vertex with the existing `step_height` quantization, so the chunky low-poly look is preserved while the underlying representation is now a continuous height field. `Terrain::height_at(x, z)` returns the surface Y for a given world position, replacing the `step_height(noise.sample(...).elevation * height_scale()) * 0.5 + 0.3` formula scattered across props/animals/building
- **Alternatives**: Voxel grid (good for caves, overkill for top-down). Hand-painted heightmaps (breaks PCG promise). Keep cuboids and bolt brushes onto entity transforms (would multiply DEBT-007 by every brush)
- **Consequences**: Tile entities are gone. `Tile`/`WaterTile` components removed; `tile_tint` warm-cell visual parked under DEBT-018 because the per-Tile material-clone trick no longer applies. Closes DEBT-007 and DEBT-008 (terrain side)

## DEC-018: One stepped-block mesh + trimesh collider per chunk (supersedes DEC-004 entity model)
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: DEC-004 spawned 256 entities per chunk. With Phase 1's vertex grid in hand the cheaper option is one mesh per chunk. The first attempt used a smooth-grid mesh (verts shared between cells) plus a rapier heightfield collider — visually that turned the chunky stepped-tile aesthetic into rolling hills, which the user explicitly rejected on first playtest. The vertex grid is the right *data* model for brushes/save/navmesh; the *render* model needs to keep the cuboid look
- **Decision**: Each chunk renders as a stepped-block mesh — one flat top quad per cell at the cell's NW-corner vertex height, plus a vertical riser quad on each side where the neighbour cell is shorter (taller cell owns the riser). Verts are duplicated per quad so each tile reads as one solid block. Per-cell biome tint with a small position-derived shade variation, no smooth blending across cell boundaries. Collider is a rapier `Collider::trimesh` built from the same vertex/index buffers, so the cat physically walks on the same risers it sees. The trimesh + `RigidBody::Fixed` live on the chunk entity itself; mesh and collider are rebuilt by `regenerate_dirty_chunks`, capped at 4 dirty chunks per frame, with `try_insert` swallowing the unload-race. Chunk size grew from 16 to 32 cells; render distance dropped from 3 to 2 to keep loaded area roughly comparable
- **Alternatives**: Heightfield collider with smooth-grid mesh (the wrong aesthetic). Per-cell triangle colliders (collider count explosion). Keep per-tile entities and bolt a separate ground mesh on top (doubles the work). Smooth grid + post-process shader to fake step risers (more complex, fights the lighting). Hard-stepped tops without riser geometry (tiles would float with sky visible through the gaps)
- **Consequences**: ~2k–8k tris per chunk depending on terrain variation (vs 256 entities). Trimesh is heavier than heightfield but the geometry-physics match is exact — no visual/collision mismatch when the cat steps up risers. Vertex count per chunk roughly 3–4× the smooth-grid version because of the per-quad duplication, still tiny vs the old per-tile entity cost. W1.4's "smooth biome blending" acceptance criterion needs to be amended to "tile-aligned biome edges" in the spec. W1.3 (slope shader) becomes simpler: tops are always 0°, risers always 90°, no smoothstep blend zone needed

## DEC-019: Phase 1 closed with W1.4 atlas + W1.12/W1.13 navmesh deferred
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Phase 1 (`spec/phases/02-terrain.md`) shipped 13 of 15 work items: vertex-grid terrain (W1.1/W1.2/W1.5), all five brushes (W1.10), auto-flatten footprint API (W1.11), brush hotbar UI (W1.14), edit persistence including paint (W1.15), water/props/animals/gathering migrations (W1.6-W1.9), and a partial W1.4 (procedural noise tile texture). Slope/rock material override on risers (W1.3) was rejected per art direction (DEC-018). Two items remain unshipped: full per-biome texture atlas (W1.4 acceptance criterion: "tile-aligned biome edges per biome") and navmesh + override (W1.12 / W1.13)
- **Decision**: Close Phase 1 with the two unshipped items as accepted deferrals (DEBT-020 atlas, DEBT-021 navmesh). Move directly to Phase 2 (`spec/phases/03-build-feel.md`). Phase 2 hooks up the W1.11 `Terrain::flatten_rect` API as a real building-placement consumer — the natural next slice that compounds Phase 1 work into player-visible building feel
- **Alternatives**: (a) keep Phase 1 open and ship the atlas — pure cosmetic polish, no gameplay payoff before Phase 7. (b) keep Phase 1 open and ship the navmesh — heavy lift (1-2 sessions for `bevy_landmass` integration), no visible payoff until Phase 5 NPC cats. Both fail the cost-benefit test that DEC-013 / DEC-018 set: defer when the cost shape is wrong for the immediate phase, pair each deferral with a DEBT entry
- **Consequences**: Phase 1 status flips to Closed in memory. The foundation+follow-up memory file (`project_phase1_foundation.md`) is replaced by a `project_phase1_status.md` "closed" memory matching the Phase 0 pattern. Phase 5 will need a navmesh integration spike before NPC AI work; treat W1.12 as fresh research at that point because `bevy_landmass`'s tile model differs from the original `oxidized_navigation` assumption. Phase 7 (visual polish) is the natural home for the atlas if it doesn't surface earlier as a player-feedback issue

## DEC-009: Temperature/moisture biome classification
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need diverse biomes that feel natural and transition smoothly. Simple height-based coloring (sand/dirt/grass) was too uniform
- **Decision**: Two additional noise layers (temperature, moisture) at large scale drive biome classification. Temperature decreases with altitude (lapse rate). Mountains use ridged noise for sharp peaks. Rivers carved by domain-warped noise near zero-crossings
- **Alternatives**: Voronoi-based biome regions (hard edges), single noise axis (limited variety), hand-painted biome map (breaks infinite PCG)
- **Consequences**: 10 distinct biomes emerge naturally. Smooth transitions at boundaries. Deterministic from seed. Temperature-altitude coupling creates snow caps automatically

## DEC-020: Minecraft-style cube placement (supersedes Phase 2 W2.1-W2.4 snap algorithm)
- **Date**: 2026-04-30
- **Status**: Accepted (supersedes the snap-point approach planned in `spec/phases/03-build-feel.md` W2.1/W2.2/W2.3/W2.4)
- **Context**: Phase 2's spec called for a snap-point algorithm — each form publishes connection points (WallEnd, FloorEdge, etc.) and the build system finds compatible pairs across ghost and placed pieces, translating the ghost so they coincide. Implemented W2.1-W2.4 against that model, then playtested. The result was unintuitive: snap fired aggressively along the wall's length axis even when the cursor moved perpendicular, so dragging "north" near a wall facing east teleported the ghost east. Iso projection compounded the problem — the cursor's ground-plane projection lands a cell or two behind the visible piece it points at, so any "find pieces near cursor cell" logic missed the target. Multiple fix attempts (height-scaled search radius, yaw alignment, etc.) hit the same wall: the snap-point model fights iso cameras and hides what the ghost will actually do behind a coordinate transform the player can't see
- **Decision**: Drop the snap-point algorithm entirely. Replace with **Minecraft-style cube placement**: a rapier raycast from camera through cursor finds the first collider the camera ray hits (`cursor_hit` on `CursorState`), and `compute_placement` decides placement from the hit point + surface normal — top face → cube above, side face → cube adjacent in normal direction, terrain hit → cube on terrain at hit cell. Walls become true 1×1×1 cubes (`Cuboid::new(1.0, 1.0, 1.0)`, lift 0.5, height 1.0) so vertical stacking is uniform per click. Line tool stays for chains: anchor stores a wall's *center* Y from `compute_placement`, all walls in the segment share that Y; `segment_end` advances to the last placed cube so perpendicular cursor moves naturally extend from the corner cube's adjacent face (sharing the corner). Cursor IS just the column picker; ghost mesh shows exactly where the click will place
- **Alternatives**: (a) Keep grinding on the snap-point algorithm — every iteration revealed another iso/scoring edge case. (b) Implement a true cube-grid data model (`HashMap<IVec3, ItemId>` save format, integer cell coords end-to-end). Bigger refactor; the camera-direction raycast solves the placement problem without touching save/registry, so deferred. (c) Side-face / top-face heuristics from cursor.cursor_world distance (no rapier raycast). Wider search radii partially work but still fight the iso projection
- **Consequences**: Phase 2 W2.1 (Form snap data) and W2.2 (compatibility matrix) are deleted dead code as of this session. W2.3 (snap algorithm) and W2.4 (snap-validity tint) are superseded by `compute_placement` + the green/red ghost wash that already lived alongside. The Phase 2 spec needs to be rewritten — the snap-point pillar is replaced by the cube-placement pillar. Future cube-grid migration (proper integer cell coords + cube-aligned save) is the natural Stage 2 follow-up if this proves out under longer playtests. Stage 2 of Phase 2 (window/door insertion into walls + plank refund) drops cleanly into `compute_placement` as a special "Replace" placement style. New `Form::placement_style()` method routes between `Single` and `Line` (and `Replace` later) — replaces the hardcoded `matches!(form, Form::Wall)` checks that were sprouting across `update_preview` / `place_building`

## DEC-021: Decoration mode split from Build mode (Phase 2 stage 3)
- **Date**: 2026-04-30
- **Status**: Accepted
- **Context**: Phase 2 cube placement (DEC-020) shipped with a single `BuildMode` covering both structural placement (walls/floors/doors/windows/roofs) and the LowPoly Interior pack (~1000 decoration items). Playtesting surfaced two issues: (1) cube-grid snap is the wrong physics for decoration -- a chair forced into a 1m cell centre cannot be slid along the wall it should sit against; (2) one hotbar plus a 1000-thumbnail catalog mixed structural pieces and interior decor with no contextual UI cue, making mode-aware tools (Move, Place, Remove) impossible to thread through a single palette. The game's #1 pillar is build feel; decoration is half of build feel and was sharing physics with the wrong half
- **Decision**: Split `BuildMode` into two mutually exclusive modes. **Build** (cube-grid, structural pieces, Place/Remove tools, B-key, 6-swatch hotbar) keeps the cube placement model from DEC-020. **Decoration** (magnetic-continuous, Bed/Chair/Lantern/all interior pack, Place/Move/Remove tools, N-key, right-side catalog + recent quickbar) places pieces on attach surfaces (terrain, floor top, wall face, furniture top) with magnetic pull toward cell centres / wall mid-points / neighbour edges; `Alt` releases the magnet for free continuous. Rotation in decoration is 15-degree steps with `Alt` for continuous. First shippable version uses a fine 0.1m grid in place of full magnetic snap; magnet anchors land in a polish pass. Code reorganises into `src/building/` (cube-grid only, slim), new `src/decoration/` (magnetic placement, Move tool, catalog UI), and new `src/edit/` shared infra (`EditHistory` renamed from `BuildHistory`; `PlacedItem` renamed from `PlacedBuilding`; `HighlightPlugin` for hover/held tints). Single shared undo / redo stack across both modes
- **Alternatives**: (a) Generic placement framework with a `PlacementProfile` trait and the two modes as profiles -- the abstraction lies because Build and Decoration share almost nothing in their happy paths; the trait is a switch statement in costume. (b) Decoration as a layer on top of building, importing building's primitives -- dependency arrow points the wrong way; building's internals leak into decoration. (c) Stay with one mode, add Move/magnetic as flags on `BuildTool` -- doesn't address the 1000-thumbnail catalog mixing structural and decor items, and the tool palette grows beyond what fits a hotbar. (d) Fine-grid (0.1m) for decoration without ever building anchor magnets -- viable; treated here as v1, with magnetic v2 as a polish target
- **Consequences**: Decoration becomes the cozy creative-flow tool the spec's #1 pillar always wanted. Build mode shrinks to its actual job (structural skeleton). The `building/mod.rs` 2,111-line file gets carved roughly in thirds (build proper, decoration, shared edit infra) so each module reads top-to-bottom. Move tool eliminates the "Remove + Place" two-step that decoration churns through today. Single shared undo/redo stack means Ctrl+Z works across mode switches. Save format is unaffected -- both modes spawn the same `PlacedItem` + `Transform`, and the existing `INFINITE_RESOURCES` cheat covers both modes' inventory checks. Phase 2 spec rewrites again -- this time the "build feel" pillar is split into Stage 1 (cube placement, shipped) and Stage 3 (decoration mode, this ADR). See `docs/superpowers/specs/2026-04-30-decoration-mode-split-design.md`
