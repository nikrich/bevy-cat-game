# Voxel Caves V0 -- Debug-Carved Cavities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Player can press a debug key (G) over a mountain to carve a vertical cylinder of voxels into it. The cavity renders as cube-faced air pocket inside the mountain that the player can see and (with sufficient mountain height) drop into.

**Architecture:** Extend the Stage 1 voxel substrate with a **carve API** (mutate `VoxelChunk` solid bits, track changes in a new `VoxelLayer.carved` set + `VoxelLayer.dirty` set). Add a **cave-face emission helper** that walks `VoxelLayer.carved` for a chunk and emits cube faces only at carved-air vs. solid-voxel boundaries (intentionally NOT emitting at solid-solid or air-air boundaries). Integrate the helper into the existing `build_chunk_geometry` so the cave faces append to the same chunk mesh + collider buffers -- one mesh per chunk, one collider, no new render entities. A **bridge system** copies `VoxelLayer.dirty` into `Terrain.dirty` so the existing `regenerate_dirty_chunks` rebuilds the chunk mesh whenever a carve happens. The **debug brush** (G key) reads the cursor world position, computes a vertical cylinder of voxel coordinates, and calls `carve_voxel` for each.

**Tech Stack:** Bevy 0.18, no new dependencies. Reuses existing chunk regen, brush input, and `Gizmos` patterns.

**Reference:** [Stage 1 storage substrate](2026-05-02-voxel-storage-substrate.md), [cave spec](../specs/2026-05-02-voxel-mountain-caves-design.md), `DEC-024` in `.claude/memory/decisions.md`.

---

## Scope deviation from the spec (and from earlier Stage 3 framing)

The spec's Stage 3 calls for a full PCG alpine generator with chambers, worms, visible mountain-face entrances, crystal content, and ambient masking. This V0 ships **only** the carve API + mesher + debug brush. The PCG generator becomes a V1 follow-up plan whose tasks are dramatically smaller once carving and meshing exist.

Reasons:
- The user has been blocked from "see a cave" by the absence of carving + meshing. Shipping that first creates the testing surface that V1's PCG generator needs.
- Carving + meshing have well-defined success criteria (carve a voxel, mesh shows the face) that don't depend on noise tuning, content placement, or lighting.
- The mountain-face entrance, crystal placement, and lighting work all assume a working voxel mesh. Building those before the mesh is buying-the-house-before-the-foundation.

What V0 explicitly defers:
- PCG cave generator (V1)
- Crystal voxel content (V1.5)
- Glow material + crystal point lights (V1.5)
- Ambient cave mask (couples with `DarknessFactor` from the torch spec; V1)
- Visible side-face entrances (V1 -- V0 carves vertical shafts so player drops in from above)
- Sinkhole-on-Lower-brush integration (Stage 2 work; deferred indefinitely until brush sinkholes become a priority)
- World-gen mountain amplification (the user already has an 8.75m brush-raised tower in their save; that's tall enough for V0 testing)
- DEBT-027 fix (system ordering for fill in lifecycle chain) -- V1 will hit this when caves need to be generated in the same frame as fill

---

## Testing approach

Same patterns as Stage 1 (project is bin-only, no `--lib` target):

- **Pure functions** (carve API, voxel coordinate translation, face emission) get inline `#[cfg(test)] mod tests` unit tests in their source file. Run with `cargo test`.
- **System / integration / debug-brush work** is verified via `cargo check` (compile clean) + a manual playtest checkpoint at the end (raise a mountain, carve into it with G, observe the hole).

`cargo test` runs all inline tests in `src/` plus integration tests in `tests/`.

---

## File structure (target)

```
src/
  world/
    voxel.rs          # MODIFIED -- add `carved` + `dirty` fields, `carve_voxel` API, cave-face emission, bridge system
    terrain.rs        # MODIFIED -- call into voxel cave-face emission from build_chunk_geometry for highland chunks
    edit.rs           # MODIFIED -- add G hotkey for debug shaft carve (mutually exclusive with existing brush keys)
```

No new files. Voxel-side concerns stay in `voxel.rs`; the integration point in `terrain.rs` is one extra call inside `build_chunk_geometry`. The debug hotkey lives in `edit.rs` next to the existing brush hotkeys.

`voxel.rs` will grow from ~270 lines to ~450 lines. Still one clear responsibility (voxel storage, mutation, and cube-face geometry). If it crosses 600 lines, V1 will split it.

---

## Task 1: Add `dirty` and `carved` fields to `VoxelLayer`

**Files:**
- Modify: `src/world/voxel.rs` (extend `VoxelLayer` struct + tests)

- [ ] **Step 1: Write failing test for the new field defaults**

Append to the existing `tests` module in `src/world/voxel.rs`:

```rust
    #[test]
    fn voxel_layer_starts_with_empty_dirty_and_carved_sets() {
        let layer = VoxelLayer::default();
        assert!(layer.dirty.is_empty());
        assert!(layer.carved.is_empty());
    }
```

- [ ] **Step 2: Run to confirm fail**

Run: `cargo test voxel_layer_starts_with_empty`
Expected: FAIL -- `no field 'dirty' on type VoxelLayer` (and same for `carved`).

- [ ] **Step 3: Add the fields to `VoxelLayer`**

In `src/world/voxel.rs`, find the `VoxelLayer` definition (currently around line 161-166):

```rust
/// Resource holding voxel chunks for highland chunks. Non-highland
/// chunks are absent from the map (not stored as empty).
#[derive(Resource, Default)]
pub struct VoxelLayer {
    pub chunks: HashMap<ChunkCoord, VoxelChunk>,
}
```

Replace with:

```rust
use std::collections::HashSet;

/// Resource holding voxel chunks for highland chunks. Non-highland
/// chunks are absent from the map (not stored as empty).
///
/// `dirty` lists chunks whose voxel mesh needs regeneration. The bridge
/// system copies these into `Terrain.dirty` so the existing chunk
/// regen path rebuilds the mesh.
///
/// `carved` records which voxels have been mutated from their PCG
/// default. Stage 1 fills voxels solid up to the heightmap; carving
/// turns them back into air. The set is the source of truth for "this
/// voxel is part of a cavity" so the cave-face mesher knows where to
/// emit visible cube faces.
#[derive(Resource, Default)]
pub struct VoxelLayer {
    pub chunks: HashMap<ChunkCoord, VoxelChunk>,
    pub dirty: HashSet<ChunkCoord>,
    pub carved: HashMap<ChunkCoord, HashSet<VoxelLocal>>,
}
```

(Move the `use std::collections::HashSet;` to the top of the file alongside the existing `use std::collections::HashMap;` import -- combine into `use std::collections::{HashMap, HashSet};`.)

- [ ] **Step 4: Run the test to confirm it passes**

Run: `cargo test voxel_layer_starts_with_empty`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): add dirty and carved fields to VoxelLayer"
```

---

## Task 2: Implement `VoxelLayer::carve` API

**Files:**
- Modify: `src/world/voxel.rs` (add method on `VoxelLayer` + tests)

- [ ] **Step 1: Write failing tests**

Append to the `tests` module:

```rust
    fn highland_chunk_filled() -> (Terrain, VoxelLayer) {
        let mut terrain = Terrain::default();
        let mut data = ChunkData::empty();
        // Mark cell (3, 7) as Mountain at heightmap_y = 6.25 so its
        // voxel column at lx in {6,7}, lz in {14,15} is solid for
        // ly = 0..12.
        let i = ChunkData::idx(3, 7);
        data.heights[i] = 6.25;
        data.biomes[i] = Biome::Mountain;
        terrain.chunks.insert((0, 0), data);
        let mut layer = VoxelLayer::default();
        layer.chunks.insert((0, 0), build_voxel_chunk_for_coord((0, 0), &terrain).unwrap());
        (terrain, layer)
    }

    #[test]
    fn carve_flips_solid_voxel_to_air_and_records_in_carved_set() {
        let (_terrain, mut layer) = highland_chunk_filled();
        // (6, 5, 14) is in the solid range for the test chunk.
        layer.carve((0, 0), (6, 5, 14));
        let chunk = layer.chunks.get(&(0, 0)).unwrap();
        assert!(!chunk.get((6, 5, 14)));
        let carved = layer.carved.get(&(0, 0)).unwrap();
        assert!(carved.contains(&(6, 5, 14)));
        assert!(layer.dirty.contains(&(0, 0)));
    }

    #[test]
    fn carve_on_already_air_voxel_is_noop() {
        let (_terrain, mut layer) = highland_chunk_filled();
        // (6, 50, 14) is air (above the cap of ly=12).
        layer.carve((0, 0), (6, 50, 14));
        // Nothing recorded, nothing dirtied.
        assert!(layer.carved.get(&(0, 0)).is_none() || layer.carved[&(0, 0)].is_empty());
        assert!(!layer.dirty.contains(&(0, 0)));
    }

    #[test]
    fn carve_on_unloaded_chunk_is_noop() {
        let mut layer = VoxelLayer::default();
        layer.carve((42, 42), (0, 0, 0));
        assert!(layer.carved.is_empty());
        assert!(layer.dirty.is_empty());
    }
```

- [ ] **Step 2: Run to confirm fail**

Run: `cargo test carve`
Expected: FAIL with `no method named 'carve' found for struct VoxelLayer`.

- [ ] **Step 3: Implement `VoxelLayer::carve`**

Add an `impl VoxelLayer` block in `src/world/voxel.rs` immediately before the `VoxelPlugin` struct definition:

```rust
impl VoxelLayer {
    /// Flip the voxel at `(coord, local)` from solid to air. No-op if
    /// the chunk isn't loaded or the voxel is already air. On success,
    /// records the voxel in `carved` and marks the chunk `dirty` so
    /// the mesher rebuilds.
    pub fn carve(&mut self, coord: ChunkCoord, local: VoxelLocal) {
        let Some(chunk) = self.chunks.get_mut(&coord) else {
            return;
        };
        if !chunk.get(local) {
            return;
        }
        chunk.set(local, false);
        self.carved.entry(coord).or_default().insert(local);
        self.dirty.insert(coord);
    }
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `cargo test carve`
Expected: all three PASS.

- [ ] **Step 5: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): VoxelLayer::carve API marks dirty + records in carved"
```

---

## Task 3: Implement cave-face emission

**Files:**
- Modify: `src/world/voxel.rs` (add public function + tests)

- [ ] **Step 1: Write failing tests**

Append to the `tests` module:

```rust
    #[test]
    fn emit_cave_faces_returns_empty_when_no_carved_voxels() {
        let chunk = VoxelChunk::empty();
        let carved = HashSet::new();
        let faces = emit_cave_faces(&chunk, &carved);
        assert!(faces.is_empty());
    }

    #[test]
    fn emit_cave_faces_emits_six_faces_for_isolated_carved_voxel_in_solid() {
        let mut chunk = VoxelChunk::empty();
        // Make a 3x3x3 solid block centred at (10, 10, 10).
        for dx in 0..3 {
            for dy in 0..3 {
                for dz in 0..3 {
                    chunk.set((9 + dx, 9 + dy, 9 + dz), true);
                }
            }
        }
        // Carve out the centre voxel.
        chunk.set((10, 10, 10), false);
        let mut carved = HashSet::new();
        carved.insert((10, 10, 10));

        let faces = emit_cave_faces(&chunk, &carved);
        // One carved voxel, six solid neighbours -> six faces.
        assert_eq!(faces.len(), 6);
    }

    #[test]
    fn emit_cave_faces_skips_neighbour_when_neighbour_is_also_carved() {
        let mut chunk = VoxelChunk::empty();
        // 3x1x1 solid run.
        chunk.set((9, 10, 10), true);
        chunk.set((10, 10, 10), true);
        chunk.set((11, 10, 10), true);
        // Carve two adjacent voxels.
        chunk.set((10, 10, 10), false);
        chunk.set((11, 10, 10), false);
        let mut carved = HashSet::new();
        carved.insert((10, 10, 10));
        carved.insert((11, 10, 10));

        let faces = emit_cave_faces(&chunk, &carved);
        // Each carved voxel has 5 neighbours that are air (1 carved-air,
        // 4 outside-the-solid-line air) and 1 solid neighbour. So each
        // carved voxel emits 1 face = 2 total. Not 6+6.
        assert_eq!(faces.len(), 2);
    }

    #[test]
    fn emit_cave_faces_skips_voxels_at_chunk_boundary() {
        let mut chunk = VoxelChunk::empty();
        // Edge voxel (0, 10, 10) and its lone interior neighbour.
        chunk.set((0, 10, 10), true);
        chunk.set((1, 10, 10), true);
        chunk.set((0, 10, 10), false);
        let mut carved = HashSet::new();
        carved.insert((0, 10, 10));

        let faces = emit_cave_faces(&chunk, &carved);
        // Carved (0,10,10) has neighbours: (-1, off-chunk skip),
        // (1, solid), (0, 9), (0, 11), (0, *, 9), (0, *, 11) air.
        // Only (1, 10, 10) is solid -> 1 face.
        assert_eq!(faces.len(), 1);
    }
```

- [ ] **Step 2: Run to confirm fail**

Run: `cargo test emit_cave_faces`
Expected: FAIL -- `cannot find function 'emit_cave_faces' in this scope`.

- [ ] **Step 3: Implement the helper**

Add a free function `emit_cave_faces` in `src/world/voxel.rs` after the existing `build_voxel_chunk_for_coord`:

```rust
/// One face quad emitted by [`emit_cave_faces`]. Coordinates are in
/// chunk-local voxel space; the integrator multiplies by `VOXEL_SIZE`
/// and offsets by the chunk's world position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaveFace {
    /// World-space relative voxel coordinates (lx, ly, lz) of the
    /// carved voxel that owns this face.
    pub voxel: VoxelLocal,
    /// Direction the face points (away from the carved voxel toward
    /// the solid neighbour). One of +/-X, +/-Y, +/-Z unit vectors.
    pub normal: [i8; 3],
}

/// Emit one face per (carved voxel, solid neighbour) pair. Skips:
/// - Neighbours that are also carved (no face between two air voxels).
/// - Neighbours that are air outside the solid mountain (the
///   heightmap mesher renders the mountain skin; voxel mesher only
///   renders cave INTERIORS).
/// - Neighbours that fall outside the chunk (handled by V1; V0 caves
///   stay within a single chunk because the debug brush carves a
///   small cylinder under the cursor).
pub fn emit_cave_faces(chunk: &VoxelChunk, carved: &HashSet<VoxelLocal>) -> Vec<CaveFace> {
    let mut faces = Vec::new();
    let neighbours: [[i8; 3]; 6] = [
        [1, 0, 0], [-1, 0, 0],
        [0, 1, 0], [0, -1, 0],
        [0, 0, 1], [0, 0, -1],
    ];
    for &voxel in carved {
        let (lx, ly, lz) = voxel;
        for n in neighbours {
            let nx = lx as i16 + n[0] as i16;
            let ny = ly as i16 + n[1] as i16;
            let nz = lz as i16 + n[2] as i16;
            if nx < 0 || nx >= VOXELS_PER_CHUNK_SIDE as i16 {
                continue;
            }
            if nz < 0 || nz >= VOXELS_PER_CHUNK_SIDE as i16 {
                continue;
            }
            if ny < 0 || ny >= VOXEL_HEIGHT as i16 {
                continue;
            }
            let neighbour = (nx as u8, ny as u8, nz as u8);
            // Skip neighbours that are themselves carved.
            if carved.contains(&neighbour) {
                continue;
            }
            // Only emit a face if the neighbour is solid.
            if chunk.get(neighbour) {
                faces.push(CaveFace {
                    voxel,
                    normal: n,
                });
            }
        }
    }
    faces
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test emit_cave_faces`
Expected: all four PASS.

- [ ] **Step 5: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): cave-face emission helper for carved interior walls"
```

---

## Task 4: Convert `CaveFace` to mesh quads inside `build_chunk_geometry`

**Files:**
- Modify: `src/world/voxel.rs` (add public helper `append_cave_geometry` + tests)
- Modify: `src/world/terrain.rs:504-693` (call the helper in `build_chunk_geometry` for highland chunks)

- [ ] **Step 1: Write failing tests for `append_cave_geometry`**

Append to the `tests` module in `src/world/voxel.rs`:

```rust
    use super::super::terrain::ChunkGeometry;

    fn empty_geom() -> ChunkGeometry {
        ChunkGeometry {
            positions: Vec::new(),
            normals: Vec::new(),
            colors: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    #[test]
    fn append_cave_geometry_adds_six_quads_for_one_carved_voxel_in_solid() {
        let mut chunk = VoxelChunk::empty();
        for dx in 0..3 {
            for dy in 0..3 {
                for dz in 0..3 {
                    chunk.set((9 + dx, 9 + dy, 9 + dz), true);
                }
            }
        }
        chunk.set((10, 10, 10), false);
        let mut carved = HashSet::new();
        carved.insert((10, 10, 10));
        let mut geom = empty_geom();
        append_cave_geometry(&chunk, &carved, &mut geom);
        // Six face quads = 6 * 4 = 24 vertices, 6 * 6 = 36 indices.
        assert_eq!(geom.positions.len(), 24);
        assert_eq!(geom.normals.len(), 24);
        assert_eq!(geom.colors.len(), 24);
        assert_eq!(geom.uvs.len(), 24);
        assert_eq!(geom.indices.len(), 36);
    }

    #[test]
    fn append_cave_geometry_is_noop_when_carved_set_is_empty() {
        let chunk = VoxelChunk::empty();
        let carved = HashSet::new();
        let mut geom = empty_geom();
        append_cave_geometry(&chunk, &carved, &mut geom);
        assert!(geom.positions.is_empty());
        assert!(geom.indices.is_empty());
    }
```

- [ ] **Step 2: Run to confirm fail**

Run: `cargo test append_cave_geometry`
Expected: FAIL -- `cannot find function 'append_cave_geometry'`.

- [ ] **Step 3: Implement `append_cave_geometry`**

Add this free function in `src/world/voxel.rs` after `emit_cave_faces`:

```rust
/// Cave wall colour. Slightly darker than the rock biome colour so the
/// interior reads as "you are inside a stone cavity" not "you are
/// looking at a stone wall from outside". Linear-space.
const CAVE_WALL_COLOR: [f32; 4] = [0.32, 0.30, 0.28, 1.0];

/// Append cave-interior cube faces to an existing `ChunkGeometry`.
/// Face vertices are in chunk-local space (multiplied by `VOXEL_SIZE`
/// to convert voxel indices to metres). The chunk entity's
/// `Transform` already places the chunk's origin at its world NW
/// corner, so chunk-local positions render at the right world place.
///
/// Each face is one quad (two triangles). Winding follows the right-
/// hand rule against the face normal so backface culling does the
/// right thing.
pub fn append_cave_geometry(
    chunk: &VoxelChunk,
    carved: &HashSet<VoxelLocal>,
    geom: &mut super::terrain::ChunkGeometry,
) {
    for face in emit_cave_faces(chunk, carved) {
        let (lx, ly, lz) = face.voxel;
        // Chunk-local world position of the carved voxel's NW-bottom
        // corner.
        let x0 = (lx as f32) * VOXEL_SIZE;
        let y0 = (ly as f32) * VOXEL_SIZE;
        let z0 = (lz as f32) * VOXEL_SIZE;
        let s = VOXEL_SIZE;
        // The face sits on the side of the carved voxel facing the
        // solid neighbour, i.e. shifted by +s in the face's normal
        // direction from the carved voxel's NW-bottom corner.
        let (corners, normal) = match face.normal {
            // +X: face on the east side of the carved voxel, normal +X
            // (visible from inside the cave looking east).
            [1, 0, 0] => (
                [
                    [x0 + s, y0,     z0    ],
                    [x0 + s, y0,     z0 + s],
                    [x0 + s, y0 + s, z0    ],
                    [x0 + s, y0 + s, z0 + s],
                ],
                [1.0, 0.0, 0.0],
            ),
            // -X
            [-1, 0, 0] => (
                [
                    [x0,     y0,     z0 + s],
                    [x0,     y0,     z0    ],
                    [x0,     y0 + s, z0 + s],
                    [x0,     y0 + s, z0    ],
                ],
                [-1.0, 0.0, 0.0],
            ),
            // +Y (ceiling -- normal points up, visible from below)
            [0, 1, 0] => (
                [
                    [x0,     y0 + s, z0    ],
                    [x0 + s, y0 + s, z0    ],
                    [x0,     y0 + s, z0 + s],
                    [x0 + s, y0 + s, z0 + s],
                ],
                [0.0, 1.0, 0.0],
            ),
            // -Y (floor -- normal points down, visible from above)
            [0, -1, 0] => (
                [
                    [x0,     y0,     z0 + s],
                    [x0 + s, y0,     z0 + s],
                    [x0,     y0,     z0    ],
                    [x0 + s, y0,     z0    ],
                ],
                [0.0, -1.0, 0.0],
            ),
            // +Z (south wall, normal +Z)
            [0, 0, 1] => (
                [
                    [x0 + s, y0,     z0 + s],
                    [x0,     y0,     z0 + s],
                    [x0 + s, y0 + s, z0 + s],
                    [x0,     y0 + s, z0 + s],
                ],
                [0.0, 0.0, 1.0],
            ),
            // -Z (north wall, normal -Z)
            [0, 0, -1] => (
                [
                    [x0,     y0,     z0    ],
                    [x0 + s, y0,     z0    ],
                    [x0,     y0 + s, z0    ],
                    [x0 + s, y0 + s, z0    ],
                ],
                [0.0, 0.0, -1.0],
            ),
            _ => continue,
        };
        let base = geom.positions.len() as u32;
        geom.positions.extend_from_slice(&corners);
        for _ in 0..4 {
            geom.normals.push(normal);
            geom.colors.push(CAVE_WALL_COLOR);
        }
        geom.uvs.extend_from_slice(&[
            [0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0],
        ]);
        geom.indices.extend_from_slice(&[
            base, base + 2, base + 1,
            base + 1, base + 2, base + 3,
        ]);
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test append_cave_geometry`
Expected: both PASS.

- [ ] **Step 5: Integrate into `build_chunk_geometry`**

Open `src/world/terrain.rs`. Find `build_chunk_geometry` (currently around line 504). Find the very end of the function -- the line `ChunkGeometry { positions, normals, colors, uvs, indices }`. Just BEFORE that line, add:

```rust
    let mut geom = ChunkGeometry {
        positions,
        normals,
        colors,
        uvs,
        indices,
    };
    geom
```

Wait -- we need access to `VoxelLayer` from `build_chunk_geometry`, but the existing signature doesn't take it. Update the signature instead. The full change to `build_chunk_geometry`:

In `src/world/terrain.rs`, change the function signature from:

```rust
pub fn build_chunk_geometry(
    coord: ChunkCoord,
    terrain: &Terrain,
    noise: &WorldNoise,
) -> ChunkGeometry {
```

to:

```rust
pub fn build_chunk_geometry(
    coord: ChunkCoord,
    terrain: &Terrain,
    noise: &WorldNoise,
    voxel_layer: &super::voxel::VoxelLayer,
) -> ChunkGeometry {
```

Then in the function body, just BEFORE the final `ChunkGeometry { ... }` return statement, insert the cave-face append:

```rust
    let mut geom = ChunkGeometry {
        positions,
        normals,
        colors,
        uvs,
        indices,
    };
    if let (Some(chunk), Some(carved)) = (
        voxel_layer.chunks.get(&coord),
        voxel_layer.carved.get(&coord),
    ) {
        super::voxel::append_cave_geometry(chunk, carved, &mut geom);
    }
    geom
```

Replace the existing final return `ChunkGeometry { positions, normals, colors, uvs, indices }` with the block above (the block returns `geom` at the end implicitly).

- [ ] **Step 6: Update the caller in `regenerate_dirty_chunks`**

Still in `src/world/terrain.rs`, find `regenerate_dirty_chunks` (around line 751). It currently calls:

```rust
let geom = build_chunk_geometry(coord, &terrain, &noise);
```

Change the function's signature to accept `VoxelLayer` and pass it into the build call. Update the system signature near the top:

```rust
pub fn regenerate_dirty_chunks(
    mut commands: Commands,
    mut terrain: ResMut<Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain_material: Res<TerrainMaterial>,
    chunk_manager: Res<ChunkManager>,
    noise: Res<WorldNoise>,
    voxel_layer: Res<super::voxel::VoxelLayer>,
) {
```

Then update BOTH call sites of `build_chunk_geometry` inside the function (there are two -- one in the dirty pass, one in the color_dirty pass) to pass the new parameter:

```rust
let geom = build_chunk_geometry(coord, &terrain, &noise, &voxel_layer);
```

- [ ] **Step 7: Verify compile + tests**

Run: `cargo check 2>&1 | tail -10`
Expected: clean. The new parameter is auto-injected by Bevy's system param resolution.

Run: `cargo test 2>&1 | tail -10`
Expected: all tests still pass (no regressions). The new tests from Tasks 1--4 plus the previous 30 = 36 unit + 2 smoke = 38 total.

- [ ] **Step 8: Commit**

```bash
git add src/world/voxel.rs src/world/terrain.rs
git commit -m "feat(voxel): integrate cave-face geometry into chunk mesh build"
```

---

## Task 5: Bridge `VoxelLayer.dirty` into `Terrain.dirty`

**Files:**
- Modify: `src/world/voxel.rs` (add `bridge_voxel_dirty_to_terrain` system + tests for the system body)

This system runs each frame: every coord in `voxel_layer.dirty` gets copied into `terrain.dirty` and removed from `voxel_layer.dirty`. The existing `regenerate_dirty_chunks` then rebuilds those chunks' meshes (with the voxel cave faces appended via Task 4's integration).

- [ ] **Step 1: Write the failing test for the bridge**

The bridge is a Bevy system, so we test the body indirectly via a small helper function. Append to the `tests` module:

```rust
    use super::super::terrain::Terrain as TerrainResource;

    #[test]
    fn drain_voxel_dirty_into_terrain_dirty_moves_all_coords() {
        let mut layer = VoxelLayer::default();
        layer.dirty.insert((1, 2));
        layer.dirty.insert((3, 4));
        let mut terrain = TerrainResource::default();
        drain_voxel_dirty_into_terrain_dirty(&mut layer, &mut terrain);
        assert!(layer.dirty.is_empty());
        assert!(terrain.dirty.contains(&(1, 2)));
        assert!(terrain.dirty.contains(&(3, 4)));
    }

    #[test]
    fn drain_voxel_dirty_is_noop_when_empty() {
        let mut layer = VoxelLayer::default();
        let mut terrain = TerrainResource::default();
        drain_voxel_dirty_into_terrain_dirty(&mut layer, &mut terrain);
        assert!(terrain.dirty.is_empty());
    }
```

- [ ] **Step 2: Run to confirm fail**

Run: `cargo test drain_voxel_dirty`
Expected: FAIL -- `cannot find function`.

- [ ] **Step 3: Implement the helper + the system**

Add this free function and system in `src/world/voxel.rs` after `append_cave_geometry`:

```rust
/// Move every coord from `voxel_layer.dirty` into `terrain.dirty` and
/// clear `voxel_layer.dirty`. Pure-data helper so it's directly
/// testable; the wrapping Bevy system is one line.
pub fn drain_voxel_dirty_into_terrain_dirty(
    voxel_layer: &mut VoxelLayer,
    terrain: &mut Terrain,
) {
    for coord in voxel_layer.dirty.drain() {
        terrain.dirty.insert(coord);
    }
}

/// Bevy system: runs each frame, drains the voxel-dirty set into the
/// terrain-dirty set. The existing `regenerate_dirty_chunks` then
/// rebuilds the affected chunks' meshes with the new voxel cave
/// faces appended.
fn bridge_voxel_dirty_to_terrain(
    mut voxel_layer: ResMut<VoxelLayer>,
    mut terrain: ResMut<Terrain>,
) {
    drain_voxel_dirty_into_terrain_dirty(&mut voxel_layer, &mut terrain);
}
```

- [ ] **Step 4: Add the system to `VoxelPlugin`**

Find `impl Plugin for VoxelPlugin` in `src/world/voxel.rs`. The current systems tuple is `(fill_voxels_on_chunk_load, drop_voxels_on_chunk_unload)`. Add the bridge:

```rust
impl Plugin for VoxelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelLayer>()
            .add_systems(
                Update,
                (
                    fill_voxels_on_chunk_load,
                    drop_voxels_on_chunk_unload,
                    bridge_voxel_dirty_to_terrain,
                ),
            );
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test drain_voxel_dirty`
Expected: both PASS.

Run: `cargo test 2>&1 | tail -5`
Expected: all tests still pass, total now 38 unit + 2 smoke = 40.

- [ ] **Step 6: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): bridge VoxelLayer.dirty into Terrain.dirty for regen"
```

---

## Task 6: Debug shaft brush -- G key carves a cylinder under the cursor

**Files:**
- Modify: `src/world/edit.rs` (add a new system `apply_shaft_carve_debug` registered in `register`)

This is a debug feature. Press G with the brush mode active (T-toggle on) and the cursor over terrain: carves a vertical cylinder of radius 1m, depth 5m, downward from the cursor position into whatever voxel chunk is below.

- [ ] **Step 1: Add the system to `src/world/edit.rs`**

Open `src/world/edit.rs`. At the end of the file (after `draw_brush_preview`), add:

```rust
/// Debug hotkey: G carves a vertical cylinder of voxels under the
/// cursor. Radius 1m (= 2 voxels), depth 5m (= 10 voxels), starting
/// at the heightmap surface. No-op if the cursor is over a non-
/// highland chunk (no voxel storage = nothing to carve).
fn apply_shaft_carve_debug(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Res<crate::input::CursorState>,
    edit_mode: Res<EditMode>,
    mut voxel_layer: ResMut<crate::world::voxel::VoxelLayer>,
) {
    if !edit_mode.active || !keys.just_pressed(KeyCode::KeyG) {
        return;
    }
    let Some(world_pos) = cursor.cursor_world else {
        return;
    };
    use crate::world::voxel::{VOXEL_PER_CELL, VOXEL_SIZE, VOXELS_PER_CHUNK_SIDE};
    use crate::world::terrain::CHUNK_CELLS;

    let radius_voxels = 2i32;
    let depth_voxels = 10i32;

    // Cursor world XZ -> chunk + local voxel coordinates of the centre column.
    let centre_vx_world = (world_pos.x / VOXEL_SIZE).floor() as i32;
    let centre_vz_world = (world_pos.z / VOXEL_SIZE).floor() as i32;
    // Voxel ly at the cursor's surface Y. Carve downward from here.
    let top_ly = (world_pos.y / VOXEL_SIZE).floor() as i32;

    for dvz in -radius_voxels..=radius_voxels {
        for dvx in -radius_voxels..=radius_voxels {
            // Circular footprint.
            if dvx * dvx + dvz * dvz > radius_voxels * radius_voxels {
                continue;
            }
            let vx_world = centre_vx_world + dvx;
            let vz_world = centre_vz_world + dvz;
            // Map world voxel coord to (chunk, local).
            let voxels_per_chunk = (CHUNK_CELLS * VOXEL_PER_CELL as i32) as i32;
            let cx = vx_world.div_euclid(voxels_per_chunk);
            let cz = vz_world.div_euclid(voxels_per_chunk);
            let lx = vx_world.rem_euclid(voxels_per_chunk) as u8;
            let lz = vz_world.rem_euclid(voxels_per_chunk) as u8;
            debug_assert!((lx as usize) < VOXELS_PER_CHUNK_SIDE);
            debug_assert!((lz as usize) < VOXELS_PER_CHUNK_SIDE);
            for ly_offset in 0..depth_voxels {
                let ly = top_ly - ly_offset;
                if ly < 0 || ly >= crate::world::voxel::VOXEL_HEIGHT as i32 {
                    continue;
                }
                voxel_layer.carve((cx, cz), (lx, ly as u8, lz));
            }
        }
    }
    bevy::log::info!(
        "[voxel-debug] carved shaft at world ({:.1}, {:.1}, {:.1})",
        world_pos.x, world_pos.y, world_pos.z
    );
}
```

- [ ] **Step 2: Register the system in `register`**

Find `pub fn register(app: &mut App)` in `src/world/edit.rs`. The systems tuple includes the brush systems. Add `apply_shaft_carve_debug`:

```rust
pub fn register(app: &mut App) {
    app.init_resource::<EditMode>().add_systems(
        Update,
        (
            toggle_edit_mode,
            switch_brush,
            cycle_paint_biome,
            adjust_radius,
            apply_brush,
            apply_footprint_flatten,
            apply_shaft_carve_debug,
            draw_brush_preview,
        ),
    );
}
```

- [ ] **Step 3: Verify compile**

Run: `cargo check 2>&1 | tail -10`
Expected: clean. There may be a "function never used" warning on `apply_shaft_carve_debug` until the system runs at least once; the registration in step 2 resolves that.

- [ ] **Step 4: Commit**

```bash
git add src/world/edit.rs
git commit -m "feat(voxel): debug G-key carves vertical voxel shaft at cursor"
```

---

## Task 7: Manual playtest

This is a **required gate**.

- [ ] **Step 1: Boot the game**

Run: `cargo run` (from the worktree).
Expected: game launches without panic.

- [ ] **Step 2: Raise a tall mountain manually**

In the game:
1. Press T to enter edit mode.
2. Press 1 to select the Raise brush.
3. Find a Mountain biome cell (the gray-tan tiles you found earlier near `(-200, +500)` in your save). Or stay where you are and raise plain grassland -- the biome doesn't matter for the carve test, only the voxel layer does. Actually it DOES matter: only Mountain or Snow biome chunks have voxel storage. So either:
   - Walk to the existing Mountain biome region, OR
   - Use the Paint brush (key 5) to paint a chunk full of Mountain biome first, then raise it.

If staying near spawn: press 5 (Paint), press `[` until "Mountain" shows in the brush hotbar, paint a 5x5m square. Then press 1 (Raise) and hold LMB on the painted area for ~30 seconds to raise it to 8-10m tall.

- [ ] **Step 3: Carve a shaft into it**

1. Hover the cursor over the top of the raised mountain.
2. Press G.

Expected: a `[voxel-debug] carved shaft at world (...)` log line in the terminal. Visually, you should see a square-ish hole appear in the top of the mountain -- the cube-faced cavity. Walk the cat to the edge of the hole and look down: you should see voxel cube walls forming a shaft going down into the mountain.

- [ ] **Step 4: Drop in (optional)**

Walk the cat over the hole and let gravity pull it down into the shaft. The cat should fall into the cavity and stand on the floor. Cube walls visible all around.

If the cat gets stuck at the lip of the shaft, the trimesh collider may not be regenerating correctly with the cave faces. Note this as a Stage 3 V1 issue -- V0's task is the visual mesh, not the collider integration. (The collider rebuild happens in `regenerate_dirty_chunks` and uses the same `geom` we appended to, so it SHOULD include the cave faces. If it doesn't, it's a debug-worthy regression.)

- [ ] **Step 5: Confirm working & report**

Once you've seen the shaft visually appear and (optionally) walked into it, report what you saw in the next session.

If something looks wrong (cat falls through the floor, walls invisible from the inside, weird Z-fighting), note it. V1 will need to address those.

---

## Task 8: Final review and commit cleanup

**Files:** none (review-only).

- [ ] **Step 1: Run the full test suite**

Run: `cargo test 2>&1 | tail -10`
Expected: 40 unit + 2 smoke = 42 tests pass.

- [ ] **Step 2: Run cargo check for warnings**

Run: `cargo check 2>&1 | grep -E "^warning" | wc -l`
Expected: 46 ± 2 warnings (pre-existing). No new warnings from V0 work.

- [ ] **Step 3: Push and open PR**

```bash
git push -u origin feat/voxel-caves-v1
gh pr create --base main --head feat/voxel-caves-v1 \
  --title "feat(voxel): caves V0, debug-carved cavities (DEC-024 stage 3)" \
  --body "$(cat <<'EOF'
## Summary

V0 of the voxel cave system (DEC-024 Stage 3, scoped down). Adds a carve API on `VoxelLayer`, a cave-face mesh emitter, integration into the existing chunk mesh + collider build, and a debug G-key hotkey that carves a vertical voxel shaft at the cursor.

**Visible payoff:** raise a Mountain biome chunk with the brush, hover the cursor over the top, press G. A cube-faced cylindrical shaft appears in the mountain. The cat can drop into it.

## What ships

- `VoxelLayer.dirty: HashSet<ChunkCoord>` and `VoxelLayer.carved: HashMap<ChunkCoord, HashSet<VoxelLocal>>` fields (forward-compat with the spec)
- `VoxelLayer::carve(coord, local)` API: idempotent, no-op on already-air or unloaded chunks, marks dirty + records in carved
- `emit_cave_faces` + `append_cave_geometry` in voxel.rs: emit cube faces only at carved-air vs. solid-voxel boundaries (no double-rendering with the heightmap mesher)
- `bridge_voxel_dirty_to_terrain` system: drains voxel-dirty into terrain-dirty each frame, the existing chunk regen path handles the rest
- `apply_shaft_carve_debug` in edit.rs: G key in edit mode carves a 1m-radius x 5m-deep cylinder under the cursor

## Scope deviation from the spec

The spec's Stage 3 calls for a full PCG alpine generator + crystal content + lighting. V0 ships only the carving substrate + visualization. Rationale documented in the plan: PCG generation builds on top of carving, not under it. Without carving + meshing, there's nothing to test.

V1 will add: PCG cave generator, side-face entrances, crystal content, glow lighting, ambient mask, mountain noise tuning. V1's plan will be much smaller now that the substrate exists.

## Verification

- 12 new inline tests across Tasks 1-5 (40 unit + 2 smoke = 42 total)
- `cargo check` clean, no new warnings
- Manual playtest: G key carves visible voxel shaft into a brush-raised mountain
- Per-task spec compliance + code quality reviews via subagent-driven-development
- Final whole-branch review by superpowers:code-reviewer

## Test plan

- [x] `cargo test` (42/42 pass)
- [x] `cargo check` (no new warnings)
- [x] Manual playtest: shaft carving works visually
- [ ] Eyeball the diff before clicking merge

## References

- Plan: `docs/superpowers/plans/2026-05-02-voxel-caves-v0-debug-carve.md`
- Spec: `docs/superpowers/specs/2026-05-02-voxel-mountain-caves-design.md`
- ADR: DEC-024
- Stage 1 (storage substrate): merged in PR #2

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Done criteria

- All 40 unit + 2 smoke tests pass: `cargo test`
- `cargo check` clean (no NEW warnings)
- Manual playtest: G key carves a visible cube-faced shaft into a raised mountain; cat can drop into the shaft (collider working correctly is a bonus, not a hard requirement at V0)
- Branch pushed, PR opened

## Hand-off to V1

V1 of the cave system will add:
- PCG alpine cave generator (3D Perlin chambers + worm tunnel connectors)
- Visible side-face entrances on mountain faces (not just vertical shafts)
- Crystal voxel content + glow material + per-crystal point lights
- Ambient cave mask coupled with the torch's `DarknessFactor`
- Cross-chunk cave coherence (the V0 carve API only operates within one chunk per call site; PCG chambers may span 2-3 chunks)
- Trimesh collider correctness verification (V0 may have edge cases at the chunk boundary; V1 must fix any that surface during V0 playtest)
- DEBT-027 fix (chain `fill_voxels_on_chunk_load` after `load_nearby_chunks` in WorldPlugin)
- Tune mountain noise amplification so PCG mountains have room for caves without manual brush-raising

V1 reads the V0 carve API and mesher unchanged. The only carve-side change is a new caller (the PCG generator) instead of the debug brush.
