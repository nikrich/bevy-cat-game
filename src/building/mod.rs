use bevy::prelude::*;

use crate::input::GameInput;
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{ItemId, ItemRegistry, ItemTags};
use crate::player::Player;
use crate::world::biome::WorldNoise;
use crate::world::chunks::ChunkManager;
use crate::world::terrain::step_height;

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlaceEvent>()
            .init_resource::<PlaceableItems>()
            .add_systems(
                Startup,
                init_placeable_items.after(crate::items::registry::seed_default_items),
            )
            .add_systems(
                Update,
                (
                    toggle_build_mode,
                    select_build_item,
                    update_preview,
                    place_building,
                ),
            );
    }
}

/// Cached list of placeable item IDs (anything tagged PLACEABLE in the registry),
/// in stable registration order. Used by the build hotbar and BuildMode.selected.
#[derive(Resource, Default)]
pub struct PlaceableItems(pub Vec<ItemId>);

fn init_placeable_items(
    registry: Res<ItemRegistry>,
    mut placeables: ResMut<PlaceableItems>,
) {
    placeables.0 = registry
        .iter_with_tag(ItemTags::PLACEABLE)
        .map(|d| d.id)
        .collect();
}

#[derive(Resource)]
pub struct BuildMode {
    pub selected: usize,
    pub rotation: f32,
    pub preview_entity: Option<Entity>,
}

impl BuildMode {
    pub fn selected_item(&self, placeables: &PlaceableItems) -> Option<ItemId> {
        placeables.0.get(self.selected).copied()
    }
}

#[derive(Component)]
pub struct PlacedBuilding {
    pub item: ItemId,
}

#[derive(Component)]
struct BuildPreview;

#[derive(Event)]
pub struct PlaceEvent {
    pub item: ItemId,
    pub position: Vec3,
}

fn toggle_build_mode(
    mut commands: Commands,
    input: Res<GameInput>,
    build_mode: Option<Res<BuildMode>>,
    inventory: Res<Inventory>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !input.toggle_build {
        return;
    }

    match build_mode {
        Some(mode) => {
            if let Some(preview) = mode.preview_entity {
                commands.entity(preview).despawn();
            }
            commands.remove_resource::<BuildMode>();
        }
        None => {
            let selected = placeables
                .0
                .iter()
                .position(|id| inventory.count(*id) > 0)
                .unwrap_or(0);
            let mut mode = BuildMode {
                selected,
                rotation: 0.0,
                preview_entity: None,
            };
            if let Some(item) = placeables.0.get(selected).copied() {
                if inventory.count(item) > 0 {
                    refresh_build_preview(
                        &mut commands,
                        &mut mode,
                        item,
                        &registry,
                        &mut meshes,
                        &mut materials,
                    );
                }
            }
            commands.insert_resource(mode);
        }
    }
}

/// Despawn the current build preview (if any) and spawn a fresh one for `item`.
pub fn refresh_build_preview(
    commands: &mut Commands,
    mode: &mut BuildMode,
    item: ItemId,
    registry: &ItemRegistry,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    if let Some(preview) = mode.preview_entity.take() {
        commands.entity(preview).despawn();
    }
    let Some(def) = registry.get(item) else { return };
    let mesh = def.form.make_mesh();
    let color = def.material.base_color();
    let preview = commands
        .spawn((
            BuildPreview,
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: color.with_alpha(0.5),
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_xyz(0.0, -100.0, 0.0),
        ))
        .id();
    mode.preview_entity = Some(preview);
}

fn select_build_item(
    mut commands: Commands,
    input: Res<GameInput>,
    mut build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    inventory: Res<Inventory>,
    registry: Res<ItemRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mode) = &mut build_mode else { return };
    let n = placeables.0.len();
    if n == 0 {
        return;
    }

    if let Some(slot) = input.build_select {
        let new_idx = match slot {
            99 => (mode.selected + 1) % n,            // next (gamepad)
            98 => (mode.selected + n - 1) % n,        // prev (gamepad)
            i if i < n => i,
            _ => return,
        };
        if let Some(item) = placeables.0.get(new_idx).copied() {
            if inventory.count(item) > 0 && new_idx != mode.selected {
                mode.selected = new_idx;
                refresh_build_preview(
                    &mut commands,
                    mode,
                    item,
                    &registry,
                    &mut meshes,
                    &mut materials,
                );
            }
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
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
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

    let Some(item) = placeables.0.get(mode.selected).copied() else { return };
    if inventory.count(item) == 0 {
        return;
    }

    let Ok(preview_tf) = previews.single() else { return };
    let place_pos = preview_tf.translation;

    let entry = inventory.items.entry(item).or_insert(0);
    *entry = entry.saturating_sub(1);
    inv_events.write(InventoryChanged {
        item,
        new_count: inventory.count(item),
    });

    place_events.write(PlaceEvent {
        item,
        position: place_pos,
    });

    spawn_placed_building(
        &mut commands,
        &registry,
        &asset_server,
        &mut meshes,
        &mut materials,
        item,
        Transform::from_translation(place_pos).with_rotation(Quat::from_rotation_y(mode.rotation)),
    );

    if inventory.count(item) == 0 {
        if let Some(preview) = mode.preview_entity {
            commands.entity(preview).despawn();
        }
        commands.remove_resource::<BuildMode>();
    }
}

/// Spawn a placed building from a known transform. Used by `place_building`
/// and by save/load. If the item's `Form` has a glTF `scene_path`, spawn a
/// `SceneRoot` of the Kenney model; otherwise fall back to the procedural
/// primitive mesh tinted by `Material::base_color`.
pub fn spawn_placed_building(
    commands: &mut Commands,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    item: ItemId,
    transform: Transform,
) {
    let Some(def) = registry.get(item) else { return };

    if let Some(path) = def.form.scene_path() {
        commands.spawn((
            PlacedBuilding { item },
            SceneRoot(asset_server.load(path)),
            transform,
        ));
        return;
    }

    let mesh = def.form.make_mesh();
    let color = def.material.base_color();
    commands.spawn((
        PlacedBuilding { item },
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.8,
            ..default()
        })),
        transform,
    ));
}
