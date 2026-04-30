# Session Journal

## 2026-04-30 -- Phase 2 build pivot: snap algorithm scrapped, Minecraft cube placement shipped

**What shipped:** Phase 2 W2.1/W2.2/W2.3/W2.4 went in as the snap-point algorithm per spec, then got fully replaced over the same session with **Minecraft-style cube placement** (DEC-020). Walls are now true 1×1×1 cubes (`Cuboid::new(1.0, 1.0, 1.0)`, lift 0.5, height 1.0). Cursor uses a rapier raycast against colliders (`CursorState.cursor_hit`), and `compute_placement` decides target cell from hit point + normal — top face stacks, side face places adjacent, terrain hit places on terrain. Line tool keeps the chain UX: anchor.y carries the wall's center Y, `segment_end` advances to the last placed cube so perpendicular cursor moves trace L-bends sharing the corner cube. New `Form::placement_style()` returns `Single | Line` and replaces the hardcoded `matches!(form, Form::Wall)` routing checks.

**What got deleted in cleanup pass:**
- Dead Phase 2 W2.1/W2.2 metadata in `src/items/form.rs`: `SnapPoint`, `SnapKind`, `Category`, `snap_points()`, `cozy_value()`, `build_time_secs()`, plus ~150 lines of const SnapPoint tables. Zero call sites once the snap algorithm was scrapped.
- `WallCollision` component + `push_player_out_of_walls` system in `src/building/collision.rs`. Hand-rolled penetration system was unregistered ages ago in favour of rapier; cube migration confirmed the rapier path works, so the dead code is gone.

**Lessons worth carrying forward:**
- **Iso-camera projection breaks any "find pieces near cursor cell" heuristic.** When the cursor visually points at a wall's top, its ground-plane projection lands a cell or two behind the wall. Heuristic search radii partially compensate but always leave bug surface. The right fix is a **camera-direction rapier raycast** that returns the exact collider the camera ray hits — no iso math, no guessing.
- **Don't conflate "cursor position" with "placement target".** Cursor picks the column; the *ghost mesh* is the placement target. Once we made the ghost auto-rise to the column top (anchor + per-cell stacking), the player only needs to read the ghost — they no longer have to mentally project the cursor onto the right surface. This UX framing came from the user, not the spec.
- **A spec is a hypothesis, not a contract.** The W2.1-W2.4 snap-point pillar was the spec's #1 build-feel idea. Multiple iterations confirmed it fights iso projection and hides ghost behaviour from the player. Throwing it out and pivoting to cube placement was the right call. Leave the spec, write the new direction in `decisions.md` (DEC-020), keep moving.
- **Inventory cheats during build-tool playtest.** `INFINITE_RESOURCES = true` const + F2 hotkey to top up. Lets the focus stay on the placement model instead of the meta-loop of crafting / refilling. Flip to `false` before shipping.

**Open threads (next session):**
- Stage 2 of Phase 2: window/door insertion into walls with plank refund. Drops cleanly into `compute_placement` as a new `PlacementStyle::Replace` variant — when ghost is a window/door and the click hits a wall, despawn the wall and spawn the new piece at the same transform; refund the wall's plank cost.
- Walls in line tool don't always line up at corners (user noted, deferred). Likely a 1-cell off-by-one in the corner-sharing logic; revisit when corner pieces become unsightly enough.
- Save format still serializes raw `Transform` per `PlacedBuilding`. A `HashMap<IVec3, ItemId>` cube-cell save is the natural next refactor once the cube model is fully proven out — not urgent.
- Spec `spec/phases/03-build-feel.md` is stale. The snap-point pillar is dead; the spec needs a rewrite around cube placement before Phase 2 can be "closed" in the Phase 0 / Phase 1 pattern.

## 2026-04-29 -- Phase 1 closed (13 shipped + 2 accepted deferrals)

**Closure:** Phase 1 (`spec/phases/02-terrain.md`) shipped 13 of 15 work items: vertex-grid terrain (W1.1/W1.2/W1.5), all five brushes (W1.10), auto-flatten API (W1.11), brush hotbar UI (W1.14), edit persistence including biome paint (W1.15), water/props/animals/gathering migrations (W1.6-W1.9), partial W1.4 (procedural noise tile), plus three bonus follow-ups not in spec (props snap with terrain, paint-driven prop respawn, pop animation on painted props). W1.3 was rejected per art direction (DEC-018). W1.4 full atlas and W1.12/W1.13 navmesh are deferred (DEC-019).

**Accepted deferrals:**
- W1.4 full per-biome texture atlas → DEBT-020. Procedural noise tile gets us most of the visual lift; per-biome differentiation is cosmetic polish that doesn't gate any phase. Phase 7, or earlier if biome distinguishability becomes a player-feedback issue
- W1.12 / W1.13 `bevy_landmass` navmesh + `NavmeshOverride` → DEBT-021. Heavy lift, no visible payoff until Phase 5 NPC cats. Treat as fresh research when picked up — `bevy_landmass` API differs from the original `oxidized_navigation` spec assumption

**Decisions recorded:** DEC-019 (Phase 1 closed; W1.4 atlas + navmesh deferred). DEC-017 / DEC-018 already on the books from earlier in the phase.

**Tech debt closed:** DEBT-007 (per-tile entities), DEBT-008 (material/mesh duplication per chunk).

**New tech debt:** DEBT-018, DEBT-019, DEBT-020, DEBT-021.

**Next phase:** Phase 2 build-feel (`spec/phases/03-build-feel.md`). The natural first slice is wiring `Terrain::flatten_rect` (W1.11 API, currently on the debug `F` hotkey) into actual building placement so a placed piece auto-flattens its footprint.

## 2026-04-29 -- Phase 1 continued: props snap with terrain, textured ground, paint-driven prop respawn with pop animation

**What was done:**
- Props move with the terrain. Added `PropTerrainAnchor { terrain_y: f32 }` on every spawned prop (records the surface Y at spawn). New `snap_props_to_terrain` system fires on `Changed<Mesh3d>` of chunk entities, computes `delta = new_terrain_y - anchor.terrain_y`, applies the delta to `Transform.y` AND `PropCollision.top_y`. Result: Raise/Lower/Smooth/Flatten brushes now also move trees, rocks, mushrooms etc. with the ground instead of leaving them floating or buried. Per-kind visual lifts (e.g. `+0.05` for DeadBush, `+0.1` for IceRock) stay implicit in `Transform.y` because we work in pure deltas
- W1.4 partial: procedural per-cell noise texture on `TerrainMaterial.base_color_texture`. 128×128 grayscale tile-friendly noise (4D Perlin sampled on a torus, two octaves) multiplies with the per-vertex biome tint, so each cell reads as "tinted textured ground" instead of flat color. Range clamped to [0.78, 1.0] so the biome tint stays dominant. Risers reuse the same UVs and the noise reads as natural rocky-face dapples on the vertical surfaces. The full per-biome atlas (one tile per biome on the top face) is still pending
- Paint-driven prop respawn. `PropAssets` was promoted to a `Resource` (built once via `FromWorld`) so both the initial chunk-load spawn and the new respawn path share the same prefab handles. New `PropCell { cx, cz, lx, lz }` component on every prop. New `Terrain.painted_cells: HashMap<ChunkCoord, HashSet<(u8,u8)>>` transient overlay (NOT persisted) populated by `set_vertex_biome` and drained each frame by `respawn_props_for_painted_cells`. That system finds existing props in painted cells via the `PropCell` component, despawns them (Bevy 0.18 `commands.despawn()` is recursive — children come down with the root), then runs `try_spawn_cell_prop` for the painted biome. Density check + variety hash are deterministic per (wx, wz), so painting Forest onto Desert produces a tree at the same world position the cell would have had if it had been Forest from PCG
- Pop animation. New `PropSpawnPop` component (default 0.35s duration). `animate_prop_spawn_pop` snapshots `Transform.scale` on the first tick and animates the prop from `POP_SCALE_FLOOR * base_scale` to `base_scale` via `ease_out_back` (~10% overshoot). Initial chunk-load spawns are NOT tagged, so only paint-driven appearances pop in. Leaf spawners (`spawn_cactus`, `spawn_flower`, `spawn_dead_bush`, `spawn_ice_rock`, `try_spawn_kenney_prop`) all return `Entity` now so the respawn system can tag them
- Persistence fix. `spawn_chunk_props` now reads biome from `terrain.vertex_biome(wx, wz)` (which has `biome_edits` re-applied on chunk load) instead of raw PCG. Painted cells now persist their props across save/quit/reload — previously the durable `biome_edits` overlay restored vertex colours but `spawn_chunk_props` ignored it and re-spawned PCG props for the painted cells

**Crash bugs found and fixed mid-implementation:**
- parry BVH crash mid-paint (`bvh_binned_build.rs:58`, "len is 8 but index is 8"). Root cause: Paint originally added the chunk to `Terrain.dirty`, so `regenerate_dirty_chunks` re-handed rapier an *identical* trimesh `Collider` every paint tick (Paint changes biomes, not heights — geometry is unchanged). The rapid identical-trimesh re-insertion crashed parry's BVH binned builder when a step_simulation was mid-traversal at the moment the collider component swapped. Fix: split chunk dirtiness into `Terrain.dirty` (geometry) vs `Terrain.color_dirty` (biome only). Biome paint puts the chunk in `color_dirty`; the regen system rebuilds the mesh only and leaves the existing collider component untouched. Geometry-dirty chunks are processed first in the same system; a chunk in both sets is skipped from the color pass (the geometry pass already refreshed colours)
- Same parry crash from pop animation. Rocks/boulders/mushrooms have a *child* rapier cuboid collider that inherits the parent's `GlobalTransform`. Scaling the parent to `Vec3::ZERO` collapses the child collider's AABB and crashes the same BVH builder. Fix: clamp the pop scale floor to `POP_SCALE_FLOOR = 0.001`. Visually indistinguishable from zero at typical prop sizes, keeps the AABB non-degenerate

**Files created:** —

**Files modified:** `src/world/biome.rs` (already had Serialize/Deserialize), `src/world/terrain.rs` (`color_dirty` set + 2-pass regen, procedural noise texture on `TerrainMaterial`, painted_cells field), `src/world/props.rs` (full pass — `PropTerrainAnchor`, `PropCell`, `PropSpawnPop` components; `PropAssets` to `Resource`; `try_spawn_cell_prop` helper; `respawn_props_for_painted_cells` + `snap_props_to_terrain` + `animate_prop_spawn_pop` systems; leaf spawners return `Entity`; `spawn_chunk_props` reads biome from `terrain.vertex_biome`), `src/world/mod.rs` (registers the three new systems in the chunk-lifecycle chain after `spawn_chunk_props`, plus `init_resource::<PropAssets>`)

**Surprising things:**
- The "ease-out-back from 0" animation can't actually start at 0 if the entity has a child collider — it has to start at a tiny non-zero floor (we used 0.001) or parry crashes the same way the trimesh churn did. Useful to remember if any future system mass-spawns entities with hierarchical colliders
- The collider-churn crash showed up as "index out of bounds: the len is 8 but the index is 8" in parry's binned BVH builder — that "8" smelled like SIMD bin count overrun, and the trigger was "rapidly replacing the collider component while step_simulation was mid-traversal." The fix was structural (don't replace identical colliders), not "add bounds checks." Worth remembering: when parry panics with a small fixed-size index OOB, suspect concurrent modification of the collider, not bad geometry
- `spawn_chunk_props` reading biome from PCG instead of from the chunk's vertex grid was a silent persistence bug — it didn't crash, didn't warn, just looked like "paint doesn't persist" even though `biome_edits` was being saved and restored correctly. The vertex-grid biome IS the painted overlay (because `generate_chunk` re-applies `biome_edits` after PCG fills the grid). Anything reading "what biome lives here" should always go through `terrain.vertex_biome` / `terrain.biome_at`, never `noise.sample().biome` — DEC-018 implies this but it wasn't explicit
- `Terrain` now has three sets that look similar but mean different things: `dirty` (geometry-changed chunks), `color_dirty` (biome-changed chunks, no geometry change), `painted_cells` (cells whose biome was painted, for prop respawn). All three are populated by the same `set_vertex_biome` write, except `dirty` (height edits only). The doc comment on the `Terrain` resource now spells this out

**Open threads:**
- W1.4 full per-biome atlas (one image per biome on the top face) — the procedural noise above gets us most of the way; an atlas would let each biome look more distinct (different patterns for forest leaves vs sand vs snow). Lower priority now that the texture exists
- W1.12 / W1.13 navmesh + override — still parked. Phase 5 NPC cats are when the cost-benefit flips
- Spec amendment to `spec/phases/02-terrain.md` W1.4 wording ("smooth blending across vertex boundaries" → "tile-aligned biome edges") still pending in the doc

## 2026-04-29 -- Phase 1 continued: Smooth + Paint brushes, biome paint persistence, brush hotbar UI

**What was done:**
- W1.10 finished (Smooth + Paint). `BrushTool` enum extended with `Smooth` and `Paint`; `Hotbar4` selects Smooth, `Hotbar5` selects Paint. Smooth lerps each vertex toward the average of its 4 cardinal neighbours, snapshotting heights inside the brush bounds (+1 ring) at the start of each tick so iteration order doesn't feed half-updated values back into later neighbour reads. Paint is binary inside a >0.5-falloff gate (biome ids don't lerp); cycles through the 9 paintable biomes (Ocean excluded — painting water onto land doesn't lower the height to the wading floor and the per-chunk water plane only covers PCG-water cells). `[` / `]` cycle the paint biome while Paint is active. Gizmo ring tints: blue for Smooth, magenta for Paint
- W1.10 biome paint persistence: `Terrain` gains a parallel `biome_edits: HashMap<ChunkCoord, HashMap<(u8,u8), Biome>>` overlay alongside the existing height edits overlay. `set_vertex_biome` records the write and marks the *single* owning chunk dirty (biomes only feed the cell whose NW corner is the vertex — they don't cross into neighbour-cell risers like heights do, so the 5-cell dirty fan-out from `set_vertex_height` is unnecessary). `generate_chunk` re-applies biome edits after PCG, mirroring the height path. Save format extends with `biome_edits: Vec<ChunkBiomeEditsSave>` (flat list because JSON map keys must be strings), `#[serde(default)]` so older saves load. `Biome` gained `Serialize, Deserialize` derives
- W1.14 brush hotbar UI. New `src/world/edit_egui.rs` registers an egui panel anchored to bottom-centre while edit mode is active. Lists the 5 brushes with their hotkey numbers, highlights the active one in gold, displays current radius, and (when Paint is selected) the active biome name + the `[` / `]` hint. Style matches the crafting menu (parchment + gold). Hidden when edit mode is off so it doesn't fight the inventory hotbar

**Deferred / not started in this session:**
- W1.4 vertex tint atlas (one texture tile per biome on the top face) — still pending; per-cell vertex tint with shade variation already works
- W1.12 / W1.13 `bevy_landmass` navmesh + override — still parked. Phase 5 NPC cats are when the cost-benefit flips
- Paint-driven prop respawn — when a vertex's biome changes, the props in that cell should also update (e.g. paint Forest onto Desert and trees should appear next time the chunk regens). Right now Paint only changes the surface tint; props were placed on chunk load and stay where they are. Logged for later
- Spec amendment to `spec/phases/02-terrain.md` W1.4 wording ("smooth blending across vertex boundaries" → "tile-aligned biome edges") still pending in the doc

**Files created:** `src/world/edit_egui.rs`

**Files modified:** `src/world/biome.rs` (Serialize/Deserialize on `Biome`), `src/world/terrain.rs` (`biome_edits` overlay, `set_vertex_biome`, `vertex_biome`, `generate_chunk` re-apply), `src/world/edit.rs` (Smooth + Paint brushes, `[`/`]` biome cycle, snapshotted reads for Smooth, gizmo tints), `src/world/mod.rs` (register `edit_egui`), `src/save.rs` (`biome_edits` round-trip)

**Surprising things:**
- Paint with smoothstep falloff would have produced visible dot patterns at the radius edge — biome ids are discrete, so a "70% painted" vertex looks identical to a "100% painted" one and the falloff just becomes a noisy boundary. A binary gate at falloff > 0.5 gives a clean rounded footprint
- Smooth needs the snapshot pass not because of intra-frame ordering (the iteration is single-threaded) but because the brush is *iterative*: each tick should converge a little, and feeding a half-updated row's value into the next row's read accelerates the smear directionally rather than smoothing isotropically
- The `set_vertex_biome` 1-chunk dirty fan-out vs `set_vertex_height`'s 5-chunk fan-out tripped me up briefly. The reason: a vertex's height feeds the cell at its NW corner (top quad) AND the four neighbour cells' risers, because risers compare against the neighbour's NW height. A vertex's biome only feeds the NW-corner cell's *colour*, since risers are coloured by their owner cell, not their neighbour. So biome edits never cross cells

## 2026-04-29 -- Phase 1 underway: vertex grid + cuboid mesh + brushes + persistence + auto-flatten

**What was done:**
- W1.1 / W1.2 / W1.5 / W1.6 / W1.7 / W1.8 / W1.9 — foundation slice. Replaced the per-tile cuboid entity model with a 33×33 vertex height grid per chunk, rendered as a stepped-block mesh (one flat top quad per cell + vertical riser quads on each side where the neighbour is shorter, taller cell owns the riser), backed by a rapier `Collider::trimesh` built from the same vert/index buffers. Per-cell biome tint with a small position-derived shade variation. Chunk size 16 → 32 cells, render distance 3 → 2. Closes DEBT-007 + DEBT-008. DEC-017 + DEC-018 record the data-vs-render split (vertex grid is the source of truth for brushes/save/navmesh; mesh builder emits cuboid topology to keep the chunky look)
- Migrations: `props`, `animals`, `building` preview, `water`, `tile_tint` all read `Terrain::height_at_or_sample` (or are stubbed under debt). Water became one alpha-blended plane per chunk at sea level; ocean cells get pulled to a wading floor (`WATER_FLOOR_Y = -1.30`) so the cat physically sinks in. `tile_tint` parked under DEBT-018 (per-cell warmth glow needs vertex-color reimpl); per-tile water swell parked under DEBT-019
- W1.10 (partial) terrain brushes: `T` toggles edit mode (gated against build mode + crafting), `1/2/3` select Raise / Lower / Flatten, LMB-held paints at the cursor on a 100 ms tick (one 0.25 m step per tick), mouse wheel adjusts radius 1–8 m, gizmo ring shows the brush footprint. Falloff is hard-core + edge fade (~70% radius full force, outer 30% smoothsteps) so Flatten produces a clearly flat plateau. Flatten target is captured on LMB-press so a sweep doesn't drift the anchor
- W1.15 save persistence: `Terrain` gains a persistent `edits: HashMap<ChunkCoord, HashMap<(u8,u8), f32>>` overlay. `set_vertex_height` records each brush write there in addition to mutating the live chunk; `generate_chunk` re-applies edits after PCG when the chunk re-loads. Save format extends with a flat `terrain_edits: Vec<ChunkEditsSave>` field, `#[serde(default)]` so existing saves still load. Empty maps are skipped on save → unedited worlds add nothing to disk
- W1.11 auto-flatten footprint API: `Terrain::flatten_rect(min_x, min_z, max_x, max_z, skirt_width, noise)` snaps every vertex inside the footprint to the median ground height under it, then blends a smoothstep skirt outward by `skirt_width` tiles using Chebyshev distance. Wired to a debug `F` hotkey (4×4 footprint, 2-tile skirt) in edit mode; Phase 2 building placement will swap the hotkey for a real footprint read off the placed piece

**Deferred / not started in this session:**
- W1.10 Smooth + Paint brushes (next slice)
- W1.14 brush hotbar UI (egui readout of active brush + radius)
- W1.3 slope/rock material override on risers — *skipped per art direction*: the user explicitly wants tile sides to read as the same colour as the top, so painting risers as rock contradicts the look they signed off on. DEC-018 captures this
- W1.4 vertex tint atlas — partial: per-cell biome tint with shade variation already ships; the full atlas (one texture tile per biome on the top face) is a follow-up
- W1.12 / W1.13 navmesh + override — heavy lift, not started, no immediate visible payoff before Phase 5 NPC cats
- Spec amendment to W1.4 wording ("smooth blending" → "tile-aligned biome edges") still pending in spec/phases/02-terrain.md

**Decisions recorded:** DEC-017 (vertex-height grid replaces per-tile cuboids), DEC-018 (one stepped-block mesh + trimesh collider per chunk; collider lives on the chunk entity itself, no child)

**Tech debt closed:** DEBT-007 (per-tile entities), DEBT-008 (material/mesh duplication). Opened: DEBT-018 (warm-cell tile tint disabled by terrain rewrite), DEBT-019 (per-tile water swell parked by water-mesh-per-chunk migration)

**Files created:** `src/world/edit.rs`

**Files heavily modified:** `src/world/terrain.rs` (full rewrite — `Terrain` resource, vertex grid, stepped-block mesh builder, trimesh collider, edits overlay, brush APIs, `flatten_rect`), `src/world/chunks.rs` (lifecycle systems hand the resource), `src/world/water.rs` (per-chunk plane), `src/world/props.rs` / `src/animals/mod.rs` / `src/building/mod.rs` (height_at_or_sample), `src/world/mod.rs` (chained chunk lifecycle to cover load → unload → regen → water → props), `src/save.rs` (terrain_edits round-trip), `src/input/mod.rs` (Action::ToggleEditTerrain), `src/memory/tile_tint.rs` (stub)

**Surprising things:**
- Heightfield collider can't represent vertical risers — for the cuboid look, the cat physically *needs* trimesh collision so it has to jump up step risers (matches what it sees). Trimesh is heavier per chunk (~2-8k tris) but BVH build stays fast in release; debug build was where the freeze showed up
- `Collider::trimesh_with_flags(verts, tris, TriMeshFlags::all())` was a freeze trap: pseudo-normals + duplicate-vertex merging + topology graph + degenerate-triangle filtering all run on every brush regen. ~10–100 ms per chunk in debug, enough to lock the schedule when several chunks regen on the same frame. Plain `Collider::trimesh(verts, tris)` is dramatically faster and the preprocessing is redundant for our hand-built meshes
- Riser triangle winding has to flip per face — passing corners in the same NW/NE/SW/SE convention the helper uses for top quads gave the wrong cross-product direction for risers, so backface culling hid them. The visible symptom was "sides missing"; the diagnostic was that geometric and declared normals disagreed
- Even with the chunk-lifecycle `.chain()` covering load → unload → regen, `spawn_chunk_water` and `spawn_chunk_props` were still racing: they parent themselves to the chunk entity, and if Bevy scheduled them in parallel with `load_nearby_chunks`, the props' `add_child` could apply before the chunk-entity spawn command. Extending the chain to cover both consumers is the fix
- The cuboid topology choice has nice cascading wins downstream: W1.3 slope shader becomes binary (always 0° or 90°) — could even be replaced by two materials. W1.12 navmesh boundaries become cell-aligned. Auto-flatten footprints look clean as defined "building pads"
- The rounded-stepped-grid (heights are stored continuous f32 but the mesh emits per-cell flat tops so the visual stays chunky regardless) means brushes can use continuous deltas without breaking the look — no per-vertex accumulator needed
- "Same colour all over" with Lambertian shading: the user wanted tile sides to *read* as the same biome colour as the top. First attempt set all riser normals to +Y (so the sun lit them identically); user rejected — "looks weird". Reverting to true side-facing normals gave proper depth shading while keeping the colour identical, which is what they wanted

**Open threads:**
- W1.10 Smooth + Paint brushes — Smooth nudges step-jumps toward neighbours' average; Paint cycles biome IDs (which would also need to drive prop respawn and per-chunk regen)
- W1.14 brush hotbar UI — egui readout
- W1.3 / W1.4 polish — atlas + (skipped) rock-on-riser shader
- W1.12 / W1.13 navmesh — bigger lift, save for when NPC cats need it
- Spec amendment to `spec/phases/02-terrain.md` W1.4 wording

## 2026-04-29 -- Phase 0 closed (12 shipped + 2 accepted deferrals)

**What was done:**
- W0.1 Bevy 0.16 → 0.18.1 with the full breaking-change pass: `Event` → `Message` (derive, EventReader/Writer, add_event/add_message), UI bundles (BorderRadius into Node), AmbientLight split into per-camera component + GlobalAmbientLight resource, ScrollPosition Vec2 tuple, BorderColor per-side with `::all()`, CascadeShadowConfigBuilder moved to bevy::light, ScalingMode moved to bevy::camera, UiSystem → UiSystems, WindowResolution constructor changed
- W0.2 WorldNoise cached as a `Resource` via `FromWorld` reading `ChunkManager.seed`. Every per-frame consumer (player, animals, building, particles, props, memory/verbs) now takes `Res<WorldNoise>`. Closes DEBT-012
- W0.5 Replaced hand-rolled GameInput with leafwing-input-manager: `Action` enum is exhaustive per spec §4.4/§4.5 plus game-specific verbs (ToggleCraft, Save, Nap/Examine/Mark, MenuUp/Down/Confirm, HotbarNext/Prev). KB+M and gamepad bindings shipped. Mouse-left intentionally unbound; gated through `CursorState::world_click`. The unread `PlaceEvent` was dropped to fit the 16-param SystemParam limit on `place_building`. Supersedes DEC-007
- W0.6 Pilot egui port of the crafting menu under `EguiPrimaryContextPass`. Data layer unchanged — `CraftingState`/`RecipeRegistry`/`CraftRequest` events still drive crafting. Visual approximation of the Spiritfarer parchment: warm dark Frame, gold stroke, gold/dim/red palette, scrollable recipe list, CRAFT/need-more pills. The Bevy UI crafting tree is no longer spawned
- W0.8 Day cycle bumped from 2.0 → 1.0 (12 min/day → 24 min/day). DEC-016 supersedes DEC-006
- W0.9 bevy_asset_loader Loading state: `UiAssets` derives `AssetCollection`; `LoadingState::new(Loading).continue_to_state(MainMenu).load_collection::<UiAssets>()` configured in `StatePlugin`. `spawn_ui` moved from PostStartup to `OnEnter(GameState::Playing)`. HUD systems gated with `run_if(in_state(Playing))`. Three new egui screens: Loading (centred title + spinner), MainMenu (title + Start Game + Quit), Pause overlay (Resume / Main Menu / Quit). Pause now also routes back to MainMenu cleanly
- W0.10 Game state machine: `GameState` (Loading / MainMenu / Playing / Paused) plus `BuildState` sub-state (Idle / Building) under Playing. Esc pauses by freezing `Time<Virtual>` rather than per-system gating. Esc-to-pause is suppressed when build mode or crafting menu is active so it doesn't hijack their cancel/close. Closes DEBT-003 + DEBT-011 / DEC-014
- W0.12 Save path resolves via `directories::BaseDirs::data_dir()` namespaced as "Cat World" — Steam Cloud-friendly per-user dir, e.g. `~/Library/Application Support/Cat World/savegame.json` on macOS. `--save-dir` CLI override for tests
- W0.13 World seed persisted in SaveData with `serde(default)` so legacy saves keep loading; ChunkManager.seed restored on load. Closes DEBT-004
- W0.14 Headless smoke tests in `tests/smoke.rs` using `MinimalPlugins + StatesPlugin`. Two tests: app boots and ticks 30 frames; state transition resolves in one tick. First brick of DEBT-006
- W0.3 + W0.4 rapier + tnua + jump (follow-up session, DEBT-016 closed). `RapierPhysicsPlugin` + `TnuaControllerPlugin::<ControlScheme>::new(Update)` + `TnuaRapier3dPlugin::new(Update)`. Per-tile `Collider::cuboid` on terrain (intentionally throwaway, Phase 1 swaps for vertex-height trimesh). Prop colliders attached on a child entity offset to the prop's vertical centre so climb-on-top works. Per-form colliders on placed buildings (walls/doors/windows/floors/roofs) via `attach_for_form`, retiring the hand-rolled `push_player_out_of_walls`. Player: capsule rigid body + `LockedAxes::ROTATION_LOCKED` + `TnuaController` + `TnuaConfig` with `TnuaBuiltinWalkConfig { speed: 5.0, float_height: 1.0, cling_distance: 0.3, max_slope: PI/3 }` and `TnuaBuiltinJumpConfig { height: 1.6 }`. `Action::Jump` (Space + gamepad South) triggers the jump action; suppressed in build mode where Space is overloaded for placement. `snap_to_terrain` removed. Wading tuned: `floor_y = step_height(SEA_LEVEL) * 0.5 - 1.05` so capsule centre settles at y≈0 and the cat is half-submerged inside the water mesh. `init_water_ripples` switched to `try_insert` to swallow chunk-unload races that physics-driven `player_chunk` churn surfaced. Quit hard-exits via `std::process::exit(0)` (DEBT-017 — `AppExit` deadlocks under Bevy 0.18 + rapier + egui)

**Deferred with debt (acknowledged for Phase 0):**
- W0.7 procedural atmosphere — trialled and reverted; Bevy 0.18's Earth-scale lighting (RAW_SUNLIGHT, km-scale geometry, HDR + AcesFitted + Bloom) clashed with the warm pastel palette. DEC-013 amended to keep the manual `daynight::update_sky_color` gradient. Revisit only if a phase needs something the manual sky can't represent, or pair with the Phase 7 polish pass. DEBT-014
- W0.11 moonshine-save — declined after digging in: save data is dominated by *resources* (Inventory, WorldMemory, Journal, ChunkManager.seed), not entity-tagged components, so moonshine's reflection-auto-serialize win is marginal; on-disk format change JSON→RON would amend DEC-015; `ItemId`↔`save_key` glue still needed. Revisit at start of Phase 5 (NPC archetypes flip the cost-benefit). DEBT-015

**Decisions recorded:** DEC-013 (Bevy 0.18 + ecosystem stack, atmosphere clause amended to keep the manual gradient), DEC-014 (game state machine), DEC-015 (moonshine save format — declared, then deferred to Phase 5), DEC-016 (24-minute day cycle, supersedes DEC-006). Superseded: DEC-006, DEC-007, DEC-011

**Tech debt closed:** DEBT-003, DEBT-004, DEBT-011, DEBT-012, DEBT-016. Opened: DEBT-013 (Phase 0 catch-all, since superseded), DEBT-014 (atmosphere mismatch), DEBT-015 (moonshine deferred), DEBT-017 (AppExit deadlock).

**Files created:** `src/state.rs`, `src/ui/crafting_egui.rs`, `tests/smoke.rs`. Spec docs added under `spec/phases/` (00-index through 08-launch)

**Files heavily modified:** `Cargo.toml` (bevy 0.18, +leafwing-input-manager, +bevy_egui, +bevy_asset_loader, +bevy_rapier3d, +bevy-tnua, +bevy-tnua-rapier3d, +directories), `src/main.rs`, `src/input/mod.rs` (full rewrite), `src/ui/mod.rs` (crafting menu spawn skipped, HUD gated to Playing), `src/save.rs` (OS-aware path + seed persistence), `src/player/mod.rs` (full rewrite — tnua-driven), `src/world/mod.rs`, `src/world/biome.rs`, `src/world/terrain.rs` (rapier colliders + wade depth), `src/world/props.rs` (child collider entities), `src/building/collision.rs` (rapier-resolved walls), plus every gameplay consumer to swap `GameInput` → `ActionState<Action>` + `CursorState`

**Surprising things:**
- bevy_egui's `EguiPrimaryContextPass` is a proper schedule; multiple egui screens just register multiple systems on it with state-gated `run_if`
- `bevy-tnua` is the dashed crate name; `bevy_tnua` doesn't exist on crates.io
- Bevy 0.18's atmosphere is not a clean drop-in for non-realistic art directions; "tune the dials" fights the model
- 16-param SystemParam limit hits faster than expected once you split `GameInput` into `ActionState` + `CursorState` and consumers add `crafting`/`build_mode`/etc. — `place_building` had to drop the unread `PlaceEvent` to fit
- `TnuaBuiltinWalkConfig::speed` defaults to 20.0 and *multiplies* `desired_motion`. If you also pre-multiply by a unit-scale player speed, you get a 100 m/s cat. Pass a unit vector × sprint factor and let the config own the m/s
- Tnua's `TnuaScheme` derive generates `<EnumName>Config` as a sibling type in the same module — not under `bevy_tnua::controller`. Importing that path was a dead end
- The chunk unload race (entity despawned between query collection and deferred command apply) gets *much* more frequent under physics-driven player position because `player_chunk` recomputes faster while gravity/spring are settling. `try_insert` is the idiomatic fix
- Bevy 0.18 + rapier + egui can deadlock on `AppExit` shutdown — process exits cleanly via `std::process::exit(0)` but hangs on the polite path. DEBT-017

**Open threads (out of Phase 0 scope):**
- W0.7 atmosphere stays as DEBT-014; revisit during Phase 7 polish or pair with Phase 1 W1.3 shader work as a custom gradient skybox
- W0.11 moonshine-save stays as DEBT-015; revisit at start of Phase 5 when NPC archetypes flip the cost-benefit
- DEBT-017 AppExit deadlock; bisect across Bevy/rapier/egui upgrades, or re-route Quit through "auto-save + return to MainMenu"

## 2026-04-29 -- Full gameplay loop: biomes, crafting, building, animals, particles, save/load

**What was done:**
- Built 10-biome system (temperature/moisture noise: ocean, beach, desert, grassland, meadow, forest, taiga, tundra, snow, mountain)
- Mountains with ridged noise for dramatic peaks, snow caps
- Water system: ocean + rivers with ambient wave animation
- Prop sway: vegetation tilts away from player and springs back
- Inventory with 15 item types (7 raw, 8 crafted)
- Gathering system: proximity detection, E/click to collect, shrink animation
- Crafting system: Tab menu, 8 recipes, ingredient checking
- Building system: B to enter, mouse-aimed ghost preview, grid-snapped placement, R to rotate
- NPC animals: 5 types per biome (rabbit, fox, deer, penguin, lizard), wander + flee AI
- Particle effects: 5 types (leaves, fireflies, snowflakes, sand wisps, pollen), biome + time-of-day aware
- Save/load: auto-save 30s, F5 manual, JSON format (player pos, inventory, buildings)
- Input abstraction: unified GameInput resource, KB+mouse and gamepad support, cursor raycasting
- HUD: inventory hotbar, crafting menu, build mode prompt, gather prompt with context hints
- Attempted cat model animation integration (failed -- Blender 5.1 glTF export issue, see animation_pitfalls memory)
- Smooth terrain snapping (lerp Y position)
- Thicker terrain tiles (0.6) to prevent gaps

**Key decisions:**
- DEC-004 through DEC-006 recorded (chunks, props, day/night)
- Input abstraction designed for gamepad from the start
- Manual JSON save (no serde dependency)
- Biome classification: temperature + moisture noise, altitude cooling

**Files created:**
- src/world/biome.rs, src/world/water.rs
- src/input/mod.rs, src/inventory/mod.rs, src/gathering/mod.rs
- src/crafting/mod.rs, src/building/mod.rs
- src/animals/mod.rs, src/particles/mod.rs
- src/save.rs, src/ui/mod.rs
- assets/models/cat.glb (model works, animations don't)

**Open threads:**
- Cat model animations (need to validate GLB externally, try Blender 4.x)
- Music and ambient audio not started
- Discovery journal not started
- Weather system not started
- Crafting UI could be more polished (user feedback: "looks bad")
- Water reflections/distortion not working well with tile-based approach

## 2026-04-29 -- Chunk terrain, props, and day/night cycle

**What was done:**
- Implemented chunk-based infinite terrain (16x16 tiles, render distance 3, 4 chunks/frame cap)
- Refactored terrain.rs from single spawn to per-chunk generation with shared height/biome helpers
- Added noise-based prop spawning: trees (trunk+canopy), rocks, flowers, bushes, mushrooms
- Props are biome-aware (grass: trees/bushes/flowers/mushrooms, dirt: rocks/mushrooms, sand: sparse rocks)
- Built full day/night cycle with 6 phases (dawn/morning/day/dusk/twilight/night), 12-min real cycle
- Sky color, sun position, directional light, and ambient light all transition smoothly
- Ran playtest: compiles clean, clippy clean, game launches and runs well

**Key decisions:**
- DEC-004: Per-tile entities in chunks (simpler, enables future interaction)
- DEC-005: Noise-based prop placement (deterministic, biome-aware)
- DEC-006: Day/night timing (12-min cycle, peaceful moonlight at night)

**Files created:**
- src/world/chunks.rs, src/world/props.rs, src/world/daynight.rs

**Files modified:**
- src/world/mod.rs (rewired to chunk/props/daynight systems)
- src/world/terrain.rs (refactored to per-chunk generation)

**Tech debt logged:**
- DEBT-005: let-else patterns in player/camera, DEBT-006: no tests
- DEBT-007: per-tile entities, DEBT-008: material/mesh duplication per chunk

**Open threads:**
- Cat .glb model still needed from user
- Next priorities: inventory system, gathering, crafting

## 2026-04-29 -- Project scaffolding

**What was done:**
- Initialized Bevy 0.16 project with Cargo
- Created module structure: main, camera, player, world/terrain
- Implemented PCG terrain with layered Perlin noise and stepped heights
- Built isometric orthographic camera with smooth player follow
- Added WASD player movement aligned to iso camera
- Set up warm earthy color palette (grass/dirt/sand)
- Created full skill/memory infrastructure

**Key decisions:**
- DEC-001: Bevy over Unity/Godot (code-driven, no GUI editor)
- DEC-002: Isometric orthographic camera
- DEC-003: Perlin noise stepped terrain

**Files created:**
- Cargo.toml, src/main.rs, src/camera/mod.rs, src/player/mod.rs
- src/world/mod.rs, src/world/terrain.rs
- .claude/ skill and memory infrastructure

**Open threads:**
- Cat .glb model needed from user
- Chunk system needed before expanding world
- No game states yet
