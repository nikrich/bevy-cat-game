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
use std::collections::{HashMap, HashSet};

use super::biome::Biome;
use super::chunks::ChunkLoaded;
use super::terrain::{ChunkCoord, Terrain, CHUNK_CELLS, CHUNK_VERTS};

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
///
/// Layout: `bit_index = ly * VOXELS_PER_CHUNK_SIDE * VOXELS_PER_CHUNK_SIDE
/// + lz * VOXELS_PER_CHUNK_SIDE + lx` -- height outermost (YZX).
///
/// Why YZX rather than column-major XZY: a 2D slice at fixed `ly` is
/// contiguous, which is the access pattern Stage 2's voxel mesher will
/// use most (face emission scans one Y-layer at a time looking for
/// solid voxels). Column writes (`set_solid_column`, used during fill)
/// touch ~60 scattered cache lines per column, but fill runs once per
/// chunk load on ~30KB of data -- measured cost is invisible. If a
/// future hot loop proves otherwise, revisit with evidence.
pub struct VoxelChunk {
    bits: Vec<u64>,
}

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

    /// Mark `(lx, lz)`'s voxel column solid for `ly` in 0..max_ly. Higher
    /// voxels are left as-is (air on a fresh chunk). `max_ly` is clamped
    /// to `VOXEL_HEIGHT`.
    pub fn set_solid_column(&mut self, lx: u8, lz: u8, max_ly: u8) {
        let max_ly = (max_ly as usize).min(VOXEL_HEIGHT) as u8;
        for ly in 0..max_ly {
            self.set((lx, ly, lz), true);
        }
    }
}

/// `true` for biomes that warrant voxel substrate storage. Mountain
/// always qualifies. Snow qualifies because it caps high mountains
/// (elevation > SNOW_CAP_LEVEL); cold-lowland Snow tiles (assigned
/// at low elevation when temperature is very cold) also pass, which
/// allocates a tiny voxel column for those cells. Harmless at Stage
/// 1; Stage 2's cave generator can gate on elevation if it ever
/// matters.
pub fn is_highland_biome(b: Biome) -> bool {
    matches!(b, Biome::Mountain | Biome::Snow)
}

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
            let i = lz * CHUNK_VERTS + lx;
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
            // Each heightmap cell owns a 2x2 voxel sub-grid in XZ.
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
/// Each face is one quad (two triangles). Normals point INTO the cave
/// (toward the carved-air side) so lighting reads correctly from inside.
/// Winding is CCW from the cave-interior viewpoint so backface culling
/// keeps the faces visible from inside the cave.
pub fn append_cave_geometry(
    chunk: &VoxelChunk,
    carved: &HashSet<VoxelLocal>,
    geom: &mut super::terrain::ChunkGeometry,
) {
    for face in emit_cave_faces(chunk, carved) {
        let (lx, ly, lz) = face.voxel;
        let x0 = (lx as f32) * VOXEL_SIZE;
        let y0 = (ly as f32) * VOXEL_SIZE;
        let z0 = (lz as f32) * VOXEL_SIZE;
        let s = VOXEL_SIZE;
        // Corners are placed on the face plane, and the winding order is
        // CCW when viewed from the cave-interior side (the carved-air side),
        // which is the OPPOSITE direction from face.normal (face.normal
        // points toward the solid neighbour / outward into rock).
        //
        // The normal reported to the GPU also points INTO the cave so that
        // directional lighting is computed correctly from the cave viewer's
        // perspective.
        let (corners, normal) = match face.normal {
            [1, 0, 0] => (
                // Wall at x = x0 + s, visible from -X (cave is to the west).
                [
                    [x0 + s, y0,     z0    ],
                    [x0 + s, y0 + s, z0    ],
                    [x0 + s, y0,     z0 + s],
                    [x0 + s, y0 + s, z0 + s],
                ],
                [-1.0, 0.0, 0.0],
            ),
            [-1, 0, 0] => (
                // Wall at x = x0, visible from +X (cave is to the east).
                [
                    [x0,     y0,     z0 + s],
                    [x0,     y0 + s, z0 + s],
                    [x0,     y0,     z0    ],
                    [x0,     y0 + s, z0    ],
                ],
                [1.0, 0.0, 0.0],
            ),
            [0, 1, 0] => (
                // Ceiling at y = y0 + s, visible from -Y (cave is below).
                [
                    [x0,     y0 + s, z0    ],
                    [x0,     y0 + s, z0 + s],
                    [x0 + s, y0 + s, z0    ],
                    [x0 + s, y0 + s, z0 + s],
                ],
                [0.0, -1.0, 0.0],
            ),
            [0, -1, 0] => (
                // Floor at y = y0, visible from +Y (cave is above).
                [
                    [x0,     y0,     z0 + s],
                    [x0,     y0,     z0    ],
                    [x0 + s, y0,     z0 + s],
                    [x0 + s, y0,     z0    ],
                ],
                [0.0, 1.0, 0.0],
            ),
            [0, 0, 1] => (
                // Wall at z = z0 + s, visible from -Z (cave is to the north).
                [
                    [x0 + s, y0,     z0 + s],
                    [x0 + s, y0 + s, z0 + s],
                    [x0,     y0,     z0 + s],
                    [x0,     y0 + s, z0 + s],
                ],
                [0.0, 0.0, -1.0],
            ),
            [0, 0, -1] => (
                // Wall at z = z0, visible from +Z (cave is to the south).
                [
                    [x0,     y0,     z0    ],
                    [x0,     y0 + s, z0    ],
                    [x0 + s, y0,     z0    ],
                    [x0 + s, y0 + s, z0    ],
                ],
                [0.0, 0.0, 1.0],
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
            [0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0],
        ]);
        // Winding: CCW from the cave interior side. Corners are laid out as
        // (bottom-left, top-left, bottom-right, top-right) from the cave
        // viewer's perspective -- so triangle 1 = (0, 1, 2) and
        // triangle 2 = (1, 3, 2) both go CCW from inside.
        geom.indices.extend_from_slice(&[
            base, base + 1, base + 2,
            base + 1, base + 3, base + 2,
        ]);
    }
}

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

/// Plugin that registers the [`VoxelLayer`] resource and keeps it in
/// sync with the chunk lifecycle. Listens to [`ChunkLoaded`] to
/// populate voxels for highland chunks and to
/// [`super::chunks::ChunkUnloaded`] to release them.
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
        // Cell (3, 7) maps to voxel columns at lx in {6, 7}, lz in {14, 15}.
        // heightmap_y = 6.25 -> max_ly = floor(6.25 / 0.5) = 12.
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
        // Cell (0, 0) is the default Grassland, height 0.0 -- its voxel
        // columns at lx in {0, 1}, lz in {0, 1} stay all-air.
        for lx in 0..=1u8 {
            for lz in 0..=1u8 {
                for ly in 0..12u8 {
                    assert!(!voxel.get((lx, ly, lz)));
                }
            }
        }
    }

    #[test]
    fn voxel_layer_starts_with_empty_dirty_and_carved_sets() {
        let layer = VoxelLayer::default();
        assert!(layer.dirty.is_empty());
        assert!(layer.carved.is_empty());
    }

    /// Builds a `(Terrain, VoxelLayer)` pair reusing the existing
    /// highland terrain helper. Cell (3, 7) is Mountain at height 6.25,
    /// giving voxel columns lx in {6,7}, lz in {14,15} solid for ly 0..12.
    fn highland_chunk_filled() -> (Terrain, VoxelLayer) {
        let terrain = highland_terrain_with_one_cell();
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
        // (10,10,10): solid neighbour at (9,10,10) -> 1 face.
        // (11,10,10): -X neighbour (10,10,10) is carved (skip); all
        //   other neighbours are air -> 0 faces.
        // Total: 1.
        assert_eq!(faces.len(), 1);
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
}
