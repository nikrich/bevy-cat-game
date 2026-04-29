//! Per-chunk water mesh (Phase 1 / W1.6).
//!
//! Each loaded chunk gets a single flat quad child at sea level. Where the
//! terrain mesh sits below the water plane it shows through; where terrain
//! sits above, the depth buffer hides the water. The result is roughly the
//! same coastline shape as before without per-tile entities.
//!
//! The previous per-tile bobbing wave shader is parked: a flat plane has
//! no per-vertex motion, and re-implementing the swell needs either a real
//! shader or per-frame mesh mutation. Tracked as DEBT-019.

use bevy::prelude::*;

use super::biome::SEA_LEVEL;
use super::chunks::{ChunkLoaded, CHUNK_SIZE};
use super::terrain::step_height;

/// Marker for the per-chunk water plane.
#[derive(Component)]
pub struct WaterPlane;

/// Y-coordinate of the water surface. Matches the previous per-tile water
/// mesh so the cat's wading depth doesn't change.
fn water_y() -> f32 {
    step_height(SEA_LEVEL) * 0.5 - 0.15
}

/// Cached handles for the chunk-sized water quad and its material so every
/// chunk's water plane shares one mesh + one material.
#[derive(Resource)]
pub struct WaterAssets {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

impl FromWorld for WaterAssets {
    fn from_world(world: &mut World) -> Self {
        let mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Mesh::from(Plane3d::default().mesh().size(
                CHUNK_SIZE as f32,
                CHUNK_SIZE as f32,
            )))
        };
        let material = {
            let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
            materials.add(StandardMaterial {
                base_color: Color::srgba(0.20, 0.38, 0.55, 0.85),
                emissive: Color::srgb(0.03, 0.06, 0.10).into(),
                perceptual_roughness: 0.08,
                metallic: 0.0,
                reflectance: 0.8,
                alpha_mode: AlphaMode::Blend,
                ..default()
            })
        };
        Self { mesh, material }
    }
}

/// Spawn one water plane per newly-loaded chunk, parented to the chunk
/// entity and positioned at the chunk centre in chunk-local space.
pub fn spawn_chunk_water(
    mut commands: Commands,
    assets: Res<WaterAssets>,
    mut events: MessageReader<ChunkLoaded>,
) {
    for event in events.read() {
        let half = CHUNK_SIZE as f32 * 0.5;
        let water = commands
            .spawn((
                WaterPlane,
                Mesh3d(assets.mesh.clone()),
                MeshMaterial3d(assets.material.clone()),
                Transform::from_xyz(half, water_y(), half),
            ))
            .id();
        commands.entity(event.entity).add_child(water);
    }
}
