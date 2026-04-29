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
            .init_resource::<PickupHold>()
            .add_systems(
                Startup,
                init_placeable_items.after(crate::items::registry::seed_default_items),
            )
            .add_systems(
                Update,
                (
                    toggle_build_mode,
                    cancel_placing,
                    select_build_item,
                    update_preview,
                    place_building,
                    pickup_held_building,
                ),
            );
    }
}

/// Cached list of placeable item IDs (anything tagged PLACEABLE in the registry),
/// in stable registration order. Used by the build hotbar and BuildMode.selected.
#[derive(Resource, Default)]
pub struct PlaceableItems(pub Vec<ItemId>);

/// Tracks a left-mouse hold on a placed building so we can refund-pickup it
/// after the player has held the button for `PICKUP_HOLD_SECS`.
#[derive(Resource, Default)]
pub struct PickupHold {
    pub target: Option<Entity>,
    pub started_at: f32,
}

const PICKUP_HOLD_SECS: f32 = 0.5;
const PICKUP_RADIUS: f32 = 0.55;

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
    asset_server: Res<AssetServer>,
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
                        &asset_server,
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
/// Uses the same Kenney glTF the placed building will use, so the ghost
/// matches what you're about to plant. Scale comes from `Form::placement_scale`.
pub fn refresh_build_preview(
    commands: &mut Commands,
    mode: &mut BuildMode,
    item: ItemId,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    if let Some(preview) = mode.preview_entity.take() {
        commands.entity(preview).despawn();
    }
    let Some(def) = registry.get(item) else { return };
    let scale = def.form.placement_scale();
    let xform = Transform::from_xyz(0.0, -100.0, 0.0).with_scale(Vec3::splat(scale));

    let preview = if let Some(path) = def.form.scene_path() {
        commands
            .spawn((BuildPreview, SceneRoot(asset_server.load(path)), xform))
            .id()
    } else {
        let mesh = def.form.make_mesh();
        let color = def.material.base_color();
        commands
            .spawn((
                BuildPreview,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: color.with_alpha(0.5),
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })),
                xform,
            ))
            .id()
    };
    mode.preview_entity = Some(preview);
}

fn select_build_item(
    mut commands: Commands,
    input: Res<GameInput>,
    mut build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    inventory: Res<Inventory>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
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
                    &asset_server,
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
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    placed_q: Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    mut previews: Query<&mut Transform, With<BuildPreview>>,
    chunk_manager: Res<ChunkManager>,
    player_query: Query<&GlobalTransform, With<Player>>,
) {
    let Some(mode) = &build_mode else { return };
    let Ok(mut preview_tf) = previews.single_mut() else { return };
    let Some(item) = placeables.0.get(mode.selected).copied() else { return };
    let Some(def) = registry.get(item) else { return };
    let form = def.form;

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

    // Snap to either tile centres or half-grid (edges) depending on the form.
    let (grid_x, grid_z) = match form.snap_mode() {
        crate::items::SnapMode::Cell => (place_pos.x.round(), place_pos.z.round()),
        crate::items::SnapMode::Edge => (
            (place_pos.x * 2.0).round() / 2.0,
            (place_pos.z * 2.0).round() / 2.0,
        ),
    };

    let sample = noise.sample(grid_x as f64, grid_z as f64);
    let sh = step_height(sample.elevation * sample.biome.height_scale());
    // The terrain tile is a 1.0 x 0.6 x 1.0 cuboid centred on `sh * 0.5`, so its
    // top sits at `sh * 0.5 + 0.3`.
    let tile_top = sh * 0.5 + 0.3;

    // Stack on top of any existing buildings at this grid cell so walls can
    // sit on floors, roofs on walls, etc. We take the highest top in the cell.
    let mut base = tile_top;
    for (tf, building) in &placed_q {
        let dx = (tf.translation.x - grid_x).abs();
        let dz = (tf.translation.z - grid_z).abs();
        if dx < 0.5 && dz < 0.5 {
            if let Some(existing) = registry.get(building.item) {
                let bottom = tf.translation.y - existing.form.placement_lift();
                let top = bottom + existing.form.placement_height();
                if top > base {
                    base = top;
                }
            }
        }
    }

    let place_y = base + form.placement_lift();
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
    placed_q: Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mode) = &build_mode else { return };
    if !input.place {
        return;
    }

    // Suppress placement when the cursor is hovering an existing building --
    // that click is reserved for the hold-to-pickup flow.
    if let Some(cursor) = input.cursor_world {
        for (tf, _) in &placed_q {
            let dx = tf.translation.x - cursor.x;
            let dz = tf.translation.z - cursor.z;
            if (dx * dx + dz * dz).sqrt() <= PICKUP_RADIUS {
                return;
            }
        }
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

/// Right-click or Esc clears placing mode. Used so the cursor goes back to
/// "gather / pick up" mode without forcing the player to use B.
fn cancel_placing(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    build_mode: Option<Res<BuildMode>>,
) {
    let cancel = keyboard.just_pressed(KeyCode::Escape)
        || mouse.just_pressed(MouseButton::Right);
    if !cancel {
        return;
    }
    let Some(mode) = build_mode else { return };
    if let Some(preview) = mode.preview_entity {
        commands.entity(preview).despawn();
    }
    commands.remove_resource::<BuildMode>();
}

/// Public helper: enter placing mode (or switch the current selection) for
/// `item`. Called from inventory / hotbar slot clicks.
pub fn enter_placing_with(
    commands: &mut Commands,
    build_mode: Option<&mut BuildMode>,
    placeables: &PlaceableItems,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    item: ItemId,
) {
    let Some(idx) = placeables.0.iter().position(|i| *i == item) else { return };
    if let Some(mode) = build_mode {
        if mode.selected != idx {
            mode.selected = idx;
            refresh_build_preview(commands, mode, item, registry, asset_server, meshes, materials);
        }
    } else {
        let mut mode = BuildMode { selected: idx, rotation: 0.0, preview_entity: None };
        refresh_build_preview(commands, &mut mode, item, registry, asset_server, meshes, materials);
        commands.insert_resource(mode);
    }
}

/// Hold left-mouse on a placed building for `PICKUP_HOLD_SECS` and the
/// building disappears, refunding one of its item back to the inventory.
/// Works regardless of placing mode -- the placement system separately checks
/// for "cursor over existing building" and suppresses placement in that case.
fn pickup_held_building(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    input: Res<crate::input::GameInput>,
    mut commands: Commands,
    mut inventory: ResMut<Inventory>,
    mut inv_events: EventWriter<InventoryChanged>,
    mut hold: ResMut<PickupHold>,
    placed: Query<(Entity, &Transform, &PlacedBuilding)>,
) {
    if input.pointer_over_ui {
        hold.target = None;
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        hold.target = None;
        return;
    }
    let Some(cursor) = input.cursor_world else {
        hold.target = None;
        return;
    };

    // Find the closest building with the cursor inside its pickup radius.
    let mut closest: Option<(Entity, ItemId, f32)> = None;
    for (entity, tf, building) in &placed {
        let dx = tf.translation.x - cursor.x;
        let dz = tf.translation.z - cursor.z;
        let d = (dx * dx + dz * dz).sqrt();
        if d <= PICKUP_RADIUS && closest.map(|c| d < c.2).unwrap_or(true) {
            closest = Some((entity, building.item, d));
        }
    }

    let now = time.elapsed_secs();
    let Some((entity, item, _)) = closest else {
        hold.target = None;
        return;
    };

    // First frame on this target -> start the timer.
    if hold.target != Some(entity) {
        hold.target = Some(entity);
        hold.started_at = now;
        return;
    }

    if now - hold.started_at >= PICKUP_HOLD_SECS {
        commands.entity(entity).despawn();
        inventory.add(item, 1);
        inv_events.write(InventoryChanged { item, new_count: inventory.count(item) });
        hold.target = None;
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
    let scale = def.form.placement_scale();
    let scaled = transform.with_scale(transform.scale * Vec3::splat(scale));

    if let Some(path) = def.form.scene_path() {
        commands.spawn((
            PlacedBuilding { item },
            SceneRoot(asset_server.load(path)),
            scaled,
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
        scaled,
    ));
}
