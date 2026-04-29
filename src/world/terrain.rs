use bevy::prelude::*;
use noise::{NoiseFn, Perlin};

use super::chunks::{Chunk, CHUNK_SIZE};

const TILE_SIZE: f32 = 1.0;

#[derive(Component)]
pub struct Tile {
    pub height: f32,
}

/// Terrain height at a world-space position. Used by other systems (props, player snapping).
pub fn terrain_height(perlin: &Perlin, world_x: f64, world_z: f64) -> f32 {
    let nx = world_x * 0.05;
    let nz = world_z * 0.05;

    let height = perlin.get([nx, nz]) * 2.0
        + perlin.get([nx * 2.0, nz * 2.0]) * 0.5
        + perlin.get([nx * 4.0, nz * 4.0]) * 0.25;

    height as f32
}

/// Quantize height to stepped increments for the low-poly look.
pub fn step_height(height: f32) -> f32 {
    (height * 4.0).round() / 4.0
}

/// Determine biome type from height (used by props system too).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BiomeKind {
    Sand,
    Dirt,
    Grass,
}

pub fn biome_at_height(height: f32) -> BiomeKind {
    if height < -0.5 {
        BiomeKind::Sand
    } else if height < -0.2 {
        BiomeKind::Dirt
    } else {
        BiomeKind::Grass
    }
}

pub fn spawn_chunk_terrain(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    chunk_x: i32,
    chunk_z: i32,
    seed: u32,
) -> Entity {
    let perlin = Perlin::new(seed);

    // Color palette matching the warm, earthy art style
    let grass_colors = [
        Color::srgb(0.45, 0.65, 0.35),
        Color::srgb(0.55, 0.72, 0.40),
        Color::srgb(0.62, 0.78, 0.45),
    ];
    let dirt_color = Color::srgb(0.60, 0.48, 0.35);
    let sand_color = Color::srgb(0.82, 0.76, 0.62);

    let tile_mesh = meshes.add(Mesh::from(Cuboid::new(TILE_SIZE, 0.2, TILE_SIZE)));

    let grass_materials: Vec<_> = grass_colors
        .iter()
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: *c,
                perceptual_roughness: 0.9,
                ..default()
            })
        })
        .collect();

    let dirt_material = materials.add(StandardMaterial {
        base_color: dirt_color,
        perceptual_roughness: 0.95,
        ..default()
    });

    let sand_material = materials.add(StandardMaterial {
        base_color: sand_color,
        perceptual_roughness: 0.85,
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

            let height = terrain_height(&perlin, wx as f64, wz as f64);
            let biome = biome_at_height(height);

            let material = match biome {
                BiomeKind::Sand => sand_material.clone(),
                BiomeKind::Dirt => dirt_material.clone(),
                BiomeKind::Grass => {
                    let nx = wx as f64 * 0.05;
                    let nz = wz as f64 * 0.05;
                    let shade_noise = perlin.get([nx * 3.0 + 100.0, nz * 3.0 + 100.0]);
                    let idx = if shade_noise < -0.3 {
                        0
                    } else if shade_noise < 0.3 {
                        1
                    } else {
                        2
                    };
                    grass_materials[idx].clone()
                }
            };

            let sh = step_height(height);

            let child = commands
                .spawn((
                    Tile { height },
                    Mesh3d(tile_mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_xyz(
                        wx as f32 * TILE_SIZE,
                        sh * 0.5,
                        wz as f32 * TILE_SIZE,
                    ),
                ))
                .id();

            commands.entity(chunk_entity).add_child(child);
        }
    }

    chunk_entity
}
