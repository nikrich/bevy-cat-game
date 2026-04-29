#![allow(clippy::too_many_arguments)]

use bevy::prelude::*;
use noise::NoiseFn;

use std::collections::HashSet;

use super::biome::{Biome, WorldNoise};
use super::chunks::{Chunk, ChunkLoaded, ChunkManager, CHUNK_SIZE};
use super::terrain::{ChunkCoord, Terrain};

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

/// Terrain surface Y the prop was last anchored against. Used by
/// [`snap_props_to_terrain`] to follow Raise/Lower/Smooth/Flatten brush
/// edits — when the chunk's mesh regenerates, the delta between
/// `terrain_y` and the new surface height is applied to the prop's
/// `Transform.y` (and `PropCollision.top_y`), so trees/rocks rise and
/// fall with the ground instead of hanging in air or getting buried.
///
/// We store the terrain height (not the prop's Y) because each prop type
/// adds its own visual lift on top — recording the terrain reference
/// keeps that lift implicit in the current Transform and lets the snap
/// system work in pure deltas.
#[derive(Component)]
pub struct PropTerrainAnchor {
    pub terrain_y: f32,
}

/// Cell coordinates a prop belongs to, so the Paint brush respawn system
/// can find and despawn props in cells whose biome just changed. `(cx, cz)`
/// is the chunk coord; `(lx, lz)` is the chunk-local cell index in
/// `0..CHUNK_SIZE`.
#[derive(Component, Clone, Copy)]
pub struct PropCell {
    pub cx: i32,
    pub cz: i32,
    pub lx: u8,
    pub lz: u8,
}

/// Pop-in animation tag for props that were spawned by the Paint brush
/// respawn pass. Initial chunk-load props skip this so the world doesn't
/// pop in every time you walk; only paint-driven appearances pop.
///
/// `base_scale` is `None` until the first tick — we snapshot the prop's
/// current `Transform.scale` then, snap to zero, and animate back up.
/// This way the spawner doesn't need to know the per-kind scale to set
/// up the animation; it just inserts the marker.
#[derive(Component)]
pub struct PropSpawnPop {
    pub elapsed: f32,
    pub duration: f32,
    pub base_scale: Option<Vec3>,
}

impl Default for PropSpawnPop {
    fn default() -> Self {
        Self {
            elapsed: 0.0,
            duration: 0.35,
            base_scale: None,
        }
    }
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

/// Spawn a Kenney glTF prop attached to a chunk. Returns the spawned
/// prop entity, or `None` if `kind` doesn't have a scene path (caller
/// should fall back to the procedural primitive path).
fn try_spawn_kenney_prop(
    commands: &mut Commands,
    chunk: Entity,
    asset_server: &AssetServer,
    kind: PropKind,
    x: f32,
    y: f32,
    z: f32,
    cell: PropCell,
) -> Option<Entity> {
    let hash = pos_hash(x, z);
    let path = prop_scene_path(&kind, hash)?;
    let (scale, lift) = prop_scene_transform(&kind);
    let climb = prop_climb(&kind);
    let prop_y = y + lift;

    let prop = commands
        .spawn((
            Prop,
            PropSway::default(),
            kind,
            PropTerrainAnchor { terrain_y: y },
            cell,
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
    Some(prop)
}

/// Shared mesh/material handles for all prop types. Built once at
/// startup so prop spawn paths (initial chunk load + Paint brush
/// respawn) reuse the same handles instead of rebuilding the table per
/// invocation.
#[derive(Resource)]
pub struct PropAssets {
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

impl FromWorld for PropAssets {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let trunk = meshes.add(Mesh::from(Cylinder::new(0.08, 0.5)));
        let canopy = meshes.add(Mesh::from(Cone { radius: 0.35, height: 0.7 }));
        let pine_canopy = meshes.add(Mesh::from(Cone { radius: 0.25, height: 0.9 }));
        let rock = meshes.add(Mesh::from(Sphere::new(0.15)));
        let boulder = meshes.add(Mesh::from(Sphere::new(0.3)));
        let flower_stem = meshes.add(Mesh::from(Cylinder::new(0.02, 0.2)));
        let flower_head = meshes.add(Mesh::from(Sphere::new(0.06)));
        let bush = meshes.add(Mesh::from(Sphere::new(0.25)));
        let mushroom_stem = meshes.add(Mesh::from(Cylinder::new(0.03, 0.1)));
        let mushroom_cap = meshes.add(Mesh::from(Sphere::new(0.1)));
        let cactus_body = meshes.add(Mesh::from(Cylinder::new(0.1, 0.5)));
        let dead_bush = meshes.add(Mesh::from(Sphere::new(0.15)));
        let tundra_grass = meshes.add(Mesh::from(Cylinder::new(0.04, 0.15)));
        let ice_rock = meshes.add(Mesh::from(Cuboid::new(0.2, 0.25, 0.2)));
        drop(meshes);

        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        Self {
            // Meshes
            trunk,
            canopy,
            pine_canopy,
            rock,
            boulder,
            flower_stem,
            flower_head,
            bush,
            mushroom_stem,
            mushroom_cap,
            cactus_body,
            dead_bush,
            tundra_grass,
            ice_rock,
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
    terrain: Res<Terrain>,
    assets: Res<PropAssets>,
    mut chunk_events: MessageReader<ChunkLoaded>,
) {
    let events: Vec<_> = chunk_events.read().collect();
    if events.is_empty() {
        return;
    }

    let prop_noise = &noise.moisture; // reuse for prop placement
    let variety_noise = &noise.temperature; // reuse for variety

    for event in &events {
        for lx in 0..CHUNK_SIZE {
            for lz in 0..CHUNK_SIZE {
                let wx = event.x * CHUNK_SIZE + lx;
                let wz = event.z * CHUNK_SIZE + lz;
                // Read biome off the chunk's vertex grid — that's the
                // PCG biome with `biome_edits` already re-applied from
                // a save, so painted cells persist across reload. Fall
                // back to PCG if the chunk somehow isn't loaded by now
                // (it should be: we're handling its `ChunkLoaded`).
                let biome = terrain
                    .vertex_biome(wx, wz)
                    .unwrap_or_else(|| noise.sample(wx as f64, wz as f64).biome);
                try_spawn_cell_prop(
                    &mut commands,
                    &asset_server,
                    event.entity,
                    event.x,
                    event.z,
                    lx as u8,
                    lz as u8,
                    biome,
                    &terrain,
                    &noise,
                    &assets,
                );
            }
        }
    }
    // Silence unused-warnings on the cached aliases (kept for symmetry
    // with how the inner helper destructures `noise`).
    let _ = (prop_noise, variety_noise);
}

/// Spawn one prop in a cell if its (biome, density) sample passes the
/// per-biome threshold. Shared by initial chunk-load spawn and Paint
/// brush respawn — the only difference between the two callers is which
/// `biome` they pass (PCG sample vs. the painted overlay).
///
/// Returns the spawned prop's `Entity`, or `None` when the cell either
/// failed the density gate or holds a biome with no spawnable variant
/// at the rolled `variety`.
fn try_spawn_cell_prop(
    commands: &mut Commands,
    asset_server: &AssetServer,
    chunk_entity: Entity,
    cx: i32,
    cz: i32,
    lx: u8,
    lz: u8,
    biome: Biome,
    terrain: &Terrain,
    noise: &WorldNoise,
    assets: &PropAssets,
) -> Option<Entity> {
    if biome.is_water() {
        return None;
    }
    let density_threshold = match biome {
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
    let wx = cx * CHUNK_SIZE + lx as i32;
    let wz = cz * CHUNK_SIZE + lz as i32;
    let density = noise.moisture.get([wx as f64 * 0.15, wz as f64 * 0.15]).abs() as f32;
    if density < density_threshold {
        return None;
    }
    // Props are children of the chunk entity, which sits at the chunk's
    // NW corner — so we pass chunk-local x/z here, not world-space.
    let base_y = terrain.height_at_or_sample(wx as f32, wz as f32, noise);
    let variety = noise.temperature.get([wx as f64 * 0.3, wz as f64 * 0.3]) as f32;
    spawn_biome_prop(
        commands,
        asset_server,
        chunk_entity,
        lx as f32,
        base_y,
        lz as f32,
        biome,
        variety,
        assets,
        PropCell { cx, cz, lx, lz },
    )
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
    cell: PropCell,
) -> Option<Entity> {
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
            else { return None }
        }
        Biome::Ocean => return None,
    };

    // Kenney glTF first; fall back to procedural primitives for the rest.
    Some(match kind {
        PropKind::Tree
        | PropKind::PineTree
        | PropKind::Rock
        | PropKind::Boulder
        | PropKind::Mushroom
        | PropKind::Bush
        | PropKind::TundraGrass => {
            try_spawn_kenney_prop(commands, chunk, asset_server, kind, x, y, z, cell)?
        }
        PropKind::Cactus => spawn_cactus(commands, chunk, x, y, z, assets, cell),
        PropKind::Flower => spawn_flower(commands, chunk, x, y, z, assets, variety, cell),
        // Internal lift moved into the spawner so `y` here remains the
        // reference terrain height for `PropTerrainAnchor`.
        PropKind::DeadBush => spawn_dead_bush(commands, chunk, x, y, z, assets, cell),
        PropKind::IceRock => spawn_ice_rock(commands, chunk, x, y, z, assets, cell),
    })
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

fn spawn_flower(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets, variety: f32, cell: PropCell) -> Entity {
    let color_idx = ((variety.abs() * 17.0) as usize) % 4;
    let flower = commands
        .spawn((
            Prop,
            PropSway::default(),
            PropKind::Flower,
            PropTerrainAnchor { terrain_y: y },
            cell,
            Transform::from_xyz(x, y, z),
            Visibility::default(),
        ))
        .id();
    let stem = commands
        .spawn((Mesh3d(assets.flower_stem.clone()), MeshMaterial3d(assets.stem_mat.clone()), Transform::from_xyz(0.0, 0.1, 0.0)))
        .id();
    let head = commands
        .spawn((Mesh3d(assets.flower_head.clone()), MeshMaterial3d(assets.flower_colors[color_idx].clone()), Transform::from_xyz(0.0, 0.22, 0.0)))
        .id();
    commands.entity(flower).add_children(&[stem, head]);
    commands.entity(chunk).add_child(flower);
    flower
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

fn spawn_cactus(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets, cell: PropCell) -> Entity {
    let cactus = commands
        .spawn((
            Prop,
            PropSway::default(),
            PropKind::Cactus,
            PropTerrainAnchor { terrain_y: y },
            cell,
            Mesh3d(assets.cactus_body.clone()),
            MeshMaterial3d(assets.cactus_mat.clone()),
            Transform::from_xyz(x, y + 0.25, z),
        ))
        .id();
    commands.entity(chunk).add_child(cactus);
    cactus
}

fn spawn_dead_bush(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets, cell: PropCell) -> Entity {
    let bush = commands
        .spawn((
            Prop,
            PropSway::default(),
            PropKind::DeadBush,
            PropTerrainAnchor { terrain_y: y },
            cell,
            Mesh3d(assets.dead_bush.clone()),
            MeshMaterial3d(assets.dead_bush_mat.clone()),
            Transform::from_xyz(x, y + 0.05, z),
        ))
        .id();
    commands.entity(chunk).add_child(bush);
    bush
}

fn spawn_ice_rock(commands: &mut Commands, chunk: Entity, x: f32, y: f32, z: f32, assets: &PropAssets, cell: PropCell) -> Entity {
    let prop = commands
        .spawn((
            Prop,
            PropKind::IceRock,
            PropTerrainAnchor { terrain_y: y },
            cell,
            Mesh3d(assets.ice_rock.clone()),
            MeshMaterial3d(assets.ice_rock_mat.clone()),
            Transform::from_xyz(x, y + 0.1, z),
        ))
        .id();
    commands.entity(chunk).add_child(prop);
    prop
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

/// Drain [`Terrain::painted_cells`] each frame: for every cell whose
/// biome was just painted, despawn any existing props in that cell and
/// spawn new ones for the painted biome. Density check + variety hash
/// are deterministic per (wx, wz), so painting Forest onto Desert
/// produces a tree at the same world position the cell would have had
/// if it had been Forest from PCG.
///
/// This runs after [`super::terrain::regenerate_dirty_chunks`] so the
/// chunk's mesh has already picked up the painted vertex colour by the
/// time props rebuild.
pub fn respawn_props_for_painted_cells(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    noise: Res<WorldNoise>,
    mut terrain: ResMut<Terrain>,
    chunk_manager: Res<ChunkManager>,
    assets: Res<PropAssets>,
    cell_props: Query<(Entity, &PropCell)>,
) {
    if terrain.painted_cells.is_empty() {
        return;
    }
    let painted: Vec<(ChunkCoord, HashSet<(u8, u8)>)> =
        terrain.painted_cells.drain().collect();

    for (coord, cells) in painted {
        let Some(&chunk_entity) = chunk_manager.loaded.get(&coord) else {
            continue;
        };
        // Despawn any existing props in painted cells. `commands.despawn()`
        // is recursive in Bevy 0.18, so per-prop trunk/canopy children
        // (and the rapier collider child on rocks) come down with the
        // root entity.
        let to_despawn: Vec<Entity> = cell_props
            .iter()
            .filter(|(_, cell)| {
                cell.cx == coord.0
                    && cell.cz == coord.1
                    && cells.contains(&(cell.lx, cell.lz))
            })
            .map(|(e, _)| e)
            .collect();
        for entity in to_despawn {
            commands.entity(entity).despawn();
        }
        // Spawn fresh props for the painted biome and tag each with
        // `PropSpawnPop` so they animate in instead of appearing flatly.
        for (lx, lz) in cells {
            let world_x = coord.0 * CHUNK_SIZE + lx as i32;
            let world_z = coord.1 * CHUNK_SIZE + lz as i32;
            // Prefer the painted biome (read off the chunk's vertex grid,
            // which already includes biome_edits); fall back to PCG if
            // the chunk somehow isn't loaded by the time we get here.
            let biome = terrain
                .vertex_biome(world_x, world_z)
                .unwrap_or_else(|| noise.sample(world_x as f64, world_z as f64).biome);
            if let Some(prop_entity) = try_spawn_cell_prop(
                &mut commands,
                &asset_server,
                chunk_entity,
                coord.0,
                coord.1,
                lx,
                lz,
                biome,
                &terrain,
                &noise,
                &assets,
            ) {
                commands
                    .entity(prop_entity)
                    .insert(PropSpawnPop::default());
            }
        }
    }
}

/// Animate freshly-painted props from a near-zero scale to their
/// natural scale with an ease-out-back overshoot. The natural scale is
/// captured from the prop's `Transform.scale` on the first tick (snap
/// to a tiny start scale, start the curve), so leaf spawners don't
/// need to know about the animation — they just set their scale as
/// usual and the respawn system tags the entity with `PropSpawnPop`.
///
/// The minimum scale is clamped to [`POP_SCALE_FLOOR`] (~0.1%) instead
/// of zero. Some props (rocks/boulders/mushrooms) have a child rapier
/// cuboid collider that inherits the parent's GlobalTransform scale —
/// at exactly zero the collider's AABB collapses and parry's BVH
/// builder panics with "index out of bounds" mid-physics-step. A 0.001
/// floor is visually indistinguishable from zero at typical prop sizes
/// but keeps the collider non-degenerate.
const POP_SCALE_FLOOR: f32 = 0.001;

pub fn animate_prop_spawn_pop(
    mut commands: Commands,
    mut props: Query<(Entity, &mut Transform, &mut PropSpawnPop)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (entity, mut tf, mut pop) in &mut props {
        let base_scale = match pop.base_scale {
            Some(s) => s,
            None => {
                // First tick: snapshot the spawner-set scale, snap the
                // visible scale to the tiny floor, and skip advancing
                // so the curve starts at the floor next frame.
                let snapped = tf.scale;
                pop.base_scale = Some(snapped);
                tf.scale = snapped * POP_SCALE_FLOOR;
                continue;
            }
        };
        pop.elapsed += dt;
        let t = (pop.elapsed / pop.duration).clamp(0.0, 1.0);
        if t >= 1.0 {
            tf.scale = base_scale;
            commands.entity(entity).remove::<PropSpawnPop>();
        } else {
            // ease-out-back: starts at ~0, overshoots ~10% past 1,
            // settles. Floor at POP_SCALE_FLOOR keeps the collider AABB
            // non-degenerate.
            let f = ease_out_back(t).max(POP_SCALE_FLOOR);
            tf.scale = base_scale * f;
        }
    }
}

fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158_f32;
    let c3 = c1 + 1.0;
    let t1 = t - 1.0;
    1.0 + c3 * t1 * t1 * t1 + c1 * t1 * t1
}

/// Re-anchor props to their cell's current terrain height after a chunk
/// regenerates (W1.10 follow-up: Raise/Lower/Smooth/Flatten brushes).
///
/// Triggers off `Changed<Mesh3d>` on chunk entities — the regen system
/// inserts a fresh `Mesh3d` whenever it rebuilds a chunk, so this query
/// fires exactly when terrain heights may have moved. We compute the
/// delta against `PropTerrainAnchor.terrain_y` (cached at spawn) and
/// shift the prop's Y by that amount, which keeps each prop's
/// per-kind visual lift implicit in the existing `Transform.y`.
/// `PropCollision.top_y` (used by the cat's climb-on-top snap) gets the
/// same delta so it stays glued to the prop's visible top.
pub fn snap_props_to_terrain(
    chunks: Query<(&Children, &Chunk), Changed<Mesh3d>>,
    mut props: Query<(
        &mut Transform,
        &mut PropTerrainAnchor,
        Option<&mut PropCollision>,
    )>,
    terrain: Res<Terrain>,
    noise: Res<WorldNoise>,
) {
    for (children, chunk) in &chunks {
        let world_offset_x = (chunk.x * CHUNK_SIZE) as f32;
        let world_offset_z = (chunk.z * CHUNK_SIZE) as f32;
        for &child in children {
            let Ok((mut tf, mut anchor, collision)) = props.get_mut(child) else {
                continue;
            };
            let world_x = world_offset_x + tf.translation.x;
            let world_z = world_offset_z + tf.translation.z;
            let new_terrain_y = terrain.height_at_or_sample(world_x, world_z, &noise);
            let delta = new_terrain_y - anchor.terrain_y;
            if delta.abs() < 0.001 {
                continue;
            }
            tf.translation.y += delta;
            if let Some(mut col) = collision {
                col.top_y += delta;
            }
            anchor.terrain_y = new_terrain_y;
        }
    }
}
