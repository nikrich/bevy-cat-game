#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;
use noise::{NoiseFn, Perlin};

use super::chunks::{ChunkLoaded, CHUNK_SIZE};
use super::terrain::{biome_at_height, step_height, terrain_height, BiomeKind};

#[derive(Component)]
pub struct Prop;

#[derive(Component)]
pub enum PropKind {
    Tree,
    Rock,
    Flower,
    Bush,
    Mushroom,
}

const PROP_DENSITY_THRESHOLD: f64 = 0.55;

pub fn spawn_chunk_props(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_events: EventReader<ChunkLoaded>,
) {
    let perlin = Perlin::new(42);
    // Secondary noise for prop placement (offset seed)
    let prop_noise = Perlin::new(137);
    // Third noise for prop variety
    let variety_noise = Perlin::new(251);

    // Shared meshes for props
    let tree_trunk_mesh = meshes.add(Mesh::from(Cylinder::new(0.08, 0.5)));
    let tree_canopy_mesh = meshes.add(Mesh::from(Cone {
        radius: 0.35,
        height: 0.7,
    }));
    let rock_mesh = meshes.add(Mesh::from(Sphere::new(0.15)));
    let flower_stem_mesh = meshes.add(Mesh::from(Cylinder::new(0.02, 0.2)));
    let flower_head_mesh = meshes.add(Mesh::from(Sphere::new(0.06)));
    let bush_mesh = meshes.add(Mesh::from(Sphere::new(0.25)));
    let mushroom_cap_mesh = meshes.add(Mesh::from(Sphere::new(0.1)));
    let mushroom_stem_mesh = meshes.add(Mesh::from(Cylinder::new(0.03, 0.1)));

    // Materials
    let trunk_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.45, 0.32, 0.20),
        perceptual_roughness: 0.95,
        ..default()
    });
    let canopy_mats = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.30, 0.55, 0.25),
            perceptual_roughness: 0.9,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.35, 0.60, 0.30),
            perceptual_roughness: 0.9,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.50, 0.22),
            perceptual_roughness: 0.9,
            ..default()
        }),
    ];
    let rock_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.52, 0.48),
        perceptual_roughness: 0.95,
        ..default()
    });
    let flower_colors = [
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.90, 0.75, 0.30), // yellow
            perceptual_roughness: 0.7,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.85, 0.40, 0.45), // pink
            perceptual_roughness: 0.7,
            ..default()
        }),
        materials.add(StandardMaterial {
            base_color: Color::srgb(0.70, 0.55, 0.85), // purple
            perceptual_roughness: 0.7,
            ..default()
        }),
    ];
    let stem_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.35, 0.50, 0.25),
        perceptual_roughness: 0.9,
        ..default()
    });
    let bush_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.52, 0.28),
        perceptual_roughness: 0.9,
        ..default()
    });
    let mushroom_cap_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.75, 0.35, 0.30),
        perceptual_roughness: 0.8,
        ..default()
    });
    let mushroom_stem_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.90, 0.85, 0.75),
        perceptual_roughness: 0.85,
        ..default()
    });

    for event in chunk_events.read() {
        let world_offset_x = event.x * CHUNK_SIZE;
        let world_offset_z = event.z * CHUNK_SIZE;

        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let wx = world_offset_x + lx;
                let wz = world_offset_z + lz;

                // Check if this tile should have a prop
                let density = prop_noise.get([wx as f64 * 0.15, wz as f64 * 0.15]);
                if density < PROP_DENSITY_THRESHOLD {
                    continue;
                }

                let height = terrain_height(&perlin, wx as f64, wz as f64);
                let biome = biome_at_height(height);
                let sh = step_height(height);
                let base_y = sh * 0.5 + 0.1;

                let variety = variety_noise.get([wx as f64 * 0.3, wz as f64 * 0.3]);

                match biome {
                    BiomeKind::Grass => {
                        if variety > 0.3 {
                            // Tree
                            spawn_tree(
                                &mut commands,
                                event.entity,
                                wx as f32,
                                base_y,
                                wz as f32,
                                &tree_trunk_mesh,
                                &tree_canopy_mesh,
                                &trunk_mat,
                                &canopy_mats[(variety * 10.0) as usize % 3],
                            );
                        } else if variety > 0.0 {
                            // Bush
                            spawn_simple_prop(
                                &mut commands,
                                event.entity,
                                wx as f32,
                                base_y + 0.12,
                                wz as f32,
                                &bush_mesh,
                                &bush_mat,
                                PropKind::Bush,
                            );
                        } else if variety > -0.3 {
                            // Flower
                            spawn_flower(
                                &mut commands,
                                event.entity,
                                wx as f32,
                                base_y,
                                wz as f32,
                                &flower_stem_mesh,
                                &flower_head_mesh,
                                &stem_mat,
                                &flower_colors[((variety.abs() * 10.0) as usize) % 3],
                            );
                        } else {
                            // Mushroom
                            spawn_mushroom(
                                &mut commands,
                                event.entity,
                                wx as f32,
                                base_y,
                                wz as f32,
                                &mushroom_stem_mesh,
                                &mushroom_cap_mesh,
                                &mushroom_stem_mat,
                                &mushroom_cap_mat,
                            );
                        }
                    }
                    BiomeKind::Dirt => {
                        if variety > 0.2 {
                            // Rocks on dirt
                            spawn_simple_prop(
                                &mut commands,
                                event.entity,
                                wx as f32,
                                base_y + 0.08,
                                wz as f32,
                                &rock_mesh,
                                &rock_mat,
                                PropKind::Rock,
                            );
                        } else if variety > -0.1 {
                            // Sparse mushrooms
                            spawn_mushroom(
                                &mut commands,
                                event.entity,
                                wx as f32,
                                base_y,
                                wz as f32,
                                &mushroom_stem_mesh,
                                &mushroom_cap_mesh,
                                &mushroom_stem_mat,
                                &mushroom_cap_mat,
                            );
                        }
                    }
                    BiomeKind::Sand => {
                        if variety > 0.5 {
                            // Occasional rocks on sand
                            spawn_simple_prop(
                                &mut commands,
                                event.entity,
                                wx as f32,
                                base_y + 0.08,
                                wz as f32,
                                &rock_mesh,
                                &rock_mat,
                                PropKind::Rock,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn spawn_tree(
    commands: &mut Commands,
    chunk_entity: Entity,
    x: f32,
    base_y: f32,
    z: f32,
    trunk_mesh: &Handle<Mesh>,
    canopy_mesh: &Handle<Mesh>,
    trunk_mat: &Handle<StandardMaterial>,
    canopy_mat: &Handle<StandardMaterial>,
) {
    let tree = commands
        .spawn((
            Prop,
            PropKind::Tree,
            Transform::from_xyz(x, base_y, z),
            Visibility::default(),
        ))
        .id();

    let trunk = commands
        .spawn((
            Mesh3d(trunk_mesh.clone()),
            MeshMaterial3d(trunk_mat.clone()),
            Transform::from_xyz(0.0, 0.25, 0.0),
        ))
        .id();

    let canopy = commands
        .spawn((
            Mesh3d(canopy_mesh.clone()),
            MeshMaterial3d(canopy_mat.clone()),
            Transform::from_xyz(0.0, 0.7, 0.0),
        ))
        .id();

    commands.entity(tree).add_children(&[trunk, canopy]);
    commands.entity(chunk_entity).add_child(tree);
}

fn spawn_flower(
    commands: &mut Commands,
    chunk_entity: Entity,
    x: f32,
    base_y: f32,
    z: f32,
    stem_mesh: &Handle<Mesh>,
    head_mesh: &Handle<Mesh>,
    stem_mat: &Handle<StandardMaterial>,
    head_mat: &Handle<StandardMaterial>,
) {
    let flower = commands
        .spawn((
            Prop,
            PropKind::Flower,
            Transform::from_xyz(x, base_y, z),
            Visibility::default(),
        ))
        .id();

    let stem = commands
        .spawn((
            Mesh3d(stem_mesh.clone()),
            MeshMaterial3d(stem_mat.clone()),
            Transform::from_xyz(0.0, 0.1, 0.0),
        ))
        .id();

    let head = commands
        .spawn((
            Mesh3d(head_mesh.clone()),
            MeshMaterial3d(head_mat.clone()),
            Transform::from_xyz(0.0, 0.22, 0.0),
        ))
        .id();

    commands.entity(flower).add_children(&[stem, head]);
    commands.entity(chunk_entity).add_child(flower);
}

fn spawn_mushroom(
    commands: &mut Commands,
    chunk_entity: Entity,
    x: f32,
    base_y: f32,
    z: f32,
    stem_mesh: &Handle<Mesh>,
    cap_mesh: &Handle<Mesh>,
    stem_mat: &Handle<StandardMaterial>,
    cap_mat: &Handle<StandardMaterial>,
) {
    let mushroom = commands
        .spawn((
            Prop,
            PropKind::Mushroom,
            Transform::from_xyz(x, base_y, z),
            Visibility::default(),
        ))
        .id();

    let stem = commands
        .spawn((
            Mesh3d(stem_mesh.clone()),
            MeshMaterial3d(stem_mat.clone()),
            Transform::from_xyz(0.0, 0.05, 0.0),
        ))
        .id();

    let cap = commands
        .spawn((
            Mesh3d(cap_mesh.clone()),
            MeshMaterial3d(cap_mat.clone()),
            Transform::from_xyz(0.0, 0.12, 0.0)
                .with_scale(Vec3::new(1.0, 0.5, 1.0)),
        ))
        .id();

    commands.entity(mushroom).add_children(&[stem, cap]);
    commands.entity(chunk_entity).add_child(mushroom);
}

fn spawn_simple_prop(
    commands: &mut Commands,
    chunk_entity: Entity,
    x: f32,
    y: f32,
    z: f32,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
    kind: PropKind,
) {
    let prop = commands
        .spawn((
            Prop,
            kind,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(x, y, z),
        ))
        .id();

    commands.entity(chunk_entity).add_child(prop);
}
