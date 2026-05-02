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

/// Resource holding voxel chunks for highland chunks. Non-highland
/// chunks are absent from the map (not stored as empty).
#[derive(Resource, Default)]
pub struct VoxelLayer {
    pub chunks: HashMap<ChunkCoord, VoxelChunk>,
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
}
