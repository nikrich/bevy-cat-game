//! Vertex-height terrain grid (Phase 1 / DEC-017, DEC-018).
//!
//! Replaces the per-tile cuboid model with a single triangle mesh per chunk
//! plus a Rapier heightfield collider. The world stores heights and biome
//! ids in a [`Terrain`] resource keyed by chunk coord; mesh and collider
//! generation happens in the regen system, capped at
//! [`REGEN_BUDGET_PER_FRAME`] chunks per frame.
//!
//! Coordinate convention:
//! - Chunk `(cx, cz)` covers world XZ `[cx * CHUNK_CELLS, (cx+1) * CHUNK_CELLS)`.
//! - Vertex `(lx, lz)` in that chunk is at world XZ `(cx*CHUNK_CELLS + lx, cz*CHUNK_CELLS + lz)`.
//! - Vertex `lx == CHUNK_CELLS` is the shared edge with the neighbour chunk
//!   to the right, so each chunk's height array is `CHUNK_VERTS` per side.
//! - Heights are stored row-major over `lz` then `lx`:
//!   `heights[lz * CHUNK_VERTS + lx]`.
//! - The chunk *entity* lives at world `(cx * CHUNK_CELLS, 0, cz * CHUNK_CELLS)`
//!   so children (props, water, collider) read with chunk-local transforms.

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_rapier3d::prelude::{Collider, RigidBody};
use noise::{NoiseFn, Perlin};
use std::collections::{HashMap, HashSet};

use super::biome::{Biome, WorldNoise};
use super::chunks::ChunkManager;

/// Verts per chunk side. 32 cells + the shared edge = 33 verts.
pub const CHUNK_VERTS: usize = 33;
/// Cells per chunk side. Each cell is 1 m wide, so a chunk is 32×32 m.
pub const CHUNK_CELLS: i32 = 32;
/// Cap on dirty chunks regenerated per frame to keep regen hitches bounded
/// (W1.2 budget).
pub const REGEN_BUDGET_PER_FRAME: usize = 4;

pub type ChunkCoord = (i32, i32);

/// Quantize height to 0.25 m steps for the chunky low-poly look.
pub fn step_height(h: f32) -> f32 {
    (h * 4.0).round() / 4.0
}

/// Y of the wading floor under water cells. Tuned so the cat's float spring
/// settles its centre roughly at the water plane height (sea level minus
/// 0.15) — body submerged, head poking out. Mirrors the offset the old
/// per-tile water floor used.
pub const WATER_FLOOR_Y: f32 = -1.30;

/// Surface Y for a world position straight from the noise generator. Used as
/// the source of truth for chunk PCG and as a fallback when the chunk hasn't
/// loaded yet.
///
/// Land cells use `step_height(elevation * height_scale) * 0.5 + 0.3` so the
/// surface matches the old per-tile cuboid top. Water cells get pulled to
/// [`WATER_FLOOR_Y`] so the cat physically sinks into them and the per-chunk
/// water plane covers the surface.
pub fn surface_height(noise: &WorldNoise, world_x: f64, world_z: f64) -> f32 {
    let sample = noise.sample(world_x, world_z);
    if sample.biome.is_water() {
        WATER_FLOOR_Y
    } else {
        step_height(sample.elevation * sample.biome.height_scale()) * 0.5 + 0.3
    }
}

/// Per-chunk vertex grid.
#[derive(Clone)]
pub struct ChunkData {
    /// `CHUNK_VERTS * CHUNK_VERTS` heights, indexed `[lz * CHUNK_VERTS + lx]`.
    pub heights: Box<[f32]>,
    /// Same layout as `heights`. Used by the mesh builder for vertex tints
    /// and by callers that need the biome at a particular world position.
    pub biomes: Box<[Biome]>,
}

impl ChunkData {
    pub fn empty() -> Self {
        let n = CHUNK_VERTS * CHUNK_VERTS;
        Self {
            heights: vec![0.0; n].into_boxed_slice(),
            biomes: vec![Biome::Grassland; n].into_boxed_slice(),
        }
    }

    #[inline]
    pub fn idx(lx: usize, lz: usize) -> usize {
        lz * CHUNK_VERTS + lx
    }
}

/// Authoritative store for terrain vertex state. Mesh and collider are
/// derivatives, rebuilt by [`regenerate_dirty_chunks`].
///
/// `edits` and `biome_edits` are persistent overlays that survive chunk
/// unloads and game saves: they remember any vertex heights and biome ids
/// set via the brush APIs so re-loading the chunk re-applies them on top
/// of the PCG default. The keys are chunk-local indices in `0..CHUNK_CELLS`,
/// matching the `vertex_owner` mapping used everywhere else.
///
/// `painted_cells` is a *transient* set of cells whose biome was painted
/// since the last prop respawn pass. It's drained by
/// `respawn_props_for_painted_cells` each frame; nothing else reads it,
/// and it's never persisted (the durable record lives in `biome_edits`).
///
/// `dirty` and `color_dirty` partition chunk regen by reason:
/// - `dirty` triggers a full mesh + trimesh collider rebuild — used for
///   any height edit, since the chunk's surface geometry actually
///   changed.
/// - `color_dirty` triggers a mesh-only rebuild — used for biome paint,
///   where heights are unchanged so the trimesh is *identical*.
///   Rebuilding the rapier trimesh collider on every paint tick is
///   what was causing parry's BVH builder to crash mid-painting; a
///   biome change literally produces the same vert/tri buffers as the
///   previous build, so re-handing them to rapier was pure churn.
#[derive(Resource, Default)]
pub struct Terrain {
    pub chunks: HashMap<ChunkCoord, ChunkData>,
    pub dirty: HashSet<ChunkCoord>,
    pub color_dirty: HashSet<ChunkCoord>,
    pub edits: HashMap<ChunkCoord, HashMap<(u8, u8), f32>>,
    pub biome_edits: HashMap<ChunkCoord, HashMap<(u8, u8), Biome>>,
    pub painted_cells: HashMap<ChunkCoord, HashSet<(u8, u8)>>,
}

impl Terrain {
    /// PCG-fill a chunk's heights/biomes from the noise generator, then
    /// re-apply any persisted edits, then mark dirty so the regen system
    /// rebuilds its mesh + collider.
    pub fn generate_chunk(&mut self, coord: ChunkCoord, noise: &WorldNoise) {
        let mut data = ChunkData::empty();
        let world_offset_x = coord.0 * CHUNK_CELLS;
        let world_offset_z = coord.1 * CHUNK_CELLS;
        for lz in 0..CHUNK_VERTS {
            for lx in 0..CHUNK_VERTS {
                let wx = (world_offset_x + lx as i32) as f64;
                let wz = (world_offset_z + lz as i32) as f64;
                let sample = noise.sample(wx, wz);
                let i = ChunkData::idx(lx, lz);
                data.heights[i] = if sample.biome.is_water() {
                    WATER_FLOOR_Y
                } else {
                    step_height(sample.elevation * sample.biome.height_scale()) * 0.5 + 0.3
                };
                data.biomes[i] = sample.biome;
            }
        }
        if let Some(chunk_edits) = self.edits.get(&coord) {
            for (&(lx, lz), &h) in chunk_edits {
                let lx = lx as usize;
                let lz = lz as usize;
                if lx < CHUNK_VERTS && lz < CHUNK_VERTS {
                    data.heights[ChunkData::idx(lx, lz)] = h;
                }
            }
        }
        if let Some(chunk_biome_edits) = self.biome_edits.get(&coord) {
            for (&(lx, lz), &b) in chunk_biome_edits {
                let lx = lx as usize;
                let lz = lz as usize;
                if lx < CHUNK_VERTS && lz < CHUNK_VERTS {
                    data.biomes[ChunkData::idx(lx, lz)] = b;
                }
            }
        }
        self.chunks.insert(coord, data);
        self.dirty.insert(coord);
    }

    pub fn unload_chunk(&mut self, coord: ChunkCoord) {
        self.chunks.remove(&coord);
        self.dirty.remove(&coord);
    }

    /// Surface Y at the vertex closest to `(world_x, world_z)`. Returns
    /// `None` if the owning chunk isn't loaded.
    pub fn height_at(&self, world_x: f32, world_z: f32) -> Option<f32> {
        let (cx, cz, lx, lz) = world_to_vertex(world_x, world_z);
        let chunk = self.chunks.get(&(cx, cz))?;
        Some(chunk.heights[ChunkData::idx(lx, lz)])
    }

    /// Biome at the vertex closest to `(world_x, world_z)`.
    pub fn biome_at(&self, world_x: f32, world_z: f32) -> Option<Biome> {
        let (cx, cz, lx, lz) = world_to_vertex(world_x, world_z);
        let chunk = self.chunks.get(&(cx, cz))?;
        Some(chunk.biomes[ChunkData::idx(lx, lz)])
    }

    /// Read from the cached grid, or fall back to the noise sample when the
    /// chunk isn't loaded yet. The fallback keeps systems that operate at
    /// the world edge (animal flee, building preview) from snapping to 0.
    pub fn height_at_or_sample(&self, world_x: f32, world_z: f32, noise: &WorldNoise) -> f32 {
        self.height_at(world_x, world_z)
            .unwrap_or_else(|| surface_height(noise, world_x as f64, world_z as f64))
    }

    /// Read the height stored at world vertex `(wx, wz)`. Each integer world
    /// coord owns exactly one storage slot, in the chunk where the vertex is
    /// the NW corner of a cell (`rem_euclid` mapping — slot indices fall in
    /// `0..CHUNK_CELLS`). Returns `None` if that chunk isn't loaded.
    pub fn vertex_height(&self, wx: i32, wz: i32) -> Option<f32> {
        let (cx, cz, lx, lz) = vertex_owner(wx, wz);
        let chunk = self.chunks.get(&(cx, cz))?;
        Some(chunk.heights[ChunkData::idx(lx, lz)])
    }

    /// Write a height at world vertex `(wx, wz)` and mark every chunk whose
    /// rendered mesh depends on this vertex as dirty. Each cell's mesh
    /// reads heights at its own NW corner plus the four cardinal-neighbour
    /// NW corners (for risers); editing one vertex therefore affects up to
    /// five cells, in up to four chunks if the vertex sits at a chunk
    /// corner.
    ///
    /// Returns `true` if the height changed, `false` if the chunk wasn't
    /// loaded or the new height matched the old.
    pub fn set_vertex_height(&mut self, wx: i32, wz: i32, new_h: f32) -> bool {
        let (cx, cz, lx, lz) = vertex_owner(wx, wz);
        let Some(chunk) = self.chunks.get_mut(&(cx, cz)) else {
            return false;
        };
        let idx = ChunkData::idx(lx, lz);
        if (chunk.heights[idx] - new_h).abs() < f32::EPSILON {
            return false;
        }
        chunk.heights[idx] = new_h;

        // Record the edit so it survives chunk unload + game save. Re-edits
        // overwrite the same key, so the map is bounded by the number of
        // *distinct* vertices touched, not by brush ticks.
        self.edits
            .entry((cx, cz))
            .or_default()
            .insert((lx as u8, lz as u8), new_h);

        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)] {
            let cell_world_x = wx + dx;
            let cell_world_z = wz + dz;
            let chunk_x = cell_world_x.div_euclid(CHUNK_CELLS);
            let chunk_z = cell_world_z.div_euclid(CHUNK_CELLS);
            if self.chunks.contains_key(&(chunk_x, chunk_z)) {
                self.dirty.insert((chunk_x, chunk_z));
            }
        }
        true
    }

    /// Read the biome stored at world vertex `(wx, wz)`. Each integer world
    /// coord owns one storage slot in the chunk where the vertex is the NW
    /// corner of a cell. Returns `None` if that chunk isn't loaded.
    pub fn vertex_biome(&self, wx: i32, wz: i32) -> Option<Biome> {
        let (cx, cz, lx, lz) = vertex_owner(wx, wz);
        let chunk = self.chunks.get(&(cx, cz))?;
        Some(chunk.biomes[ChunkData::idx(lx, lz)])
    }

    /// Write a biome at world vertex `(wx, wz)` and mark the owning chunk
    /// dirty. Unlike [`Self::set_vertex_height`], biome only feeds the cell
    /// whose NW corner is this vertex (it doesn't affect neighbouring cell
    /// risers), so only one chunk needs marking.
    ///
    /// Returns `true` if the biome changed, `false` if the chunk wasn't
    /// loaded or the new biome matched the old.
    pub fn set_vertex_biome(&mut self, wx: i32, wz: i32, new_biome: Biome) -> bool {
        let (cx, cz, lx, lz) = vertex_owner(wx, wz);
        let Some(chunk) = self.chunks.get_mut(&(cx, cz)) else {
            return false;
        };
        let idx = ChunkData::idx(lx, lz);
        if chunk.biomes[idx] == new_biome {
            return false;
        }
        chunk.biomes[idx] = new_biome;

        self.biome_edits
            .entry((cx, cz))
            .or_default()
            .insert((lx as u8, lz as u8), new_biome);
        self.painted_cells
            .entry((cx, cz))
            .or_default()
            .insert((lx as u8, lz as u8));
        // Biome change → mesh vertex colors need to refresh, but the
        // trimesh collider stays identical. Use `color_dirty` so the
        // regen system rebuilds the mesh only (closes the
        // parry-BVH-crash-during-paint bug — see `Terrain` doc).
        self.color_dirty.insert((cx, cz));
        true
    }

    /// Flatten the rectangular footprint `[min_x..=max_x] × [min_z..=max_z]`
    /// to the median ground height inside it, then blend a smoothstep skirt
    /// outward by `skirt_width` tiles so the edit eases into the surrounding
    /// terrain (W1.11). Returns the number of vertices changed.
    ///
    /// Phase 2's building-placement system will call this whenever a floor
    /// or platform piece is placed; Phase 1 exposes the API and a debug
    /// hotkey so the visual can be checked in isolation.
    pub fn flatten_rect(
        &mut self,
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
        skirt_width: i32,
        noise: &WorldNoise,
    ) -> usize {
        let mut samples = Vec::with_capacity(((max_x - min_x + 1) * (max_z - min_z + 1)) as usize);
        for vz in min_z..=max_z {
            for vx in min_x..=max_x {
                samples.push(self.height_at_or_sample(vx as f32, vz as f32, noise));
            }
        }
        if samples.is_empty() {
            return 0;
        }
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let target = step_height(samples[samples.len() / 2]);

        let mut count = 0;
        // Inside the footprint: every vertex snaps to target.
        for vz in min_z..=max_z {
            for vx in min_x..=max_x {
                if self.set_vertex_height(vx, vz, target) {
                    count += 1;
                }
            }
        }
        // Skirt: outside the footprint but within `skirt_width`. Blend
        // smoothstep between the footprint edge (full target) and the
        // outer skirt boundary (untouched). Chebyshev distance keeps the
        // skirt rectangular, matching the footprint shape.
        if skirt_width > 0 {
            for vz in (min_z - skirt_width)..=(max_z + skirt_width) {
                for vx in (min_x - skirt_width)..=(max_x + skirt_width) {
                    if vx >= min_x && vx <= max_x && vz >= min_z && vz <= max_z {
                        continue;
                    }
                    let dx_out = (min_x - vx).max(vx - max_x).max(0);
                    let dz_out = (min_z - vz).max(vz - max_z).max(0);
                    let d = dx_out.max(dz_out);
                    if d == 0 || d > skirt_width {
                        continue;
                    }
                    let t = d as f32 / skirt_width as f32;
                    let smoothed = t * t * (3.0 - 2.0 * t);
                    let blend = 1.0 - smoothed;
                    let original = self.height_at_or_sample(vx as f32, vz as f32, noise);
                    let new_h = step_height(original + (target - original) * blend);
                    if self.set_vertex_height(vx, vz, new_h) {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

/// Map a world vertex coord to the chunk that owns its storage slot. Uses
/// `rem_euclid` so vertices on chunk boundaries land at `lx == 0` of the
/// next chunk, never at the vestigial `lx == CHUNK_CELLS` slot of the
/// previous one.
fn vertex_owner(wx: i32, wz: i32) -> (i32, i32, usize, usize) {
    let cx = wx.div_euclid(CHUNK_CELLS);
    let cz = wz.div_euclid(CHUNK_CELLS);
    let lx = wx.rem_euclid(CHUNK_CELLS) as usize;
    let lz = wz.rem_euclid(CHUNK_CELLS) as usize;
    (cx, cz, lx, lz)
}

/// Map a world XZ to (chunk coord, local vertex index). Floor for chunk
/// coord, round for vertex (vertices live at integer world coords). The
/// vertex index can land on the shared edge `lx == CHUNK_CELLS`, which is
/// still a valid index into the chunk's `CHUNK_VERTS`-wide row.
fn world_to_vertex(world_x: f32, world_z: f32) -> (i32, i32, usize, usize) {
    let cx = (world_x / CHUNK_CELLS as f32).floor() as i32;
    let cz = (world_z / CHUNK_CELLS as f32).floor() as i32;
    let lx = (world_x.round() as i32 - cx * CHUNK_CELLS).clamp(0, CHUNK_CELLS) as usize;
    let lz = (world_z.round() as i32 - cz * CHUNK_CELLS).clamp(0, CHUNK_CELLS) as usize;
    (cx, cz, lx, lz)
}

// ---------- Mesh + collider ----------

/// Marker for the mesh-bearing chunk entity. The trimesh collider lives
/// on the same entity now (no separate collider child), so a single marker
/// is enough.
#[derive(Component)]
pub struct TerrainChunk;

/// Shared chunk material. Vertex colours carry biome tint; the material
/// stays generic so all chunks reuse the same `Handle<StandardMaterial>`
/// (closes DEBT-008). The base-color texture is a procedurally generated
/// tile-friendly grayscale noise map (W1.4) so each cell reads as
/// "tinted textured ground" instead of flat color — the texture multiplies
/// against the per-vertex biome tint, so biome colours stay dominant.
#[derive(Resource)]
pub struct TerrainMaterial(pub Handle<StandardMaterial>);

impl FromWorld for TerrainMaterial {
    fn from_world(world: &mut World) -> Self {
        let texture = generate_terrain_noise_image();
        let texture_handle = world.resource_mut::<Assets<Image>>().add(texture);
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let handle = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(texture_handle),
            perceptual_roughness: 0.92,
            ..default()
        });
        Self(handle)
    }
}

/// Generate a 128×128 sRGB noise tile that tiles cleanly across cell UVs.
///
/// Each cell's quad has UV `0..1`, so adjacent cells repeat the same tile.
/// To avoid visible seams we sample 4D Perlin on a torus
/// (`(cos 2πu, sin 2πu, cos 2πv, sin 2πv)`), which is automatically
/// periodic on both axes. Output range is mapped to `[0.78, 1.0]`
/// grayscale — subtle enough that the per-vertex biome tint stays
/// dominant; the texture just keeps each cell from looking like a flat
/// painted square. Risers reuse the same UV mapping; the noise reads as
/// natural rocky face dapples on the vertical surfaces.
fn generate_terrain_noise_image() -> Image {
    const SIZE: usize = 128;
    let perlin = Perlin::new(9999);
    let two_pi = std::f64::consts::TAU;

    let mut data = vec![0u8; SIZE * SIZE * 4];
    for py in 0..SIZE {
        for px in 0..SIZE {
            let u = px as f64 / SIZE as f64;
            let v = py as f64 / SIZE as f64;
            // Two octaves on a torus: r=1.5 for broad bumps, r=3.5 for
            // finer grain. Higher-frequency octave is half-weighted so
            // total variation stays within ~[-1.5, 1.5].
            let n1 = perlin.get([
                (u * two_pi).cos() * 1.5,
                (u * two_pi).sin() * 1.5,
                (v * two_pi).cos() * 1.5,
                (v * two_pi).sin() * 1.5,
            ]);
            let n2 = perlin.get([
                (u * two_pi).cos() * 3.5,
                (u * two_pi).sin() * 3.5,
                (v * two_pi).cos() * 3.5,
                (v * two_pi).sin() * 3.5,
            ]) * 0.5;
            let n = (n1 + n2) as f32;
            let gray = (0.91 + n * 0.07).clamp(0.78, 1.0);
            let g = (gray * 255.0) as u8;
            let i = (py * SIZE + px) * 4;
            data[i] = g;
            data[i + 1] = g;
            data[i + 2] = g;
            data[i + 3] = 255;
        }
    }

    Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

/// Linear-space biome tint, with a small position-derived shade variation
/// so adjacent cells of the same biome aren't pure flat color (preserves
/// the texture-of-many-tiles feel from the old per-tile cuboid look).
fn cell_color(biome: Biome, world_x: i32, world_z: i32) -> [f32; 4] {
    let shade = ((world_x.wrapping_mul(7) + world_z.wrapping_mul(13)).unsigned_abs() % 3) as u8;
    let c = biome.terrain_color(shade).to_linear();
    [c.red, c.green, c.blue, 1.0]
}

/// Raw geometry for a chunk: cell-aligned flat-top quads + vertical riser
/// quads where neighbours are shorter. Shared between the renderable
/// `Mesh` and the rapier `Collider` so the player physically walks on the
/// same surface they see.
pub struct ChunkGeometry {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<[f32; 4]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// Build the cell-aligned stepped geometry for a chunk. Each cell's height
/// comes from its NW corner vertex (`heights[lz * VERTS + lx]`); a riser is
/// emitted between two cells only when *this* cell is taller, so the
/// shorter neighbour never duplicates the wall. Chunk-edge neighbours are
/// resolved via `Terrain::height_at_or_sample` (PCG fallback when the
/// neighbour chunk hasn't loaded yet).
pub fn build_chunk_geometry(
    coord: ChunkCoord,
    terrain: &Terrain,
    noise: &WorldNoise,
    voxel_layer: &super::voxel::VoxelLayer,
) -> ChunkGeometry {
    let data = terrain
        .chunks
        .get(&coord)
        .expect("build_chunk_geometry called on an unloaded chunk");

    // Look up the height of the cell whose NW corner is at vertex (lx, lz)
    // of this chunk. lx/lz can range outside [0, CHUNK_CELLS) for chunk-edge
    // neighbours; in that case fall through to terrain (or PCG) at the
    // matching world coordinate.
    let cell_height = |lx: i32, lz: i32| -> f32 {
        if lx >= 0 && lx < CHUNK_CELLS && lz >= 0 && lz < CHUNK_CELLS {
            data.heights[ChunkData::idx(lx as usize, lz as usize)]
        } else {
            let world_x = (coord.0 * CHUNK_CELLS + lx) as f32;
            let world_z = (coord.1 * CHUNK_CELLS + lz) as f32;
            terrain.height_at_or_sample(world_x, world_z, noise)
        }
    };

    // Pre-size for a typical chunk: 32×32 cells × (1 top + ~1.5 risers
    // average) × 4 verts per quad. Way over-allocates for flat areas, fine.
    let estimated_quads = (CHUNK_CELLS * CHUNK_CELLS * 3) as usize;
    let mut positions = Vec::with_capacity(estimated_quads * 4);
    let mut normals = Vec::with_capacity(estimated_quads * 4);
    let mut colors = Vec::with_capacity(estimated_quads * 4);
    let mut uvs = Vec::with_capacity(estimated_quads * 4);
    let mut indices = Vec::with_capacity(estimated_quads * 6);

    let mut emit_quad = |positions: &mut Vec<[f32; 3]>,
                         normals: &mut Vec<[f32; 3]>,
                         colors: &mut Vec<[f32; 4]>,
                         uvs: &mut Vec<[f32; 2]>,
                         indices: &mut Vec<u32>,
                         corners: [[f32; 3]; 4],
                         normal: [f32; 3],
                         color: [f32; 4]| {
        let base = positions.len() as u32;
        positions.extend_from_slice(&corners);
        for _ in 0..4 {
            normals.push(normal);
            colors.push(color);
        }
        uvs.extend_from_slice(&[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]]);
        // Triangulate (0, 2, 1) and (1, 2, 3) — CCW with the supplied normal
        // when corners are passed NW, NE, SW, SE.
        indices.extend_from_slice(&[
            base, base + 2, base + 1,
            base + 1, base + 2, base + 3,
        ]);
    };

    for lz in 0..CHUNK_CELLS {
        for lx in 0..CHUNK_CELLS {
            let h = data.heights[ChunkData::idx(lx as usize, lz as usize)];
            let biome = data.biomes[ChunkData::idx(lx as usize, lz as usize)];
            let world_x = coord.0 * CHUNK_CELLS + lx;
            let world_z = coord.1 * CHUNK_CELLS + lz;
            let color = cell_color(biome, world_x, world_z);

            let x0 = lx as f32;
            let x1 = (lx + 1) as f32;
            let z0 = lz as f32;
            let z1 = (lz + 1) as f32;

            // Top: flat quad, normal +Y. Corners NW, NE, SW, SE.
            emit_quad(
                &mut positions,
                &mut normals,
                &mut colors,
                &mut uvs,
                &mut indices,
                [
                    [x0, h, z0],
                    [x1, h, z0],
                    [x0, h, z1],
                    [x1, h, z1],
                ],
                [0.0, 1.0, 0.0],
                color,
            );

            // Risers use their true side-facing normal so directional
            // sunlight gives the cuboid sides their natural shading. The
            // colour is still the cell's biome colour, so a tile reads as
            // one block — but with proper depth from the lighting.
            //
            // Corners follow the helper's "NW, NE, SW, SE with normal up"
            // convention, but interpreted from a viewer standing on the
            // *outward* side of the riser. That puts the top-of-wall pair
            // first (NW, NE) and the bottom pair second (SW, SE) — and
            // the (NW, NE) order has to flip for each face so the cross
            // product of the resulting CCW winding lines up with the
            // outward normal we declare. A wrong order here culls the
            // riser as a backface and the cuboid sides go missing.

            // East riser: outward = +X, viewer's right = -Z.
            let east_h = cell_height(lx + 1, lz);
            if h > east_h {
                emit_quad(
                    &mut positions,
                    &mut normals,
                    &mut colors,
                    &mut uvs,
                    &mut indices,
                    [
                        [x1, h, z1],
                        [x1, h, z0],
                        [x1, east_h, z1],
                        [x1, east_h, z0],
                    ],
                    [1.0, 0.0, 0.0],
                    color,
                );
            }

            // West riser: outward = -X, viewer's right = +Z.
            let west_h = cell_height(lx - 1, lz);
            if h > west_h {
                emit_quad(
                    &mut positions,
                    &mut normals,
                    &mut colors,
                    &mut uvs,
                    &mut indices,
                    [
                        [x0, h, z0],
                        [x0, h, z1],
                        [x0, west_h, z0],
                        [x0, west_h, z1],
                    ],
                    [-1.0, 0.0, 0.0],
                    color,
                );
            }

            // South riser: outward = +Z, viewer's right = +X.
            let south_h = cell_height(lx, lz + 1);
            if h > south_h {
                emit_quad(
                    &mut positions,
                    &mut normals,
                    &mut colors,
                    &mut uvs,
                    &mut indices,
                    [
                        [x0, h, z1],
                        [x1, h, z1],
                        [x0, south_h, z1],
                        [x1, south_h, z1],
                    ],
                    [0.0, 0.0, 1.0],
                    color,
                );
            }

            // North riser: outward = -Z, viewer's right = -X.
            let north_h = cell_height(lx, lz - 1);
            if h > north_h {
                emit_quad(
                    &mut positions,
                    &mut normals,
                    &mut colors,
                    &mut uvs,
                    &mut indices,
                    [
                        [x1, h, z0],
                        [x0, h, z0],
                        [x1, north_h, z0],
                        [x0, north_h, z0],
                    ],
                    [0.0, 0.0, -1.0],
                    color,
                );
            }
        }
    }

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
}

/// Build the renderable mesh from the chunk geometry buffers.
pub fn build_chunk_mesh(geom: &ChunkGeometry) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, geom.positions.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, geom.normals.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, geom.colors.clone());
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, geom.uvs.clone());
    mesh.insert_indices(Indices::U32(geom.indices.clone()));
    mesh
}

/// Build the rapier trimesh collider from the same geometry buffers, so
/// physics matches the visual exactly — the cat's capsule clips the same
/// risers it can see.
///
/// `FIX_INTERNAL_EDGES` is required: without it, parry's EPA can fail to
/// converge when Tnua's downward capsule cast lands on a shared edge
/// between two triangles with conflicting normals (e.g. the inner corner
/// of a cliff riser). The pseudo-normals computed by this flag let EPA
/// pick a single contact normal per edge and terminate. The flag implies
/// `MERGE_DUPLICATE_VERTICES`, which we want anyway since our chunk
/// geometry duplicates verts at every face. Pre-processing cost is the
/// trade-off — measured at single-digit ms per chunk in debug, well
/// within the regen budget.
pub fn build_chunk_collider(geom: &ChunkGeometry) -> Option<Collider> {
    use bevy_rapier3d::prelude::TriMeshFlags;

    let verts: Vec<Vec3> = geom
        .positions
        .iter()
        .map(|p| Vec3::new(p[0], p[1], p[2]))
        .collect();
    let tris: Vec<[u32; 3]> = geom
        .indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    Collider::trimesh_with_flags(verts, tris, TriMeshFlags::FIX_INTERNAL_EDGES).ok()
}

// ---------- Regen system ----------

/// Up to [`REGEN_BUDGET_PER_FRAME`] dirty chunks per frame, in two
/// passes:
/// 1. `terrain.dirty` (geometry change) — rebuild mesh + trimesh
///    collider. Chunks loaded via `load_nearby_chunks` start here.
/// 2. `terrain.color_dirty` (biome paint only) — rebuild mesh only,
///    leaving the existing trimesh collider intact. The geometry hasn't
///    changed, so re-handing parry an identical trimesh every paint
///    tick was both wasted work and a parry-BVH-builder crash trigger.
///
/// A chunk in both sets is processed only by pass 1 — its mesh rebuilds
/// already.
pub fn regenerate_dirty_chunks(
    mut commands: Commands,
    mut terrain: ResMut<Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain_material: Res<TerrainMaterial>,
    chunk_manager: Res<ChunkManager>,
    noise: Res<WorldNoise>,
    voxel_layer: Res<super::voxel::VoxelLayer>,
) {
    if terrain.dirty.is_empty() && terrain.color_dirty.is_empty() {
        return;
    }
    let geom_coords: Vec<ChunkCoord> = terrain
        .dirty
        .iter()
        .copied()
        .take(REGEN_BUDGET_PER_FRAME)
        .collect();

    for coord in geom_coords {
        terrain.dirty.remove(&coord);
        // Geometry rebuild already refreshes vertex colors, so skip the
        // color-only pass for this chunk this frame.
        terrain.color_dirty.remove(&coord);
        if !terrain.chunks.contains_key(&coord) {
            continue;
        }
        let Some(&chunk_entity) = chunk_manager.loaded.get(&coord) else {
            continue;
        };

        let geom = build_chunk_geometry(coord, &terrain, &noise, &voxel_layer);
        let mesh = meshes.add(build_chunk_mesh(&geom));
        let Some(collider) = build_chunk_collider(&geom) else {
            warn!("trimesh build failed for chunk {coord:?}");
            continue;
        };

        // `try_insert` swallows the despawn race: a chunk can be unloaded
        // (commands.entity().despawn() queued) the same frame regen tries
        // to insert components on it. The chunk-lifecycle systems are
        // chained to minimise that, but the race still opens whenever
        // unload runs in a later frame between when regen drained `dirty`
        // and when its commands apply.
        commands.entity(chunk_entity).try_insert((
            Mesh3d(mesh),
            MeshMaterial3d(terrain_material.0.clone()),
            TerrainChunk,
            collider,
            RigidBody::Fixed,
        ));
    }

    // Color-only pass: vertex tints changed but geometry didn't. Swap
    // in a fresh `Mesh3d`; the existing collider component on the chunk
    // entity is left untouched.
    let color_coords: Vec<ChunkCoord> = terrain
        .color_dirty
        .iter()
        .copied()
        .take(REGEN_BUDGET_PER_FRAME)
        .collect();
    for coord in color_coords {
        terrain.color_dirty.remove(&coord);
        if !terrain.chunks.contains_key(&coord) {
            continue;
        }
        let Some(&chunk_entity) = chunk_manager.loaded.get(&coord) else {
            continue;
        };
        let geom = build_chunk_geometry(coord, &terrain, &noise, &voxel_layer);
        let mesh = meshes.add(build_chunk_mesh(&geom));
        commands.entity(chunk_entity).try_insert((
            Mesh3d(mesh),
            MeshMaterial3d(terrain_material.0.clone()),
            TerrainChunk,
        ));
    }
}
