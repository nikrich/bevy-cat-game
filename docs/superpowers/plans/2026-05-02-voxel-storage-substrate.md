# Voxel Storage Substrate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the voxel storage substrate (`VoxelLayer` resource, per-chunk bit-packed `VoxelChunk`, heightmap → voxel fill on chunk load) without changing rendering or physics. The game looks and behaves identically; voxels are populated internally for later cave-carving stages to read.

**Architecture:** New `src/world/voxel.rs` module owns the storage and lifecycle. A new `VoxelPlugin` listens for the existing `ChunkLoaded` event and a new `ChunkUnloaded` event (added to `chunks.rs`). For each highland chunk (any chunk containing a Mountain or Snow biome cell), the plugin allocates a `VoxelChunk` and fills its 2×2 voxel columns solid up to `floor(heightmap_y / 0.5)` per cell. Non-highland chunks get no voxel storage.

**Tech Stack:** Bevy 0.18, no new dependencies. Bit-packed storage uses `Vec<u64>` with manual bit ops (no `bitvec` crate needed for V1).

**Reference:** See `docs/superpowers/specs/2026-05-02-voxel-mountain-caves-design.md` for the full design spec, `DEC-024` in `.claude/memory/decisions.md` for the ADR. This plan implements the spec's Stage 1 with one deviation noted below.

---

## Scope deviation from the spec

The spec describes Stage 1 as "voxel storage + mesher + collider, renders identically to existing mountains." This plan ships **storage + lifecycle only** — no mesher, no collider changes, no renderer changes. Rationale:

- The mesher only earns its keep when there is a voxel-air pattern that the heightmap cannot represent. That pattern arrives in Stage 2 with carving.
- "Renders identically to existing mountains" is trivially achieved by leaving the renderer alone.
- Validating storage in isolation has clearer success criteria (round-trip tests, fill correctness) than validating "voxel mesher output ≈ heightmap mesher output" (subject to subtle visual diffs).
- Stage 2's mesher work is structurally simpler when carving and the mesher land together — the mesher has a real reason to exist.

If a Stage 2 implementation discovers a storage-side issue, fixing it lives there. Stage 1 stays minimal.

---

## Testing approach

Project is bin-only (`src/main.rs`, no `src/lib.rs`). Per the existing `2026-04-30-decoration-mode-split.md` plan and `src/decoration/physics.rs`:

- **Pure functions and storage operations** get unit tests in inline `#[cfg(test)] mod tests` blocks within `src/world/voxel.rs`. Run with `cargo test`.
- **Plugin / system / lifecycle work** is verified by `cargo check` (compile clean) + a manual playtest checkpoint at the end (run, walk into a mountain region, verify nothing visually regressed).

`cargo test` runs all inline `#[cfg(test)]` modules in the binary plus the `tests/smoke.rs` integration test. The bin-only crate has no `--lib` target, so the plain `cargo test` invocation is what every task in this plan uses.

---

## File structure (target)

```
src/
  world/
    voxel.rs       # NEW — VoxelLayer resource, VoxelChunk storage, fill from heightmap, VoxelPlugin
    chunks.rs      # MODIFIED — add ChunkUnloaded event, emit it from unload_distant_chunks
    mod.rs         # MODIFIED — register VoxelPlugin in WorldPlugin
```

Module visibility: `voxel.rs` is `pub mod voxel` from `world/mod.rs`. `VoxelLayer`, `VoxelChunk`, `VoxelLocal`, and the constants are `pub`. Internal storage methods stay private.

---

## Task 1: Scaffold `src/world/voxel.rs` with constants and stub types

**Files:**
- Create: `src/world/voxel.rs`
- Modify: `src/world/mod.rs:1-15` (add `pub mod voxel;`)

- [ ] **Step 1: Create the new module file with constants and a minimal `VoxelChunk` shell**

Create `src/world/voxel.rs`:

```rust
//! Voxel storage substrate (Worldcraft Expansion / DEC-024 Stage 1).
//!
//! Adds a sparse per-chunk voxel layer for highland chunks (any chunk
//! containing a Mountain or Snow biome cell). Voxels are 0.5m cubes
//! arranged on a 2×2 sub-grid inside each 1m heightmap cell, stacking
//! up to `VOXEL_HEIGHT` units tall. Storage is dense per allocated
//! chunk: a 64×64×60 bit-packed grid totalling ~30KB per chunk.
//!
//! Stage 1 ships storage + heightmap → voxel fill on chunk load only.
//! No mesher, no collider, no renderer changes. Stage 2 introduces
//! carving and the voxel mesher when there is voxel-air content the
//! heightmap cannot represent.

use bevy::prelude::*;
use std::collections::HashMap;

use super::biome::Biome;
use super::chunks::ChunkLoaded;
use super::terrain::{ChunkCoord, Terrain, CHUNK_CELLS};

/// Voxels per heightmap cell side. 1m cell × 2 voxels = 0.5m voxels.
pub const VOXEL_PER_CELL: usize = 2;
/// World-space side length of a single voxel.
pub const VOXEL_SIZE: f32 = 0.5;
/// Maximum voxel column height. 60 voxels × 0.5m = 30m of mountain.
pub const VOXEL_HEIGHT: usize = 60;
/// Voxels per chunk side. 32 cells × 2 voxels = 64.
pub const VOXELS_PER_CHUNK_SIDE: usize = (CHUNK_CELLS as usize) * VOXEL_PER_CELL;
/// Total voxels per chunk: 64 × 64 × 60 = 245_760 bits ≈ 30 KB.
pub const VOXELS_PER_CHUNK: usize = VOXELS_PER_CHUNK_SIDE * VOXELS_PER_CHUNK_SIDE * VOXEL_HEIGHT;
/// Storage word size in bits.
const BITS_PER_WORD: usize = u64::BITS as usize;

/// Local voxel coordinate inside a `VoxelChunk`. `(lx, ly, lz)` with
/// `lx, lz ∈ 0..VOXELS_PER_CHUNK_SIDE` and `ly ∈ 0..VOXEL_HEIGHT`.
pub type VoxelLocal = (u8, u8, u8);

/// One chunk's worth of voxel data, bit-packed solid (1) / air (0).
/// Layout: `bit_index = ly * VOXELS_PER_CHUNK_SIDE * VOXELS_PER_CHUNK_SIDE
/// + lz * VOXELS_PER_CHUNK_SIDE + lx`.
pub struct VoxelChunk {
    bits: Vec<u64>,
}

/// Resource holding voxel chunks for highland chunks. Non-highland
/// chunks are absent from the map (not stored as empty).
#[derive(Resource, Default)]
pub struct VoxelLayer {
    pub chunks: HashMap<ChunkCoord, VoxelChunk>,
}
```

- [ ] **Step 2: Wire the module into `world/mod.rs`**

Open `src/world/mod.rs` and find the existing module declarations near the top (look for `pub mod terrain;`). Add `pub mod voxel;` directly after `pub mod terrain;` so it can use the terrain types.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check`
Expected: clean build, no warnings about the new module (it has no dead code yet because constants and types are `pub`).

- [ ] **Step 4: Commit**

```bash
git add src/world/voxel.rs src/world/mod.rs
git commit -m "feat(voxel): scaffold storage module with constants and types"
```

---

## Task 2: Implement bit-packed get/set with tests

**Files:**
- Modify: `src/world/voxel.rs` (add impl block + tests after the `VoxelLayer` definition)

- [ ] **Step 1: Write the failing test for round-trip get/set**

Append at the bottom of `src/world/voxel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voxel_chunk_round_trip_set_get() {
        let mut chunk = VoxelChunk::empty();
        assert!(!chunk.get((0, 0, 0)));

        chunk.set((1, 2, 3), true);
        assert!(chunk.get((1, 2, 3)));
        assert!(!chunk.get((1, 2, 4)));
        assert!(!chunk.get((0, 0, 0)));

        chunk.set((1, 2, 3), false);
        assert!(!chunk.get((1, 2, 3)));
    }
}
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cargo test voxel_chunk_round_trip_set_get`
Expected: FAIL with `no associated item named empty / get / set found for struct VoxelChunk`.

- [ ] **Step 3: Implement `VoxelChunk::empty / get / set`**

Insert this `impl VoxelChunk` block immediately after the `VoxelChunk` struct definition (and before the `VoxelLayer` struct), in `src/world/voxel.rs`:

```rust
impl VoxelChunk {
    /// Allocate a chunk with all voxels air.
    pub fn empty() -> Self {
        let words = VOXELS_PER_CHUNK.div_ceil(BITS_PER_WORD);
        Self {
            bits: vec![0u64; words],
        }
    }

    #[inline]
    fn bit_index((lx, ly, lz): VoxelLocal) -> usize {
        debug_assert!((lx as usize) < VOXELS_PER_CHUNK_SIDE);
        debug_assert!((lz as usize) < VOXELS_PER_CHUNK_SIDE);
        debug_assert!((ly as usize) < VOXEL_HEIGHT);
        let lx = lx as usize;
        let ly = ly as usize;
        let lz = lz as usize;
        ly * VOXELS_PER_CHUNK_SIDE * VOXELS_PER_CHUNK_SIDE + lz * VOXELS_PER_CHUNK_SIDE + lx
    }

    /// `true` if the voxel at `local` is solid.
    pub fn get(&self, local: VoxelLocal) -> bool {
        let bi = Self::bit_index(local);
        let word = self.bits[bi / BITS_PER_WORD];
        (word >> (bi % BITS_PER_WORD)) & 1 == 1
    }

    /// Set the voxel at `local` to solid (`true`) or air (`false`).
    pub fn set(&mut self, local: VoxelLocal, solid: bool) {
        let bi = Self::bit_index(local);
        let word = &mut self.bits[bi / BITS_PER_WORD];
        let mask = 1u64 << (bi % BITS_PER_WORD);
        if solid {
            *word |= mask;
        } else {
            *word &= !mask;
        }
    }
}
```

- [ ] **Step 4: Run the test to confirm it passes**

Run: `cargo test voxel_chunk_round_trip_set_get`
Expected: PASS.

- [ ] **Step 5: Add a stress test for full coverage**

Append to the `tests` module:

```rust
    #[test]
    fn voxel_chunk_corners_and_extremes() {
        let mut chunk = VoxelChunk::empty();
        let corners: &[VoxelLocal] = &[
            (0, 0, 0),
            ((VOXELS_PER_CHUNK_SIDE - 1) as u8, 0, 0),
            (0, (VOXEL_HEIGHT - 1) as u8, 0),
            (0, 0, (VOXELS_PER_CHUNK_SIDE - 1) as u8),
            (
                (VOXELS_PER_CHUNK_SIDE - 1) as u8,
                (VOXEL_HEIGHT - 1) as u8,
                (VOXELS_PER_CHUNK_SIDE - 1) as u8,
            ),
        ];
        for &c in corners {
            chunk.set(c, true);
        }
        for &c in corners {
            assert!(chunk.get(c), "corner {:?} should be solid", c);
        }
        // Adjacent voxels stay air.
        assert!(!chunk.get((1, 0, 0)));
        assert!(!chunk.get((0, 1, 0)));
        assert!(!chunk.get((0, 0, 1)));
    }
```

- [ ] **Step 6: Run the new test**

Run: `cargo test voxel_chunk_corners_and_extremes`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): bit-packed VoxelChunk get/set with tests"
```

---

## Task 3: Add `set_solid_column` for fast column fills

**Files:**
- Modify: `src/world/voxel.rs` (extend `impl VoxelChunk`, add tests)

- [ ] **Step 1: Write the failing test**

Append to the `tests` module:

```rust
    #[test]
    fn set_solid_column_fills_y_zero_to_max_exclusive() {
        let mut chunk = VoxelChunk::empty();
        chunk.set_solid_column(5, 7, 4);
        // Voxels 0..4 solid.
        for ly in 0..4 {
            assert!(chunk.get((5, ly, 7)), "ly={ly} should be solid");
        }
        // Voxel at ly=4 is air (exclusive upper bound).
        assert!(!chunk.get((5, 4, 7)));
        // Other columns untouched.
        assert!(!chunk.get((6, 0, 7)));
        assert!(!chunk.get((5, 0, 8)));
    }

    #[test]
    fn set_solid_column_zero_height_is_noop() {
        let mut chunk = VoxelChunk::empty();
        chunk.set_solid_column(5, 7, 0);
        for ly in 0..VOXEL_HEIGHT as u8 {
            assert!(!chunk.get((5, ly, 7)));
        }
    }

    #[test]
    fn set_solid_column_clamps_to_voxel_height() {
        let mut chunk = VoxelChunk::empty();
        chunk.set_solid_column(0, 0, (VOXEL_HEIGHT + 10) as u8);
        for ly in 0..VOXEL_HEIGHT as u8 {
            assert!(chunk.get((0, ly, 0)));
        }
    }
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test set_solid_column`
Expected: FAIL (`no method named set_solid_column found`).

- [ ] **Step 3: Implement `set_solid_column`**

Add the following method inside the existing `impl VoxelChunk` block in `src/world/voxel.rs`, after `set`:

```rust
    /// Mark `(lx, lz)`'s voxel column solid for `ly ∈ 0..max_ly`. Higher
    /// voxels are left as-is (air on a fresh chunk). `max_ly` is clamped
    /// to `VOXEL_HEIGHT`.
    pub fn set_solid_column(&mut self, lx: u8, lz: u8, max_ly: u8) {
        let max_ly = (max_ly as usize).min(VOXEL_HEIGHT) as u8;
        for ly in 0..max_ly {
            self.set((lx, ly, lz), true);
        }
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test set_solid_column`
Expected: all three PASS.

- [ ] **Step 5: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): set_solid_column for heightmap fill"
```

---

## Task 4: Highland biome detection

**Files:**
- Modify: `src/world/voxel.rs` (add helpers + tests)

- [ ] **Step 1: Write failing tests**

Append to the `tests` module:

```rust
    #[test]
    fn is_highland_recognises_mountain_and_snow() {
        assert!(is_highland_biome(Biome::Mountain));
        assert!(is_highland_biome(Biome::Snow));
    }

    #[test]
    fn is_highland_rejects_lowland_biomes() {
        for b in [
            Biome::Ocean,
            Biome::Beach,
            Biome::Desert,
            Biome::Grassland,
            Biome::Meadow,
            Biome::Forest,
            Biome::Taiga,
            Biome::Tundra,
        ] {
            assert!(!is_highland_biome(b), "{:?} is not highland", b);
        }
    }
```

- [ ] **Step 2: Run to confirm fail**

Run: `cargo test is_highland`
Expected: FAIL (`is_highland_biome` not in scope).

- [ ] **Step 3: Implement the helper**

Add this free function in `src/world/voxel.rs` between the `impl VoxelChunk` block and the `VoxelLayer` struct definition:

```rust
/// `true` for biomes that are part of a mountain (high-elevation rock).
/// Mountain and Snow are the only biomes that exist above
/// `MOUNTAIN_LEVEL` in the world generator (see `world::biome`).
pub fn is_highland_biome(b: Biome) -> bool {
    matches!(b, Biome::Mountain | Biome::Snow)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test is_highland`
Expected: both PASS.

- [ ] **Step 5: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): highland biome detection (Mountain | Snow)"
```

---

## Task 5: Build a voxel chunk from terrain heightmap

**Files:**
- Modify: `src/world/voxel.rs` (add the fill function + tests)

- [ ] **Step 1: Write the failing test**

Append to the `tests` module. This test does NOT use the live `Terrain` resource; it constructs a synthetic `ChunkData` and inserts it into a `Terrain`, then asserts on the resulting voxel chunk:

```rust
    use super::super::terrain::{ChunkData, CHUNK_VERTS};

    fn highland_terrain_with_one_cell() -> Terrain {
        let mut t = Terrain::default();
        let mut data = ChunkData::empty();
        // Mark cell (3, 7) as Mountain at heightmap_y = 6.25.
        let i = ChunkData::idx(3, 7);
        data.heights[i] = 6.25;
        data.biomes[i] = Biome::Mountain;
        t.chunks.insert((0, 0), data);
        t
    }

    #[test]
    fn build_voxel_chunk_returns_some_for_highland_chunk() {
        let terrain = highland_terrain_with_one_cell();
        let voxel = build_voxel_chunk_for_coord((0, 0), &terrain);
        assert!(voxel.is_some());
    }

    #[test]
    fn build_voxel_chunk_returns_none_for_lowland_chunk() {
        let mut terrain = Terrain::default();
        let mut data = ChunkData::empty();
        for i in 0..CHUNK_VERTS * CHUNK_VERTS {
            data.heights[i] = 0.5;
            data.biomes[i] = Biome::Grassland;
        }
        terrain.chunks.insert((0, 0), data);
        assert!(build_voxel_chunk_for_coord((0, 0), &terrain).is_none());
    }

    #[test]
    fn build_voxel_chunk_fills_highland_cell_column_to_floor_of_height_over_voxel_size() {
        let terrain = highland_terrain_with_one_cell();
        let voxel = build_voxel_chunk_for_coord((0, 0), &terrain).unwrap();
        // Cell (3, 7) maps to voxel columns at lx ∈ {6, 7}, lz ∈ {14, 15}.
        // heightmap_y = 6.25 → max_ly = floor(6.25 / 0.5) = 12.
        for lx in 6..=7u8 {
            for lz in 14..=15u8 {
                for ly in 0..12u8 {
                    assert!(voxel.get((lx, ly, lz)),
                        "highland cell voxel {:?} should be solid", (lx, ly, lz));
                }
                // Voxels above the cap are air.
                assert!(!voxel.get((lx, 12, lz)));
                assert!(!voxel.get((lx, 13, lz)));
            }
        }
    }

    #[test]
    fn build_voxel_chunk_leaves_lowland_columns_empty() {
        let terrain = highland_terrain_with_one_cell();
        let voxel = build_voxel_chunk_for_coord((0, 0), &terrain).unwrap();
        // Cell (0, 0) is the default Grassland, height 0.0 — its voxel
        // columns at lx ∈ {0, 1}, lz ∈ {0, 1} stay all-air.
        for lx in 0..=1u8 {
            for lz in 0..=1u8 {
                for ly in 0..12u8 {
                    assert!(!voxel.get((lx, ly, lz)));
                }
            }
        }
    }
```

- [ ] **Step 2: Run to confirm fail**

Run: `cargo test build_voxel_chunk`
Expected: FAIL (`build_voxel_chunk_for_coord` not in scope).

- [ ] **Step 3: Implement `build_voxel_chunk_for_coord`**

Add this free function in `src/world/voxel.rs` after `is_highland_biome`:

```rust
/// Build a `VoxelChunk` for `coord` by scanning the chunk's heightmap
/// in `terrain` and filling voxels under each highland cell. Returns
/// `None` if the chunk has no highland cells (no voxel storage needed)
/// or the chunk isn't loaded.
pub fn build_voxel_chunk_for_coord(coord: ChunkCoord, terrain: &Terrain) -> Option<VoxelChunk> {
    let data = terrain.chunks.get(&coord)?;
    let mut any_highland = false;
    let mut chunk = VoxelChunk::empty();
    for lz in 0..CHUNK_CELLS as usize {
        for lx in 0..CHUNK_CELLS as usize {
            let i = lz * super::terrain::CHUNK_VERTS + lx;
            let biome = data.biomes[i];
            if !is_highland_biome(biome) {
                continue;
            }
            any_highland = true;
            let h = data.heights[i];
            // Only fill if the cell sits above the world origin. Cells
            // with non-positive heights would map to max_ly = 0 and
            // contribute nothing; skip them rather than risk a negative
            // intermediate.
            if h <= 0.0 {
                continue;
            }
            let max_ly = (h / VOXEL_SIZE).floor() as i32;
            if max_ly <= 0 {
                continue;
            }
            let max_ly = (max_ly as usize).min(VOXEL_HEIGHT) as u8;
            // Each heightmap cell owns a 2×2 voxel sub-grid in XZ.
            let base_vx = (lx * VOXEL_PER_CELL) as u8;
            let base_vz = (lz * VOXEL_PER_CELL) as u8;
            for dx in 0..VOXEL_PER_CELL as u8 {
                for dz in 0..VOXEL_PER_CELL as u8 {
                    chunk.set_solid_column(base_vx + dx, base_vz + dz, max_ly);
                }
            }
        }
    }
    if any_highland {
        Some(chunk)
    } else {
        None
    }
}
```

Note the index expression: the existing `ChunkData` stores `CHUNK_VERTS × CHUNK_VERTS` entries (not `CHUNK_CELLS × CHUNK_CELLS`), addressed via `lz * CHUNK_VERTS + lx`. We iterate cells `0..CHUNK_CELLS` but index with `CHUNK_VERTS` row stride.

- [ ] **Step 4: Run all four tests**

Run: `cargo test build_voxel_chunk`
Expected: all four PASS.

- [ ] **Step 5: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): build_voxel_chunk_for_coord fills highland columns"
```

---

## Task 6: Add a `ChunkUnloaded` event in `chunks.rs`

The voxel layer needs to release `VoxelChunk` storage when terrain chunks unload. Today there is a `ChunkLoaded` event but no unload counterpart. Add it.

**Files:**
- Modify: `src/world/chunks.rs:39-46` (add new event next to `ChunkLoaded`)
- Modify: `src/world/chunks.rs:106-127` (emit it from `unload_distant_chunks`)

- [ ] **Step 1: Add the event type**

In `src/world/chunks.rs`, find this block (around line 40):

```rust
#[derive(Message)]
pub struct ChunkLoaded {
    pub x: i32,
    pub z: i32,
    pub entity: Entity,
}
```

Add the unload event immediately after it:

```rust
#[derive(Message)]
pub struct ChunkUnloaded {
    pub x: i32,
    pub z: i32,
}
```

- [ ] **Step 2: Emit the event from `unload_distant_chunks`**

In `src/world/chunks.rs`, find `unload_distant_chunks`. Change its signature to take a `MessageWriter<ChunkUnloaded>` and emit one event per unloaded chunk:

```rust
pub fn unload_distant_chunks(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut terrain: ResMut<Terrain>,
    mut chunk_events: MessageWriter<ChunkUnloaded>,
) {
    let (cx, cz) = chunk_manager.player_chunk;
    let unload_distance = RENDER_DISTANCE + 2;

    let to_unload: Vec<(i32, i32)> = chunk_manager
        .loaded
        .keys()
        .filter(|(x, z)| (x - cx).abs() > unload_distance || (z - cz).abs() > unload_distance)
        .copied()
        .collect();

    for coord in to_unload {
        if let Some(entity) = chunk_manager.loaded.remove(&coord) {
            commands.entity(entity).despawn();
        }
        terrain.unload_chunk(coord);
        chunk_events.write(ChunkUnloaded {
            x: coord.0,
            z: coord.1,
        });
    }
}
```

- [ ] **Step 3: Register the event in `world::WorldPlugin`**

Open `src/world/mod.rs`. Find line 25:

```rust
.add_message::<chunks::ChunkLoaded>()
```

Add the unload registration immediately after it (still inside the chain):

```rust
.add_message::<chunks::ChunkLoaded>()
.add_message::<chunks::ChunkUnloaded>()
```

No import change needed — both events are accessed via the `chunks::` path.

- [ ] **Step 4: Verify compile**

Run: `cargo check`
Expected: clean build. If you see "trait `Message` is not implemented", confirm `chunks.rs` already derives `Message` for `ChunkLoaded` (it does on the original line 40) and that you used the same derive on `ChunkUnloaded`.

- [ ] **Step 5: Commit**

```bash
git add src/world/chunks.rs src/world/mod.rs
git commit -m "feat(world): emit ChunkUnloaded event from unload_distant_chunks"
```

---

## Task 7: `VoxelPlugin` — fill on load, drop on unload

**Files:**
- Modify: `src/world/voxel.rs` (add plugin + systems)

- [ ] **Step 1: Add the plugin and systems**

Append to `src/world/voxel.rs`, after `build_voxel_chunk_for_coord` and before the `#[cfg(test)] mod tests` block:

```rust
/// Plugin that registers the [`VoxelLayer`] resource and keeps it in sync
/// with the chunk lifecycle. Listens to [`ChunkLoaded`] to populate
/// voxels for highland chunks and to [`super::chunks::ChunkUnloaded`]
/// to release them.
pub struct VoxelPlugin;

impl Plugin for VoxelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoxelLayer>()
            .add_systems(Update, (fill_voxels_on_chunk_load, drop_voxels_on_chunk_unload));
    }
}

fn fill_voxels_on_chunk_load(
    mut events: MessageReader<ChunkLoaded>,
    terrain: Res<Terrain>,
    mut voxel_layer: ResMut<VoxelLayer>,
) {
    for ev in events.read() {
        let coord = (ev.x, ev.z);
        if let Some(chunk) = build_voxel_chunk_for_coord(coord, &terrain) {
            voxel_layer.chunks.insert(coord, chunk);
        }
    }
}

fn drop_voxels_on_chunk_unload(
    mut events: MessageReader<super::chunks::ChunkUnloaded>,
    mut voxel_layer: ResMut<VoxelLayer>,
) {
    for ev in events.read() {
        voxel_layer.chunks.remove(&(ev.x, ev.z));
    }
}
```

- [ ] **Step 2: Verify compile**

Run: `cargo check`
Expected: clean build. If you see unused-import warnings on `ChunkLoaded`, that's fine if the import is referenced inside the plugin function body — it should be.

- [ ] **Step 3: Commit**

```bash
git add src/world/voxel.rs
git commit -m "feat(voxel): VoxelPlugin populates layer from chunk lifecycle"
```

---

## Task 8: Wire `VoxelPlugin` into `WorldPlugin`

**Files:**
- Modify: `src/world/mod.rs` (register `VoxelPlugin`)

- [ ] **Step 1: Register the plugin**

Open `src/world/mod.rs`. The current `WorldPlugin::build` ends (around line 57-60) with:

```rust
                ),
            );
        edit::register(app);
        edit_egui::register(app);
    }
}
```

Add the voxel plugin registration immediately after `edit_egui::register(app);` and before the closing braces:

```rust
                ),
            );
        edit::register(app);
        edit_egui::register(app);
        app.add_plugins(voxel::VoxelPlugin);
    }
}
```

This places voxel registration last, so the resource and systems are available to anything that runs after `WorldPlugin` (none currently depend on it, but it preserves the convention of registering plugins after their dependencies' resources).

- [ ] **Step 2: Verify compile and run tests**

Run: `cargo check && cargo test`
Expected: clean compile. All voxel tests still pass.

- [ ] **Step 3: Commit**

```bash
git add src/world/mod.rs
git commit -m "feat(voxel): register VoxelPlugin in WorldPlugin"
```

---

## Task 9: Manual playtest checkpoint

This task is a **required gate**. The substrate is invisible to the player by design, so the verification is a behavioural checklist, not a visual one.

- [ ] **Step 1: Boot the game**

Run: `cargo run`
Expected: the game starts. No new panics. No new warnings about missing resources or unregistered events.

- [ ] **Step 2: Walk into a mountain region and back**

Use WASD to walk toward a mountain biome (Mountain or Snow tiles). Walk past it, far enough that the original spawn chunk unloads, then walk back. Make at least one of these moves cross into a 5×5 chunk window's worth of distance.

Expected: no panics, no frame stalls, no visible terrain regression.

- [ ] **Step 3: Confirm voxel storage is populated**

This step needs a tiny debug print to confirm the substrate is alive. Add this temporarily inside `fill_voxels_on_chunk_load` (in `src/world/voxel.rs`), at the end of the function body:

```rust
    if !voxel_layer.chunks.is_empty() {
        bevy::log::info!(
            "VoxelLayer holds {} chunks",
            voxel_layer.chunks.len()
        );
    }
```

Run `cargo run` again. Walk into / past a mountain region. Look for the log line in the terminal — the count should increase as highland chunks load and decrease (or hold steady, since this print only fires on load) as they unload.

Expected: the log fires at least once when a Mountain or Snow chunk loads.

- [ ] **Step 4: Remove the debug print**

Take the debug print out of `fill_voxels_on_chunk_load`. Re-run `cargo check` to confirm clean build.

- [ ] **Step 5: Final commit**

If you didn't already commit the debug print, make sure no debug code remains. Otherwise:

```bash
git add src/world/voxel.rs
git commit -m "chore(voxel): drop temporary debug log after substrate verification"
```

---

## Done criteria

- All inline tests pass: `cargo test`
- Game compiles clean: `cargo check`
- Game runs without panics: `cargo run`, exit cleanly
- Manual playtest verified that voxel chunks populate when the player approaches highland regions and release when those regions unload
- No visual or behavioural regression in non-mountain areas (terrain editing, building, decoration, props all unchanged)

## Hand-off to Stage 2

Stage 2 of the cave spec adds:
- `VoxelLayer.carved` overlay tracking player-induced voxel removal
- Brush integration: when `Lower` brush brings heightmap_y across a 0.5m boundary in a highland cell, voxels above the new cap are marked carved
- Sinkhole detection event
- Voxel mesher (cube-faced) and combined heightmap+voxel collider
- Save format extension: `voxel_carved` key in `savegame.json`

Stage 2 will read from the `VoxelLayer` resource this stage establishes — no API changes from stage 1 are needed; only additions.
