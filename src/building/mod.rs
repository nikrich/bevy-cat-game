use bevy::prelude::*;

use crate::input::GameInput;
use crate::inventory::{Inventory, InventoryChanged, ItemKind};
use crate::player::Player;
use crate::world::biome::WorldNoise;
use crate::world::chunks::ChunkManager;
use crate::world::terrain::step_height;

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlaceEvent>()
            .add_systems(
                Update,
                (toggle_build_mode, select_build_item, update_preview, place_building),
            );
    }
}

const PLACEABLE_ITEMS: &[ItemKind] = &[
    ItemKind::Fence,
    ItemKind::Bench,
    ItemKind::Lantern,
    ItemKind::FlowerPot,
    ItemKind::Wreath,
];

#[derive(Resource)]
pub struct BuildMode {
    pub selected: usize,
    pub rotation: f32,
    pub preview_entity: Option<Entity>,
}

impl BuildMode {
    pub fn selected_item(&self) -> ItemKind {
        PLACEABLE_ITEMS[self.selected]
    }
}

#[derive(Component)]
pub struct PlacedBuilding {
    pub item: ItemKind,
}

#[derive(Component)]
struct BuildPreview;

#[derive(Event)]
pub struct PlaceEvent {
    pub item: ItemKind,
    pub position: Vec3,
}

fn toggle_build_mode(
    mut commands: Commands,
    input: Res<GameInput>,
    build_mode: Option<Res<BuildMode>>,
    inventory: Res<Inventory>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !input.toggle_build {
        return;
    }

    match build_mode {
        Some(mode) => {
            // Clean up preview
            if let Some(preview) = mode.preview_entity {
                commands.entity(preview).despawn();
            }
            commands.remove_resource::<BuildMode>();
        }
        None => {
            let has_placeables = PLACEABLE_ITEMS
                .iter()
                .any(|item| inventory.count(*item) > 0);
            if has_placeables {
                // Find first available item
                let selected = PLACEABLE_ITEMS
                    .iter()
                    .position(|item| inventory.count(*item) > 0)
                    .unwrap_or(0);

                // Spawn preview entity
                let item = PLACEABLE_ITEMS[selected];
                let (mesh, color, scale) = building_visual(item);
                let preview = commands
                    .spawn((
                        BuildPreview,
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: color.with_alpha(0.5),
                            alpha_mode: AlphaMode::Blend,
                            ..default()
                        })),
                        Transform::from_xyz(0.0, -100.0, 0.0).with_scale(scale),
                    ))
                    .id();

                commands.insert_resource(BuildMode {
                    selected,
                    rotation: 0.0,
                    preview_entity: Some(preview),
                });
            }
        }
    }
}

fn select_build_item(
    mut commands: Commands,
    input: Res<GameInput>,
    mut build_mode: Option<ResMut<BuildMode>>,
    inventory: Res<Inventory>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mode) = &mut build_mode else { return };

    if let Some(slot) = input.build_select {
        let new_idx = match slot {
            99 => (mode.selected + 1) % PLACEABLE_ITEMS.len(), // next (gamepad)
            98 => {
                if mode.selected == 0 {
                    PLACEABLE_ITEMS.len() - 1
                } else {
                    mode.selected - 1
                }
            } // prev (gamepad)
            i if i < PLACEABLE_ITEMS.len() => i,
            _ => return,
        };

        if inventory.count(PLACEABLE_ITEMS[new_idx]) > 0 {
            mode.selected = new_idx;

            // Update preview mesh
            if let Some(preview) = mode.preview_entity {
                commands.entity(preview).despawn();
            }
            let item = PLACEABLE_ITEMS[new_idx];
            let (mesh, color, scale) = building_visual(item);
            let preview = commands
                .spawn((
                    BuildPreview,
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: color.with_alpha(0.5),
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    })),
                    Transform::from_xyz(0.0, -100.0, 0.0).with_scale(scale),
                ))
                .id();
            mode.preview_entity = Some(preview);
        }
    }

    if input.rotate {
        mode.rotation += std::f32::consts::FRAC_PI_2;
    }
}

fn update_preview(
    input: Res<GameInput>,
    build_mode: Option<Res<BuildMode>>,
    mut previews: Query<&mut Transform, With<BuildPreview>>,
    chunk_manager: Res<ChunkManager>,
    player_query: Query<&GlobalTransform, With<Player>>,
) {
    let Some(mode) = &build_mode else { return };
    let Ok(mut preview_tf) = previews.single_mut() else { return };

    let noise = WorldNoise::new(chunk_manager.seed);

    // Use cursor world position (mouse) or position in front of player (gamepad)
    let place_pos = if let Some(cursor) = input.cursor_world {
        cursor
    } else if let Ok(player_gt) = player_query.single() {
        let pos = player_gt.translation();
        let forward = player_gt.forward().as_vec3();
        pos + forward * 1.5
    } else {
        return;
    };

    let grid_x = place_pos.x.round();
    let grid_z = place_pos.z.round();

    let sample = noise.sample(grid_x as f64, grid_z as f64);
    let sh = step_height(sample.elevation * sample.biome.height_scale());
    let place_y = sh * 0.5 + 0.1;

    preview_tf.translation = Vec3::new(grid_x, place_y, grid_z);
    preview_tf.rotation = Quat::from_rotation_y(mode.rotation);
}

fn place_building(
    mut commands: Commands,
    input: Res<GameInput>,
    build_mode: Option<Res<BuildMode>>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: EventWriter<InventoryChanged>,
    mut place_events: EventWriter<PlaceEvent>,
    previews: Query<&Transform, With<BuildPreview>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mode) = &build_mode else { return };

    if !input.place {
        return;
    }

    let item = mode.selected_item();
    if inventory.count(item) == 0 {
        return;
    }

    let Ok(preview_tf) = previews.single() else { return };
    let place_pos = preview_tf.translation;

    // Consume item
    let entry = inventory.items.entry(item).or_insert(0);
    *entry = entry.saturating_sub(1);
    inv_events.write(InventoryChanged {
        item,
        new_count: inventory.count(item),
    });

    place_events.write(PlaceEvent { item, position: place_pos });

    // Spawn the building
    let (mesh, color, scale) = building_visual(item);
    commands.spawn((
        PlacedBuilding { item },
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_translation(place_pos)
            .with_rotation(Quat::from_rotation_y(mode.rotation))
            .with_scale(scale),
    ));

    // Exit build mode if no more of this item
    if inventory.count(item) == 0 {
        if let Some(preview) = mode.preview_entity {
            commands.entity(preview).despawn();
        }
        commands.remove_resource::<BuildMode>();
    }
}

fn building_visual(item: ItemKind) -> (Mesh, Color, Vec3) {
    match item {
        ItemKind::Fence => (
            Mesh::from(Cuboid::new(1.0, 0.6, 0.08)),
            Color::srgb(0.60, 0.45, 0.28),
            Vec3::ONE,
        ),
        ItemKind::Bench => (
            Mesh::from(Cuboid::new(1.0, 0.35, 0.4)),
            Color::srgb(0.50, 0.35, 0.20),
            Vec3::ONE,
        ),
        ItemKind::Lantern => (
            Mesh::from(Cylinder::new(0.1, 0.5)),
            Color::srgb(0.90, 0.80, 0.40),
            Vec3::ONE,
        ),
        ItemKind::FlowerPot => (
            Mesh::from(Cylinder::new(0.15, 0.25)),
            Color::srgb(0.72, 0.45, 0.35),
            Vec3::ONE,
        ),
        ItemKind::Wreath => (
            Mesh::from(Torus::new(0.05, 0.2)),
            Color::srgb(0.40, 0.65, 0.35),
            Vec3::ONE,
        ),
        _ => (
            Mesh::from(Cuboid::new(0.3, 0.3, 0.3)),
            Color::srgb(0.5, 0.5, 0.5),
            Vec3::ONE,
        ),
    }
}

pub fn building_visual_pub(item: ItemKind) -> (Mesh, Color, Vec3) {
    building_visual(item)
}
