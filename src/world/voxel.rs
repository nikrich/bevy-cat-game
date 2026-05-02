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
