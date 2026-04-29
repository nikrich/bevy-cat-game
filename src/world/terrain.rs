use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, RigidBody};

use super::biome::{Biome, WorldNoise, SEA_LEVEL};
use super::chunks::{Chunk, CHUNK_SIZE};

const TILE_SIZE: f32 = 1.0;

#[derive(Component)]
pub struct Tile {
    pub height: f32,
    pub biome: Biome,
}

#[derive(Component)]
pub struct WaterTile;

/// Quantize height to stepped increments for the low-poly look.
pub fn step_height(height: f32) -> f32 {
    (height * 4.0).round() / 4.0
}

pub fn spawn_chunk_terrain(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    noise: &WorldNoise,
    chunk_x: i32,
    chunk_z: i32,
) -> Entity {
    let tile_mesh = meshes.add(Mesh::from(Cuboid::new(TILE_SIZE, 0.6, TILE_SIZE)));
    let water_mesh = meshes.add(Mesh::from(Cuboid::new(TILE_SIZE, 0.4, TILE_SIZE)));

    // Pre-build materials per biome (cache to avoid duplicates within a chunk)
    let mut material_cache: std::collections::HashMap<(Biome, u8), Handle<StandardMaterial>> =
        std::collections::HashMap::new();

    let water_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.20, 0.38, 0.55),
        emissive: Color::srgb(0.03, 0.06, 0.10).into(),
        perceptual_roughness: 0.08,
        metallic: 0.0,
        reflectance: 0.8,
        ..default()
    });

    let world_offset_x = chunk_x * CHUNK_SIZE;
    let world_offset_z = chunk_z * CHUNK_SIZE;

    let chunk_entity = commands
        .spawn((
            Chunk {
                x: chunk_x,
                z: chunk_z,
            },
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    for lx in 0..CHUNK_SIZE {
        for lz in 0..CHUNK_SIZE {
            let wx = world_offset_x + lx;
            let wz = world_offset_z + lz;

            let sample = noise.sample(wx as f64, wz as f64);

            // Color variation within biome
            let shade_hash = ((wx * 7 + wz * 13).unsigned_abs() % 3) as u8;

            let cache_key = (sample.biome, shade_hash);
            let material = material_cache
                .entry(cache_key)
                .or_insert_with(|| {
                    materials.add(StandardMaterial {
                        base_color: sample.biome.terrain_color(shade_hash),
                        perceptual_roughness: sample.biome.roughness(),
                        ..default()
                    })
                })
                .clone();

            let sh = step_height(sample.elevation * sample.biome.height_scale());

            // Each terrain tile carries a 1.0 x 0.6 x 1.0 cuboid `Collider`
            // matching its mesh; rapier reads this as a static fixed body
            // (the implicit default when no `RigidBody` is present is fine,
            // but we tag `RigidBody::Fixed` explicitly for clarity). The
            // per-tile cuboid is intentionally throwaway: Phase 1 replaces
            // terrain with a vertex-height grid mesh and a `Collider::trimesh`.
            let tile_collider = || (Collider::cuboid(0.5, 0.3, 0.5), RigidBody::Fixed);

            if sample.biome.is_water() {
                // Wade-friendly floor: tuned so a float_height=1.0 cat
                // settles with its centre at y≈0 — capsule bottom (~-0.7)
                // well submerged inside the water mesh (top at -0.2),
                // capsule top (~+0.7) clearing the surface. Reproduces the
                // half-submerged wading look the old `snap_to_terrain` had,
                // and leaves the Jump arc (peak +1.6) tall enough to clear
                // the surrounding beach tiles.
                let floor_y = step_height(SEA_LEVEL) * 0.5 - 1.05;
                let floor = commands
                    .spawn((
                        Tile {
                            height: sample.elevation,
                            biome: sample.biome,
                        },
                        Mesh3d(tile_mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::from_xyz(
                            wx as f32 * TILE_SIZE,
                            floor_y,
                            wz as f32 * TILE_SIZE,
                        ),
                        tile_collider(),
                    ))
                    .id();

                // Water surface mesh is purely visual — no collider, so the
                // cat sinks past it onto the wade-friendly floor below.
                let water_y = step_height(SEA_LEVEL) * 0.5 - 0.15;
                let water = commands
                    .spawn((
                        WaterTile,
                        Mesh3d(water_mesh.clone()),
                        MeshMaterial3d(water_material.clone()),
                        Transform::from_xyz(
                            wx as f32 * TILE_SIZE,
                            water_y,
                            wz as f32 * TILE_SIZE,
                        ),
                    ))
                    .id();

                commands
                    .entity(chunk_entity)
                    .add_children(&[floor, water]);
            } else {
                let child = commands
                    .spawn((
                        Tile {
                            height: sample.elevation,
                            biome: sample.biome,
                        },
                        Mesh3d(tile_mesh.clone()),
                        MeshMaterial3d(material),
                        Transform::from_xyz(
                            wx as f32 * TILE_SIZE,
                            sh * 0.5,
                            wz as f32 * TILE_SIZE,
                        ),
                        tile_collider(),
                    ))
                    .id();

                commands.entity(chunk_entity).add_child(child);
            }
        }
    }

    chunk_entity
}
