# Voxel Mountain Caves

> Status: Designed (approved 2026-05-02)
> Extends: Phase 1 vertex-grid terrain (DEC-017, DEC-018)
> ADR: DEC-024
> Roadmap position: **Worldcraft Expansion workstream** (parallel to numbered Phase 0–7 EA roadmap; see `spec/phases/00-index.md` § Parallel workstreams). Not on the EA critical path; ships in independent slices alongside the lantern workstream.

## Why

The current terrain is a 2.5D heightmap: one Y per (X, Z) vertex. It's nailed the chunky outdoor aesthetic and supports brush sculpting beautifully, but every mountain is a solid block of risers — there is no "inside." Players have started raising tall columns just to *look* at the geometry and asked the natural follow-up: *can I go in there?*

A peaceful, lifelong, world-crafting game wants the world to keep rewarding exploration over hundreds of hours. Caves are exploration content with low ongoing-development cost (PCG generates them; small content packs fill them). They give mountains diegetic purpose, layer the world vertically, and unlock new prop and lighting work.

This spec adds **voxel mountains with PCG cave systems** without disturbing the heightmap surface that everything else depends on.

## Goals

- Mountains gain a **voxel interior** layered under the existing heightmap surface. Outdoor (non-mountain) terrain renders unchanged.
- **Three climate-flavored cave generators** (alpine / temperate / arid), each with a distinct shape vocabulary so the *form* of a cave previews its content. Caves only spawn in Mountain or Snow biome cells (the only high-elevation biomes); the climate flavor is decided by the *surrounding* lower-elevation biomes around the mountain's footprint.
- **Always-visible cave entrances** carved into mountain faces — the player sees a dark mouth from the surface and decides to go in.
- **Cave content** (crystals first, critters and ruins later) appears as biome-specific props inside generated chambers.
- **Terrain brush keeps its current feel.** Lowering a mountain still works; if a Lower tick breaches a cave ceiling, the chamber opens up as a dramatic **sinkhole** (rare, satisfying, not a "punch through every block" mechanic).
- **No mining verb.** The cat does not break blocks deliberately. Sinkholes are accidental discoveries via the existing brush.
- **Cube aesthetic preserved.** Caves render as stacked 0.5m cubes — finer than outdoor 1m risers but coherent with the chunky-low-poly look.

## Non-goals

- **Full Minecraft voxelization.** Outdoor terrain stays as the current heightmap. Beaches, grasslands, water — none of it converts to voxels.
- **Mining as a verb.** No pickaxe, no "break this block" input, no resource-extraction loop. Caves are explored, not stripped.
- **Underground biomes outside mountains.** No "dig down anywhere into a cave layer." Caves only exist where the heightmap rises into mountain biome.
- **Sub-mountain ore deposits, gem veins, or tiered minerals.** A future "deeper places" spec can revisit; V1 ships solid-rock voxels with surface-clinging crystal *props*.
- **Cave navmesh / AI navigation.** Critter content (Phase 4 of this spec) will define its own movement constraints; full navmesh stays under DEBT-021.
- **Sub-cell horizontal resolution outside mountains.** The 1m heightmap cell stays the world's primary spatial unit; voxels are an internal sub-grid for mountain cells only.
- **Procedural lighting beyond crystal-glow and a player lantern.** No torches, no flares, no carryable light props. Lighting polish is its own pass.
- **Critter and ruin content packs on day one.** They're scoped (see Phasing) but the spec ships with crystal-cave content only.

## Scope decisions (from brainstorming)

| Question | Choice | Why |
| --- | --- | --- |
| Voxel scope | **Mountains only** (not full Minecraft) | Outdoor cozy aesthetic stays; voxels are an additive system. |
| Brush interaction | **Brush works outside; Lower can sinkhole** | Preserves brush intuition; sinkholes become a delightful accident. |
| Cave content | **All three** (crystal / critter / ruin) | Long-term variety. V1 ships crystals only; B & C are content packs. |
| Cave shape | **Per-climate generators** | Shape previews content. Costs ~3× generator code, accepted. Climate inferred from surrounding non-mountain biomes (see Generation pipeline). |
| Cave entrances | **Always visible from outside** | Cleanest read for new players; sinkholes remain a bonus. |
| Voxel grain | **0.5m³** | Better cave detail and headroom than 1m³; 4MB total memory is fine. |

## Architecture

### Voxel storage

A new resource `VoxelLayer` lives alongside `Terrain` and stores per-chunk voxel data only for chunks that contain at least one Mountain or Snow biome cell ("highland chunk"):

```rust
#[derive(Resource, Default)]
pub struct VoxelLayer {
    pub chunks: HashMap<ChunkCoord, VoxelChunk>,
    pub dirty: HashSet<ChunkCoord>,
    /// Player-induced voxel removal (e.g. brush sinkhole). Persisted to save.
    pub carved: HashMap<ChunkCoord, HashMap<VoxelLocal, ()>>,
}

pub struct VoxelChunk {
    /// Bit-packed solid/empty: 1 = rock, 0 = air.
    /// Layout: `solid[y * (W*W) + z * W + x]` with W = CHUNK_CELLS * 2 = 64.
    solid: BitVec,
}

/// (lx, ly, lz) inside a voxel chunk: lx,lz in 0..64; ly in 0..VOXEL_HEIGHT.
pub type VoxelLocal = (u8, u8, u8);

pub const VOXEL_PER_CELL: usize = 2;          // 2×2 sub-grid per 1m heightmap cell
pub const VOXEL_SIZE: f32 = 0.5;              // metres
pub const VOXEL_HEIGHT: usize = 60;           // up to 30m tall mountains
```

Memory: `64 × 64 × 60 = 245,760 bits ≈ 30KB` per voxel chunk. With RENDER_DISTANCE=2 (max 25 loaded chunks, of which maybe 10 contain mountains), worst-case ~300KB. Trivial.

### Heightmap-voxel coupling

The heightmap is the **cap** on the voxel column. For a mountain cell with heightmap_y = H:

- Voxels in that cell's 2×2×Y column are **solid** for `y ∈ [0, floor(H / 0.5))`
- Voxels at the cap level (`floor(H / 0.5) * 0.5 ≤ y < H`) render as a **fractional cap quad** at the heightmap's true 0.25m-stepped Y, so the silhouette stays exactly where the brush put it
- Voxels at `y ≥ ceil(H / 0.5)` are **air**

When the brush Lower drops H from 6.25 → 6.00, the cap quad slides down without removing a voxel. From 6.00 → 5.75 the cap quad slides further AND the top voxel layer (y=5.5..6.0) becomes air.

Cell biome decides which renderer runs:
- Mountain or Snow biome cells → voxel mesher (combined cube faces + cap quad)
- All other cells → existing heightmap mesher (top quad + risers), unchanged

### Mesh generation

Cube-faced (not marching cubes / surface nets) — matches the existing aesthetic exactly.

For each mountain chunk:
1. Walk every solid voxel, emit a face quad for each of its 6 neighbours that is air (greedy meshing optional, V1 keeps per-face quads — geometry count is bounded by surface area, not volume).
2. For cap cells, emit a top quad at the heightmap's true Y (the 0.25m step), not at the voxel boundary.
3. Combine with the existing heightmap geometry for non-mountain cells in the same chunk into one merged `Mesh`.
4. Vertex colours: rock tint with mild per-cell variation (same `cell_color` style as outdoor); crystal voxels override with their glow material in a separate sub-mesh / second material.

Mesh build target: <5ms per chunk in release. Voxel chunks have higher tri counts than current heightmap chunks (estimate ~5–15k tris vs ~1–8k today), within tolerance.

### Collider

Same trimesh-with-`FIX_INTERNAL_EDGES` pattern as today (`build_chunk_collider`). One collider per chunk over the merged heightmap+voxel geometry. The flag remains required (capsule-vs-trimesh corner stability — see palace memory `palace_rapier_epa_trimesh.md`).

Cave interiors are walkable because the cube faces of empty-adjacent solid voxels form a navigable surface; the cat's capsule physically clips the same walls and floors it sees.

### Generation pipeline

Runs once per chunk, after `Terrain::generate_chunk` fills the heightmap:

1. **Voxel fill.** For each Mountain or Snow cell in the chunk, fill its 2×2 voxel column solid from y=0 to `floor(heightmap_y / 0.5)`.
2. **Climate classification.** Sample biomes in a 5×5-cell ring around the chunk centre. Classify the chunk's cave climate:
   - majority **Forest / Taiga / Meadow** in the ring → `Temperate` (critter caves, Phase 4)
   - majority **Desert / Beach / Grassland (hot)** → `Arid` (ruin caves, Phase 5)
   - everything else (other Mountain/Snow chunks, Tundra, mixed) → `Alpine` (crystal caves, Phase 3)
3. **Climate-specific cave carve.** Run the matching generator. Each generator carves voxels by writing air into the `solid` bitvec.
4. **Entrance guarantee.** Each generated cave network finds its closest cave-cell to an outward-facing mountain edge and carves a connecting corridor until it breaches the surface (creates a visible mouth). If the chunk has no outward face within reach (rare — fully interior of a mountain mass), skip cave generation for that chunk; the next-out chunk will own the entrance.
5. **Cave content.** Walk solid-adjacent-air voxels in cave chambers, spawn climate-specific props at sampled positions.

Generators are seeded from `(world_seed, chunk_coord)`. To keep caves coherent across chunk boundaries, generation operates over a `3×3` chunk supercluster centred on the target chunk: worm walks may cross chunk boundaries within the supercluster, but writes are scoped to the target chunk only. Determinism is preserved because each chunk re-runs the same supercluster computation; the result for each chunk is well-defined regardless of load order.

#### Alpine generator (crystal caves) — Phase 3 (V1 ships this)

- **Shape vocabulary:** Few large rounded chambers (`8–14` voxels diameter), connected by short narrow tunnels (`2–3` voxels cross-section).
- **Algorithm:** 3D Perlin noise threshold (`noise(x*0.05, y*0.07, z*0.05) > 0.55`) for chambers, biased to clump by sampling at low frequency. Perlin worm seeded at one chamber, walked to the next chamber, carved at radius 1 voxel.
- **Density:** ~1 cave network per `4×4` highland-chunk region.
- **Content placement:** Crystal clusters cling to chamber walls (any air voxel touching ≥2 solid wall voxels has a 5% chance of receiving a crystal cluster prop). "Wall" here means a side-facing solid neighbour, not a floor or ceiling — keeps clusters off the floor where the cat walks.

#### Temperate generator (critter dens) — Phase 4

- **Shape vocabulary:** Tight twisty warrens. Average tunnel cross-section `2×2` voxels. High branching factor.
- **Algorithm:** Pure Perlin worm with high curvature noise, multiple worm seeds per chunk, each branching 2–4 times.
- **Density:** ~1 warren per Temperate-classified highland chunk.
- **Content placement:** Sleeping nooks at branch ends — small dead-end alcoves with critter beds.

#### Arid generator (ruins) — Phase 5

- **Shape vocabulary:** Authored room templates stitched by straight corridors.
- **Algorithm:** Pick room templates from a `RoomTemplate` library (hand-authored 8×8×4 voxel volumes saved as flat bit arrays). Place 3–6 rooms per chunk, connect with straight A→B corridor carves.
- **Content placement:** Each template has named anchor points (`{shrine, statue, urn}`) where ruin props spawn.

### Brush interaction (sinkhole rule)

The brush operates on the heightmap as today; the voxel layer reacts:

```rust
// Inside Terrain::set_vertex_height (after the existing height update):
if cell.biome.is_highland() && new_h < old_h {
    let voxel_y_old = (old_h / VOXEL_SIZE).floor() as u8;
    let voxel_y_new = (new_h / VOXEL_SIZE).floor() as u8;
    for vy in voxel_y_new..voxel_y_old {
        // Mark voxels in this 1m cell × 0.5m vertical slice as carved.
        for vx in 0..VOXEL_PER_CELL { for vz in 0..VOXEL_PER_CELL {
            voxel_layer.carve(chunk_coord, (vx, vy, vz));
        }}
    }
    // Sinkhole detection: if any newly-air voxel was already adjacent to a
    // pre-existing air voxel below it, that's a breach into a chamber.
    if voxel_layer.detects_breach(...) {
        events.send(SinkholeEvent { ... });
    }
}
```

`SinkholeEvent` triggers a one-shot dust+rumble VFX so the moment reads as dramatic. The next mesh rebuild for that chunk shows the new opening.

Carved voxels are stored in `VoxelLayer.carved` and persisted to save (PCG cave layout is regenerated from seed; only player carving needs persistence).

### Lighting

Caves are dark by default (no skylight reaches inside). This spec owns two cave-side lighting concerns; the player-carried light itself is owned by the **Night Torch** workstream (`docs/superpowers/specs/2026-05-02-night-torch-design.md`, DEC-025) and is not described here.

- **Crystal voxels** emit point lights with climate-tinted colour (alpine crystals = soft blue/purple). Capped at the 8 nearest to camera to keep render cost bounded; further crystals fall back to emissive material only (still visible, no cast light).
- **Ambient masking via `DarknessFactor`.** The Night Torch spec already defines a shared `DarknessFactor` resource (0.0 = bright day, 1.0 = full dark) computed from `WorldTime`. This spec contributes a cave occupancy term that gets OR-ed into `DarknessFactor` (the max of "it's night" and "I'm inside a cave"). Probe rule: any solid voxel directly above the cat's head within 4m → cave-occupied = 1.0. Reuse the indoor-reveal probe pattern (`fade_camera_occluders` line 158).

Coupling rule: the cave system **only** publishes the cave occupancy contribution. It must not spawn a player light, must not touch the torch entity, must not modify ambient intensity directly. Everything that "the cat now lights its surroundings inside a cave" relies on flows through `DarknessFactor` → torch intensity, owned entirely by the Night Torch workstream.

### Save format extension

Add to `savegame.json`:

```json
{
  ...
  "voxel_carved": [
    { "cx": -3, "cz": 1, "voxels": [{ "x": 12, "y": 8, "z": 5 }, ...] },
    ...
  ],
  "discovered_caves": [
    { "chunk": [-3, 1], "cave_id": 0, "first_visit": "2026-05-02T..." },
    ...
  ]
}
```

PCG caves regenerate from seed on load. Carved voxels are re-applied after generation, same pattern as `terrain.edits`.

## Performance budget

| Item | Budget | Notes |
| --- | --- | --- |
| Voxel chunk memory | 30KB | Bit-packed solid/air |
| Total voxel data, 25 loaded chunks | ~300KB | Only ~10 of 25 likely have mountains |
| Mesh build (combined heightmap + voxel) | <8ms / chunk in release | Up from ~3ms today |
| Trimesh collider | <8ms / chunk in release | Same FIX_INTERNAL_EDGES path |
| Cave generation per chunk | <20ms (one-shot at chunk load) | Worm walks + noise samples |
| Crystal point lights | 8 max simultaneous | LRU by camera distance |
| Frame time delta vs today | <2ms in mountain-rich areas | Validate via instrumentation |

If we breach budget on debug builds (the realistic case given the existing trimesh-rebuild observation), document and accept — release is what the player ships against.

## Testing

- **Unit:** voxel chunk indexing (`VoxelLocal` ↔ flat index), neighbour queries (six-face, edge wrap), bitvec round-trip.
- **Unit:** sinkhole detection — Lower brush ticks crossing cave ceilings emit exactly one `SinkholeEvent` per breach, not one per voxel removed.
- **Unit:** generator determinism — same `(seed, chunk_coord)` produces identical voxel chunk + content layout across runs.
- **Integration:** load a known seed, walk the cat into a known cave, verify cave entrance is reachable and chamber is enterable.
- **Visual regression (manual):** screenshot a known mountain at known angle, eyeball cave entrance shape stays stable across refactors.

## Phasing

Each phase is shippable on its own and unlocks the next.

1. **Voxel storage + mesher + collider.** Voxel layer fills mountain cells solid; renders identically to existing mountains. Validates the storage / mesh / collider pipeline without any cave content. Brush works as today; no carving yet.
2. **Sinkhole carving.** Brush Lower carves voxels; sinkhole detection fires the event and VFX. Still no PCG caves — so sinkholes always punch into solid rock and produce a dent, not a chamber. Validates the carving + persistence path.
3. **Alpine cave generator + crystal content.** First real caves. Climate classification + visible entrances on mountain faces. Crystal props + glow material + lighting + ambient mask. **First playable shipping moment.**
4. **Temperate cave generator + critter content pack.** Once critter AI lands (Phase 5 of the main spec).
5. **Arid cave generator + ruin templates.** Authoring tool for templates is its own small spec.

## Open questions

- **Crystal lighting at scale.** 8 simultaneous point lights might still feel sparse in a chamber with 30 crystals. Fallback options: bake light into vertex colours per chunk on generation, or use a single chamber-wide ambient glow probe instead of per-crystal lights. Defer to phase 3 implementation.
- **Carved voxel persistence beyond unload.** Current heightmap edits persist forever. Voxel carves probably should too, but if a player goes nuts with Lower over a huge mountain range, the carved set could grow large. Cap or accept? Probably accept; the brush is slow enough that this isn't realistic in normal play.

## Resolved (during brainstorming)

- **Sinkhole egress.** If the cat falls into a deep chamber, the player must find a way out — either by walking through a connected tunnel to a natural mouth, or by Raising the floor under the cat with the brush. No auto-spawned rope or safe-descent prop. The cozy framing tolerates "I'm stuck for a moment, let me look around."
- **Player light.** Owned by the **Night Torch** workstream (`docs/superpowers/specs/2026-05-02-night-torch-design.md`, DEC-025). This spec's only coupling is publishing a cave-occupancy term into the shared `DarknessFactor` resource defined there.
