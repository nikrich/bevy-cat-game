# Phase 1 — Terrain Rewrite & Editing

> Status: Planned
> Depends on: Phase 0
> Exit criteria: World runs on a vertex-height grid with chunked single-mesh rendering. Player can raise, lower, flatten, smooth, and paint terrain. Buildings auto-flatten footprints. Navmesh updates with terrain edits. Existing biomes, water, props, and ambient wildlife continue to work. Spawn terrain visibly elevated, matching today's aesthetic.

## Goal

Replace the per-tile cuboid stepped terrain with the spec's vertex-height grid. Add the player-facing brushes that turn terrain into a creative material. Make placing a building automatically level the ground beneath it. Generate a navmesh so cats can pathfind in later phases.

## Why now

Building snap, auto-flatten, slope blending, and stair/path overrides all assume the vertex grid. Retrofitting them onto cuboids would be wasted work. Per user direction: terrain rewrite goes early, terrain at spawn must remain visibly elevated, not flat.

## Deliverables

- Vertex-height grid replacing per-tile cuboids
- One mesh per terrain chunk (not 256 entities per chunk)
- Slope-based material override shader (rock when normal angle > 45°)
- Per-vertex biome tint with shader-blended transitions
- Five terrain brushes: Raise, Lower, Flatten, Smooth, Paint
- Auto-flatten on building placement, with a 1–2 tile blended skirt
- Dirty-chunk regen (cap N chunks/frame to avoid hitches)
- Navmesh generated from terrain via `bevy_landmass`, regenerated on dirty chunks
- Spawn-time terrain remains visibly varied (PCG elevation preserved)
- Existing water, props, biomes, animals, gathering all migrate to the new system

## Decisions to record

- DEC-017 — Vertex-height grid replaces per-tile cuboids; supersedes DEC-003 and DEC-004
- DEC-018 — One terrain mesh per chunk (32×32 vertices) supersedes DEC-004 entity model
- DEC-019 — Slope-based material override threshold: 45° normal angle for rock material
- DEC-020 — Auto-flatten skirt width: 2 tiles, smoothstep falloff
- DEC-021 — Navmesh: `bevy_landmass` (replaces `oxidized_navigation`, which is stuck on Bevy 0.16 per the 2026-04-29 crate audit). Regenerated per-chunk on dirty, max 1 navmesh chunk regen per frame.

## Tech debt closed

- DEBT-007 — per-tile entities in chunks
- DEBT-008 — material/mesh duplication per chunk (now shared meshes per chunk + shared materials)

## Work breakdown

### W1.1 — `Terrain` resource: vertex-height grid

**What:** Define `Terrain { chunk_size_verts: 32, world_dims_chunks: (W, H), heights: HashMap<ChunkCoord, Box<[f32]>>, materials: HashMap<ChunkCoord, Box<[BiomeId]>>, dirty: HashSet<ChunkCoord> }`. Heights stored row-major, 33×33 floats per chunk (32 cells = 33 verts including shared edge). Material IDs index into a small atlas table.
**Acceptance:** Setting a vertex height and querying it round-trips. Reading a chunk's flat array is contiguous (verify with a microbench). Two chunks share their edge vertices logically (writes to one mark both dirty).

### W1.2 — Chunked terrain mesh generation

**What:** New system `regenerate_dirty_chunks` runs in `Update`, capped at 4 dirty chunks per frame. Generates a single mesh per chunk with `Mesh::new(PrimitiveTopology::TriangleList)`. Two triangles per quad. Computes per-vertex normals. Writes vertex colors from biome tint sampled at each vertex.
**Acceptance:** Visiting all chunks at render distance shows continuous, gap-free terrain. Frame time on regen is ≤2 ms per chunk on a Mac M-series.

### W1.3 — Slope-based material override shader

**What:** Custom material extending `StandardMaterial`. Shader samples a "rock" albedo + normal map and blends in based on `dot(normal, up)` exceeding 45°. Smoothstep blend across ±5° around the threshold.
**Acceptance:** Cliffs and steep ravines show rock automatically, regardless of underlying biome. Flat tiles show their biome material. Slopes between blend smoothly with no banding.

### W1.4 — Per-vertex biome tint and material atlas

**What:** Each vertex carries a biome ID. Vertex colors written into mesh from biome palette table at regen time. Texture atlas (one image, N tiles) holds biome-base albedo; UVs derived from biome ID. Slopes blend tile + rock per W1.3.
**Acceptance:** All 10 biomes visible and distinct. Transitions blend smoothly across vertex boundaries (no hard tile edges). Painting a tile with the Paint brush updates both arrays and the next regen reflects the change.

### W1.5 — PCG migration: preserve elevation aesthetic

**What:** Port `world::biome::WorldNoise` height generation to the new grid. Heights remain layered Perlin with the same step quantization to keep the chunky look (now applied as a height offset rather than entity Y position). Biome classification unchanged. Validate spawn area still visibly varies in elevation.
**Acceptance:** Spawn screenshot side-by-side with current build shows comparable terrain shape and visual density. Biome distribution matches.

### W1.6 — Water migration

**What:** Water tiles re-implemented as a separate water mesh per chunk that lives at sea level, masked by terrain height. Existing wave shader carries over. Rivers use the same domain-warped noise carving heights below water level.
**Acceptance:** Coastlines, rivers, and lakes appear in roughly the same locations as today. Wave animation unchanged. No z-fighting at shore.

### W1.7 — Props migration

**What:** Prop spawning samples the new height grid for placement Y. Existing PropSway and biome-aware rules unchanged. Prop entities are no longer parented to per-tile entities (which no longer exist) — they parent to chunk entities. Despawn-on-chunk-unload preserved.
**Acceptance:** Tree, rock, flower, mushroom, bush, cactus, dead-bush, ice-rock, tundra-grass, pine-tree placement reproduces today's distribution per biome. Sway animation unaffected.

### W1.8 — Animals migration

**What:** Wandering AI samples height grid instead of stepping per-tile entities. Despawn-with-chunk path unchanged.
**Acceptance:** Rabbits, foxes, deer, penguins, lizards spawn and wander as before. No teleport artifacts on slope transitions.

### W1.9 — Gathering migration

**What:** Gathering proximity detection unchanged in concept; reads from prop entities directly, not tile entities. Update any code that walked `Tile` components.
**Acceptance:** Existing gathering loop (E to collect, shrink animation) works unchanged from player perspective.

### W1.10 — Terrain brushes: Raise / Lower / Flatten / Smooth / Paint

**What:** Hotbar entries that, while held active, paint vertex heights or material IDs under the mouse cursor. Brush radius and intensity bound to `Action::ScrollUp`/`ScrollDown` in `BuildState::Building`. Falloff uses smoothstep over radius. Flatten target = vertex under cursor at activation start. Smooth = local average. Paint cycles through unlocked materials.
**Acceptance:** Each brush feels responsive (no lag > 1 frame between cursor move and visual update). Held-LMB sweep produces a continuous edit, not stutter. Released brush leaves clean state, no orphaned dirty chunks.

### W1.11 — Auto-flatten on building placement

**What:** When a building footprint is placed (Phase 2 hooks this), compute footprint AABB → sample median ground height under footprint → set all vertices inside footprint to target height → blend a 2-tile skirt outward via smoothstep. Mark affected chunks dirty.
**Acceptance:** Place a 4×4 m floor on a 1.5 m elevation gradient → surface inside floor is perfectly level → terrain outside skirt is unchanged → skirt looks natural with no z-fighting at footprint edge.
*Note: building placement system itself ships in Phase 2; in Phase 1, expose the function and call it from a debug hotkey.*

### W1.12 — Navmesh via `bevy_landmass`

**What:** Add `bevy_landmass` (replaces `oxidized_navigation`, stuck on Bevy 0.16 as of 2026-04-29). Generate one navmesh tile/island per terrain chunk. On chunk regen, mark the corresponding navmesh tile for rebuild (cap 1 tile/frame). Walkable slope ≤ 30° per spec §5.6. `bevy_landmass`'s tile model differs from oxidized_navigation's — adapt the chunk-to-tile mapping accordingly during the integration spike.
**Acceptance:** Navmesh covers walkable terrain, excludes water and steep slopes. Debug overlay toggleable. Rebuilding after a Raise brush sweep takes < 1s per affected chunk.

### W1.13 — Stair / path navmesh override (placeholder)

**What:** Component `NavmeshOverride { walkable_polygon, walkable_slope_override }` that, when attached to an entity, augments the navmesh with that polygon. Used in Phase 2 by stairs and Phase 6 by cart paths. Phase 1 ships the data path and a manual debug spawn.
**Acceptance:** Manually spawning a `NavmeshOverride` over a 60° slope makes that slope walkable in the navmesh debug overlay.

### W1.14 — Brush hotbar UI

**What:** Build mode hotbar (egui) shows brush icons + active brush + radius/intensity readout. Tooltips with hotkeys.
**Acceptance:** Brushes selectable via Hotbar1..5 keys and clicks. Active brush highlighted. Radius/intensity values display correctly.

### W1.15 — Save migration: terrain edits persist

**What:** Mark `Terrain` resource for moonshine save. Persist heights and materials per chunk only if differing from PCG default (delta encoding). On load, regenerate PCG default + apply deltas.
**Acceptance:** Raise a hill, save, quit, reload → hill is there. Save file size for an unedited world is small (deltas empty). Heavily edited world saves in < 5 MB for the playable area.

## Risks / open questions

- **Performance of vertex-height + custom shader on integrated GPUs.** Test on Steam Deck early. If frame time spikes, fall back to vertex colors only (no slope shader override) and add the override later.
- **Auto-flatten ergonomics with stairs/multi-level buildings.** Phase 2 may need to disable auto-flatten for certain piece types. Track as Phase 2 risk.
- **`bevy_landmass` API differences.** Replacing `oxidized_navigation` per the 2026-04-29 crate audit. `bevy_landmass` has a different API surface; budget 1–2 days for adapter work during W1.12. Fallback if it doesn't fit cleanly: `vleue_navigator` (also 0.18-ready) or a hand-rolled grid navmesh.

## Out of scope

- Building snap kit (Phase 2)
- Stairs as buildable pieces (Phase 2)
- Cart paths (post-EA, but `NavmeshOverride` data shipped in W1.13)
- Stamp tools (post-EA per spec §5.4)

## Estimated effort

8–12 work-days. Mesh regen and shader work are the slowest items.
