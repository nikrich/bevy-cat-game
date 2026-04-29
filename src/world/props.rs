#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;
use noise::NoiseFn;

use super::biome::{Biome, WorldNoise};
use super::chunks::{ChunkLoaded, CHUNK_SIZE};
use super::terrain::step_height;

#[derive(Component)]
pub struct Prop;

#[derive(Component, Default)]
pub struct PropSway {
    pub tilt_x: f32,
    pub tilt_z: f32,
}

/// Soft "climb-on-top" collision: when the player's XZ position is within
/// `radius` of this prop, snap their Y to the prop's `top_y`. Smooth-lerped in
/// `snap_to_terrain` so the cat eases up onto rocks/boulders.
#[derive(Component, Clone, Copy)]
pub struct PropCollision {
    pub top_y: f32,
    pub radius: f32,
}

#[derive(Component)]
pub enum PropKind {
    Tree,
    PineTree,
    Cactus,
    Rock,
    Boulder,
    Flower,
    Bush,
    Mushroom,
    DeadBush,
    IceRock,
    TundraGrass,
}

const SWAY_RADIUS: f32 = 1.5;
const SWAY_STRENGTH: f32 = 0.8;
const SWAY_RECOVERY: f32 = 4.0;

/// glTF scene for a prop kind, picked deterministically by `hash` so the same
/// tile always gets the same variant across reloads. Returns the asset path
/// with `#Scene0` suffix. `None` falls back to procedural primitives.
///
/// Most kinds use the KayKit Forest Nature pack (more variety, more polish);
/// rocks/boulders still use Kenney Survival because KayKit rocks are visually
/// less distinct against terrain. Mushroom uses Kenney Food.
fn prop_scene_path(kind: &PropKind, hash: u32) -> Option<&'static str> {
    let pick = |paths: &[&'static str]| paths[(hash as usize) % paths.len()];
    Some(match kind {
        PropKind::Tree => pick(&[
            "models/kaykit_forest/Tree_1_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Tree_2_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Tree_3_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Tree_4_A_Color1.gltf#Scene0",
        ]),
        PropKind::PineTree => pick(&[
            "models/kaykit_forest/Tree_1_B_Color1.gltf#Scene0",
            "models/kaykit_forest/Tree_2_B_Color1.gltf#Scene0",
            "models/kaykit_forest/Tree_3_B_Color1.gltf#Scene0",
        ]),
        PropKind::Bush => pick(&[
            "models/kaykit_forest/Bush_1_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Bush_2_B_Color1.gltf#Scene0",
            "models/kaykit_forest/Bush_3_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Bush_4_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Bush_1_C_Color1.gltf#Scene0",
            "models/kaykit_forest/Bush_2_E_Color1.gltf#Scene0",
        ]),
        PropKind::TundraGrass => pick(&[
            "models/kaykit_forest/Grass_1_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Grass_1_B_Color1.gltf#Scene0",
            "models/kaykit_forest/Grass_2_A_Color1.gltf#Scene0",
        ]),
        PropKind::Rock => pick(&[
            "models/kaykit_forest/Rock_1_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Rock_2_A_Color1.gltf#Scene0",
            "models/kaykit_forest/Rock_3_A_Color1.gltf#Scene0",
        ]),
        PropKind::Boulder => pick(&[
            "models/kaykit_forest/Rock_1_B_Color1.gltf#Scene0",
            "models/kaykit_forest/Rock_2_B_Color1.gltf#Scene0",
            "models/kaykit_forest/Rock_3_B_Color1.gltf#Scene0",
        ]),
        PropKind::Mushroom => "models/kenney_food/mushroom.glb#Scene0",
        // Procedural fallback for the rest:
        PropKind::Cactus | PropKind::Flower | PropKind::DeadBush | PropKind::IceRock => {
            return None;
        }
    })
}

/// Position-derived hash so each tile gets a consistent variant.
fn pos_hash(x: f32, z: f32) -> u32 {
    let xi = x.round() as i64;
    let zi = z.round() as i64;
    ((xi.wrapping_mul(73)) ^ (zi.wrapping_mul(137)) ^ 0x9E37_79B1) as u32
}

/// Per-kind scale and Y lift to make models sit on top of the terrain.
/// KayKit models are authored at unit-meter scale so most need a small bump
/// rather than a large multiplier; rocks/boulders still use Kenney sizing.
fn prop_scene_transform(kind: &PropKind) -> (f32, f32) {
    match kind {
        PropKind::Tree => (0.6, 0.0),
        PropKind::PineTree => (0.7, 0.0),
        PropKind::Rock => (0.9, 0.05),
        PropKind::Boulder => (1.1, 0.05),
        PropKind::Mushroom => (1.4, 0.05),
        PropKind::Bush => (1.0, 0.05),
        PropKind::TundraGrass => (1.0, 0.05),
        _ => (1.0, 0.0),
    }
}

/// Climb-on-top collision (player snaps Y to top of this prop when nearby).
/// Returns (height_above_origin_unscaled, radius_xz_unscaled). `None` for
/// props that should be pass-through visually (trees, ground cover, etc.).
///
/// Heights are tuned so `top_y` matches the visible top of the Kenney mesh
/// after the model's own scale is applied -- so the cat rests on the rock,
/// not floating above it.
fn prop_climb(kind: &PropKind) -> Option<(f32, f32)> {
    match kind {
        PropKind::Rock => Some((0.15, 0.30)),
        PropKind::Boulder => Some((0.30, 0.45)),
        PropKind::Mushroom => Some((0.20, 0.22)),
        _ => None,
    }
}

/// Spawn a Kenney glTF prop attached to a chunk. Returns true if a scene was
/// spawned (caller should skip the procedural path).
fn try_spawn_kenney_prop(
    commands: &mut Commands,
    chunk: Entity,
    asset_server: &AssetServer,
    kind: PropKind,
    x: f32,
    y: f32,
    z: f32,
) -> bool {
    let hash = pos_hash(x, z);
    let Some(path) = prop_scene_path(&kind, hash) else {
        return false;
    };
    let (scale, lift) = prop_scene_transform(&kind);
    let climb = prop_climb(&kind);
    let prop_y = y + lift;

    let prop = commands
        .spawn((
            Prop,
            PropSway::default(),
            kind,
            SceneRoot(asset_server.load(path)),
            Transform::from_xyz(x, prop_y, z).with_scale(Vec3::splat(scale)),
            Visibility::default(),
        ))
        .id();
    if let Some((height, radius)) = climb {
        commands.entity(prop).insert(PropCollision {
            top_y: prop_y + height * scale,
            radius: radius * scale,
        });
        // Spawn the rapier collider as a child entity translated up to the
        // prop's vertical centre. Centring the cuboid on the prop entity
        // would put it at `prop_y` (the foot), so the cat would collide
        // with empty air and clip through the model. The child's local
        // Transform handles the offset cleanly. Footprint a hair tighter
        // than the PropCollision climb radius so the cat doesn't snag on
        // the collider edge before its feet are over the visible prop.
        let half_h = height * 0.5;
        let half_r = radius * 0.7;
        let collider = commands
            .spawn((
                Transform::from_xyz(0.0, half_h, 0.0),
                bevy_rapier3d::prelude::Collider::cuboid(half_r, half_h, half_r),
                bevy_rapier3d::prelude::RigidBody::Fixed,
            ))
            .id();
        commands.entity(prop).add_child(collider);
    }
    commands.entity(chunk).add_child(prop);
    true
}

/// Shared mesh/material handles for all prop types.
struct PropAssets {
    // Meshes
    trunk: Handle<Mesh>,
    canopy: Handle<Mesh>,
    pine_canopy: Handle<Mesh>,
    rock: Handle<Mesh>,
    boulder: Handle<Mesh>,
    flower_stem: Handle<Mesh>,
    flower_head: Handle<Mesh>,
    bush: Handle<Mesh>,
    mushroom_stem: Handle<Mesh>,
    mushroom_cap: Handle<Mesh>,
    cactus_body: Handle<Mesh>,
    dead_bush: Handle<Mesh>,
    tundra_grass: Handle<Mesh>,
    ice_rock: Handle<Mesh>,

    // Materials
    trunk_mat: Handle<StandardMaterial>,
    canopy_mats: [Handle<StandardMaterial>; 3],
    pine_mats: [Handle<StandardMaterial>; 2],
    rock_mat: Handle<StandardMaterial>,
    dark_rock_mat: Handle<StandardMaterial>,
    flower_colors: [Handle<StandardMaterial>; 4],
    stem_mat: Handle<StandardMaterial>,
    bush_mat: Handle<StandardMaterial>,
    mushroom_cap_mat: Handle<StandardMaterial>,
    mushroom_stem_mat: Handle<StandardMaterial>,
    cactus_mat: Handle<StandardMaterial>,
    dead_bush_mat: Handle<StandardMaterial>,
    tundra_grass_mat: Handle<StandardMaterial>,
    ice_rock_mat: Handle<StandardMaterial>,
    snow_rock_mat: Handle<StandardMaterial>,
}

impl PropAssets {
    fn new(
        meshes: &mut ResMut<Assets<Mesh>>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> Self {
        Self {
            // Meshes
            trunk: meshes.add(Mesh::from(Cylinder::new(0.08, 0.5))),
            canopy: meshes.add(Mesh::from(Cone { radius: 0.35, height: 0.7 })),
            pine_canopy: meshes.add(Mesh::from(Cone { radius: 0.25, height: 0.9 })),
            rock: meshes.add(Mesh::from(Sphere::new(0.15))),
            boulder: meshes.add(Mesh::from(Sphere::new(0.3))),
            flower_stem: meshes.add(Mesh::from(Cylinder::new(0.02, 0.2))),
            flower_head: meshes.add(Mesh::from(Sphere::new(0.06))),
            bush: meshes.add(Mesh::from(Sphere::new(0.25))),
            mushroom_stem: meshes.add(Mesh::from(Cylinder::new(0.03, 0.1))),
            mushroom_cap: meshes.add(Mesh::from(Sphere::new(0.1))),
            cactus_body: meshes.add(Mesh::from(Cylinder::new(0.1, 0.5))),
            dead_bush: meshes.add(Mesh::from(Sphere::new(0.15))),
            tundra_grass: meshes.add(Mesh::from(Cylinder::new(0.04, 0.15))),
            ice_rock: meshes.add(Mesh::from(Cuboid::new(0.2, 0.25, 0.2))),

            // Materials
            trunk_mat: materials.add(StandardMaterial {
                base_color: Color::srgb(0.45, 0.32, 0.20),
                perceptual_roughness: 0.95,
                ..default()
            }),
            canopy_mats: [
                materials.add(StandardMaterial { base_color: Color::srgb(0.30, 0.55, 0.25), perceptual_roughness: 0.9, ..default() }),
                materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.60, 0.30), perceptual_roughness: 0.9, ..default() }),
                materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.50, 0.22), perceptual_roughness: 0.9, ..default() }),
            ],
            pine_mats: [
                materials.add(StandardMaterial { base_color: Color::srgb(0.18, 0.38, 0.22), perceptual_roughness: 0.9, ..default() }),
                materials.add(StandardMaterial { base_color: Color::srgb(0.22, 0.42, 0.26), perceptual_roughness: 0.9, ..default() }),
            ],
            rock_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.52, 0.48), perceptual_roughness: 0.95, ..default() }),
            dark_rock_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.42, 0.40, 0.38), perceptual_roughness: 0.95, ..default() }),
            flower_colors: [
                materials.add(StandardMaterial { base_color: Color::srgb(0.90, 0.75, 0.30), perceptual_roughness: 0.7, ..default() }),
                materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.40, 0.45), perceptual_roughness: 0.7, ..default() }),
                materials.add(StandardMaterial { base_color: Color::srgb(0.70, 0.55, 0.85), perceptual_roughness: 0.7, ..default() }),
                materials.add(StandardMaterial { base_color: Color::srgb(0.95, 0.90, 0.70), perceptual_roughness: 0.7, ..default() }),
            ],
            stem_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.50, 0.25), perceptual_roughness: 0.9, ..default() }),
            bush_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.32, 0.52, 0.28), perceptual_roughness: 0.9, ..default() }),
            mushroom_cap_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.35, 0.30), perceptual_roughness: 0.8, ..default() }),
            mushroom_stem_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.90, 0.85, 0.75), perceptual_roughness: 0.85, ..default() }),
            cactus_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.55, 0.30), perceptual_roughness: 0.85, ..default() }),
            dead_bush_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.45, 0.30), perceptual_roughness: 0.95, ..default() }),
            tundra_grass_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.50, 0.52, 0.42), perceptual_roughness: 0.9, ..default() }),
            ice_rock_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.78, 0.82, 0.88), perceptual_roughness: 0.5, ..default() }),
            snow_rock_mat: materials.add(StandardMaterial { base_color: Color::srgb(0.85, 0.86, 0.90), perceptual_roughness: 0.7, ..default() }),
        }
    }
}

pub fn spawn_chunk_props(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    noise: Res<WorldNoise>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_events: MessageReader<ChunkLoaded>,
) {
    let events: Vec<_> = chunk_events.read().collect();
    if events.is_empty() {
        return;
    }

    let prop_noise = &noise.moisture; // reuse for prop placement
    let variety_noise = &noise.temperature; // reuse for variety
    let assets = PropAssets::new(&mut meshes, &mut materials);

    for event in &events {
        let world_offset_x = event.x * CHUNK_SIZE;
        let world_offset_z = event.z * CHUNK_SIZE;

        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let wx = world_offset_x + lx;
                let wz = world_offset_z + lz;

                let sample = noise.sample(wx as f64, wz as f64);

                // No props on water
                if sample.biome.is_water() {
                    continue;
                }

                // Density check -- each biome has different density
                let density_threshold = match sample.biome {
                    Biome::Forest => 0.35,
                    Biome::Meadow => 0.45,
                    Biome::Grassland => 0.55,
                    Biome::Taiga => 0.40,
                    Biome::Desert => 0.75,
                    Biome::Beach => 0.85,
                    Biome::Tundra => 0.65,
                    Biome::Mountain => 0.70,
                    Biome::Snow => 0.80,
                    Biome::Ocean => 1.0,
                };

                let density = prop_noise.get([wx as f64 * 0.15, wz as f64 * 0.15]).abs() as f32;
                if density < density_threshold {
                    continue;
                }

                let sh = step_height(sample.elevation * sample.biome.height_scale());
                let base_y = sh * 0.5 + 0.1;
                let variety = variety_noise.get([wx as f64 * 0.3, wz as f64 * 0.3]) as f32;

                spawn_biome_prop(
                    &mut commands,
                    &asset_server,
                    event.entity,
                    wx as f32,
                    base_y,
                    wz as f32,
                    sample.biome,
                    variety,
                    &assets,
                );
            }
        }
    }
}

fn spawn_biome_prop(
    commands: &mut Commands,
    asset_server: &AssetServer,
    chunk: Entity,
    x: f32,
    y: f32,
    z: f32,
    biome: Biome,
    variety: f32,
    assets: &PropAssets,
) {
    // Pick the prop kind for this (biome, variety) cell. Decoupled from spawning
    // so we can route through Kenney scenes when available without duplicating
    // the per-biome decision tree.
    let kind = match biome {
        Biome::Grassland => {
            if variety > 0.3 { PropKind::Tree }
            else if variety > 0.0 { PropKind::Bush }
            else if variety > -0.3 { PropKind::Flower }
            else { PropKind::Mushroom }
        }
        Biome::Forest => {
            if variety > -0.2 { PropKind::Tree }
            else if variety > -0.5 { PropKind::Bush }
            else { PropKind::Mushroom }
        }
        Biome::Meadow => {
            if variety > 0.4 { PropKind::Tree }
            else if variety > -0.2 { PropKind::Flower }
            else { PropKind::Bush }
        }
        Biome::Taiga => {
            if variety > -0.3 { PropKind::PineTree } else { PropKind::Rock }
        }
        Biome::Desert => {
            if variety > 0.2 { PropKind::Cactus }
            else if variety > -0.2 { PropKind::Rock }
            else { PropKind::DeadBush }
        }
        Biome::Beach => PropKind::Rock,
        Biome::Tundra => {
            if variety > 0.2 { PropKind::TundraGrass } else { PropKind::Rock }
        }
        Biome::Mountain => {
            if variety > 0.0 { PropKind::Boulder } else { PropKind::Rock }
        }
        Biome::Snow => {
            if variety > 0.3 { PropKind::IceRock }
            else if variety > 0.0 { PropKind::Rock }
            else { return }
        }
        Biome::Ocean => return,
    };

    // Kenney glTF first; fall back to procedural primitives for the rest.
    match kind {
        PropKind::Tree
        | PropKind::PineTree
        | PropKind::Rock
        | PropKind::Boulder
        | PropKind::Mushroom
        | PropKind::Bush
        | PropKind::TundraGrass => {
            try_spawn_kenney_prop(commands, chunk, asset_server, kind, x, y, z);
        }
        PropKind::Cactus => spawn_cactus(commands, chunk, x, y, z, assets),
        PropKind::Flower => spawn_flower(commands, chunk, x, y, z, assets, variety),
        PropKind::DeadBush => spawn_dead_bush(commands, chunk, x, y + 0.05, z, assets),
        PropKind::IceRock => spawn_simple(
            commands, chunk, x, y + 0.1, z,
            &assets.ice_rock, &assets.ice_rock_mat, PropKind::IceRock,
        ),
    }
}

// --- Spawners ---

fn spawn_tree(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets, _tall: bool) {
    let canopy_idx = ((x * 7.3 + z * 13.7).abs() as usize) % 3;
    let tree = commands
        .spawn((Prop, PropSway::default(), PropKind::Tree, Transform::from_xyz(x, y, z), Visibility::default()))
        .id();
    let trunk = commands
        .spawn((Mesh3d(assets.trunk.clone()), MeshMaterial3d(assets.trunk_mat.clone()), Transform::from_xyz(0.0, 0.25, 0.0)))
        .id();
    let canopy = commands
        .spawn((Mesh3d(assets.canopy.clone()), MeshMaterial3d(assets.canopy_mats[canopy_idx].clone()), Transform::from_xyz(0.0, 0.7, 0.0)))
        .id();
    commands.entity(tree).add_children(&[trunk, canopy]);
    commands.entity(chunk).add_child(tree);
}

fn spawn_pine(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets, variety: f32) {
    let mat_idx = if variety > 0.0 { 0 } else { 1 };
    let tree = commands
        .spawn((Prop, PropSway::default(), PropKind::PineTree, Transform::from_xyz(x, y, z), Visibility::default()))
        .id();
    let trunk = commands
        .spawn((Mesh3d(assets.trunk.clone()), MeshMaterial3d(assets.trunk_mat.clone()), Transform::from_xyz(0.0, 0.25, 0.0)))
        .id();
    let canopy = commands
        .spawn((Mesh3d(assets.pine_canopy.clone()), MeshMaterial3d(assets.pine_mats[mat_idx].clone()), Transform::from_xyz(0.0, 0.75, 0.0)))
        .id();
    commands.entity(tree).add_children(&[trunk, canopy]);
    commands.entity(chunk).add_child(tree);
}

fn spawn_bush(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets) {
    let bush = commands
        .spawn((Prop, PropSway::default(), PropKind::Bush, Mesh3d(assets.bush.clone()), MeshMaterial3d(assets.bush_mat.clone()), Transform::from_xyz(x, y, z)))
        .id();
    commands.entity(chunk).add_child(bush);
}

fn spawn_flower(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets, variety: f32) {
    let color_idx = ((variety.abs() * 17.0) as usize) % 4;
    let flower = commands
        .spawn((Prop, PropSway::default(), PropKind::Flower, Transform::from_xyz(x, y, z), Visibility::default()))
        .id();
    let stem = commands
        .spawn((Mesh3d(assets.flower_stem.clone()), MeshMaterial3d(assets.stem_mat.clone()), Transform::from_xyz(0.0, 0.1, 0.0)))
        .id();
    let head = commands
        .spawn((Mesh3d(assets.flower_head.clone()), MeshMaterial3d(assets.flower_colors[color_idx].clone()), Transform::from_xyz(0.0, 0.22, 0.0)))
        .id();
    commands.entity(flower).add_children(&[stem, head]);
    commands.entity(chunk).add_child(flower);
}

fn spawn_mushroom(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets) {
    let mushroom = commands
        .spawn((Prop, PropSway::default(), PropKind::Mushroom, Transform::from_xyz(x, y, z), Visibility::default()))
        .id();
    let stem = commands
        .spawn((Mesh3d(assets.mushroom_stem.clone()), MeshMaterial3d(assets.mushroom_stem_mat.clone()), Transform::from_xyz(0.0, 0.05, 0.0)))
        .id();
    let cap = commands
        .spawn((Mesh3d(assets.mushroom_cap.clone()), MeshMaterial3d(assets.mushroom_cap_mat.clone()), Transform::from_xyz(0.0, 0.12, 0.0).with_scale(Vec3::new(1.0, 0.5, 1.0))))
        .id();
    commands.entity(mushroom).add_children(&[stem, cap]);
    commands.entity(chunk).add_child(mushroom);
}

fn spawn_cactus(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets) {
    let cactus = commands
        .spawn((Prop, PropSway::default(), PropKind::Cactus, Mesh3d(assets.cactus_body.clone()), MeshMaterial3d(assets.cactus_mat.clone()), Transform::from_xyz(x, y + 0.25, z)))
        .id();
    commands.entity(chunk).add_child(cactus);
}

fn spawn_dead_bush(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets) {
    let bush = commands
        .spawn((Prop, PropSway::default(), PropKind::DeadBush, Mesh3d(assets.dead_bush.clone()), MeshMaterial3d(assets.dead_bush_mat.clone()), Transform::from_xyz(x, y, z)))
        .id();
    commands.entity(chunk).add_child(bush);
}

fn spawn_simple(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>, kind: PropKind) {
    let prop = commands
        .spawn((Prop, kind, Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_xyz(x, y, z)))
        .id();
    commands.entity(chunk).add_child(prop);
}

fn spawn_simple_sway(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>, kind: PropKind) {
    let prop = commands
        .spawn((Prop, PropSway::default(), kind, Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::from_xyz(x, y, z)))
        .id();
    commands.entity(chunk).add_child(prop);
}

// --- Sway systems ---

pub fn sway_props_near_player(
    player_query: Query<&GlobalTransform, With<crate::player::Player>>,
    mut props: Query<(&GlobalTransform, &mut PropSway, &PropKind), With<Prop>>,
    time: Res<Time>,
) -> Result {
    let player_pos = player_query.single()?.translation();
    let dt = time.delta_secs();

    for (global_tf, mut sway, kind) in &mut props {
        if matches!(kind, PropKind::Rock | PropKind::Boulder | PropKind::IceRock) {
            continue;
        }

        let prop_pos = global_tf.translation();
        let dx = prop_pos.x - player_pos.x;
        let dz = prop_pos.z - player_pos.z;
        let dist_sq = dx * dx + dz * dz;

        if dist_sq < SWAY_RADIUS * SWAY_RADIUS && dist_sq > 0.01 {
            let dist = dist_sq.sqrt();
            let strength = (1.0 - dist / SWAY_RADIUS) * SWAY_STRENGTH;

            let scale = match kind {
                PropKind::Tree | PropKind::PineTree => 0.6,
                PropKind::Cactus => 0.3,
                PropKind::Bush | PropKind::DeadBush => 1.0,
                PropKind::Flower => 1.4,
                PropKind::Mushroom => 0.8,
                PropKind::TundraGrass => 1.2,
                PropKind::Rock | PropKind::Boulder | PropKind::IceRock => 0.0,
            };

            let push_x = (dx / dist) * strength * scale;
            let push_z = (dz / dist) * strength * scale;

            sway.tilt_x += (push_x - sway.tilt_x) * (12.0 * dt).min(1.0);
            sway.tilt_z += (push_z - sway.tilt_z) * (12.0 * dt).min(1.0);
        } else {
            sway.tilt_x *= 1.0 - (SWAY_RECOVERY * dt).min(1.0);
            sway.tilt_z *= 1.0 - (SWAY_RECOVERY * dt).min(1.0);
        }
    }

    Ok(())
}

pub fn apply_prop_sway(mut props: Query<(&PropSway, &mut Transform), With<Prop>>) {
    for (sway, mut transform) in &mut props {
        if sway.tilt_x.abs() > 0.001 || sway.tilt_z.abs() > 0.001 {
            transform.rotation = Quat::from_euler(EulerRot::XZY, sway.tilt_z, 0.0, -sway.tilt_x);
        } else {
            transform.rotation = Quat::IDENTITY;
        }
    }
}
