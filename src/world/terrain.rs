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
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, RigidBody, TriMeshFlags};
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
#[derive(Resource, Default)]
pub struct Terrain {
    pub chunks: HashMap<ChunkCoord, ChunkData>,
    pub dirty: HashSet<ChunkCoord>,
}

impl Terrain {
    /// PCG-fill a chunk's heights/biomes from the noise generator and mark it
    /// dirty so the regen system rebuilds its mesh + collider.
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
/// (closes DEBT-008).
#[derive(Resource)]
pub struct TerrainMaterial(pub Handle<StandardMaterial>);

impl FromWorld for TerrainMaterial {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let handle = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.92,
            ..default()
        });
        Self(handle)
    }
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

    ChunkGeometry {
        positions,
        normals,
        colors,
        uvs,
        indices,
    }
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
pub fn build_chunk_collider(geom: &ChunkGeometry) -> Option<Collider> {
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
    Collider::trimesh_with_flags(verts, tris, TriMeshFlags::all()).ok()
}

// ---------- Regen system ----------

/// Up to [`REGEN_BUDGET_PER_FRAME`] dirty chunks per frame: rebuild the
/// stepped-block geometry, swap in a fresh `Mesh3d` + trimesh `Collider`
/// directly on the chunk entity. Chunks loaded via `load_nearby_chunks`
/// start dirty, so this system handles both initial build and post-edit
/// updates.
pub fn regenerate_dirty_chunks(
    mut commands: Commands,
    mut terrain: ResMut<Terrain>,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain_material: Res<TerrainMaterial>,
    chunk_manager: Res<ChunkManager>,
    noise: Res<WorldNoise>,
) {
    if terrain.dirty.is_empty() {
        return;
    }
    let coords: Vec<ChunkCoord> = terrain
        .dirty
        .iter()
        .copied()
        .take(REGEN_BUDGET_PER_FRAME)
        .collect();

    for coord in coords {
        terrain.dirty.remove(&coord);
        if !terrain.chunks.contains_key(&coord) {
            continue;
        }
        let Some(&chunk_entity) = chunk_manager.loaded.get(&coord) else {
            continue;
        };

        let geom = build_chunk_geometry(coord, &terrain, &noise);
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
}
