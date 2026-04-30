use bevy::prelude::*;

pub mod collision;

use leafwing_input_manager::prelude::ActionState;

use crate::input::{Action, CursorHit, CursorState};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{Form, ItemId, ItemRegistry, ItemTags, PlacementStyle, SnapMode};
use crate::player::Player;
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaceableItems>()
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
        collision::register(app);
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

/// Dev cheat: when true, placement does not consume inventory and count
/// checks short-circuit to "always have stock". Lets us focus on the build
/// tool without the meta-loop of crafting/refilling. Flip to false (or wire
/// to a `Cheats` resource) when shipping to players.
const INFINITE_RESOURCES: bool = true;

/// Wall length in world units. The line tool stamps walls centred at
/// `anchor + (i + 0.5) * WALL_LENGTH * axis_dir` so each wall fills exactly
/// one 1 m cell along the dominant axis.
const WALL_LENGTH: f32 = 1.0;

/// XZ distance under which a planned wall position is considered already
/// covered by an existing placed piece. Skips that cell from both ghost and
/// placement so re-running a line over an existing wall — including the
/// shared corner cell of two perpendicular line segments — doesn't overlap.
const OCCUPIED_RADIUS: f32 = 0.4;
const OCCUPIED_Y: f32 = 0.6;

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
    /// Single-piece preview entity (used for non-wall forms).
    pub preview_entity: Option<Entity>,
    /// Line tool first-click anchor. `Some` while a wall line is in progress;
    /// `None` outside the line tool.
    pub line_anchor: Option<Vec3>,
    /// Ghost entities for the in-progress wall segment. Pooled across frames:
    /// transforms and material colours update each frame, count syncs to the
    /// segment length.
    pub line_ghosts: Vec<Entity>,
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

/// Resolve the ghost chain's direction and length from cursor delta to
/// anchor. Returns `(along_x, dir_sign, n)` where `n` is the number of
/// cells covered **inclusively** from anchor to cursor (so anchor + cursor
/// are both wall positions). Always at least one cell — when the cursor
/// sits on the anchor, defaults to +X so the player can see what the next
/// click will place. Shared by `wall_segment_transforms` and `segment_end`
/// so preview and placement agree on geometry.
fn resolve_chain(anchor: Vec3, cursor: Vec3) -> (bool, f32, usize) {
    let dx = cursor.x - anchor.x;
    let dz = cursor.z - anchor.z;
    let cursor_moved = dx.abs() > 0.05 || dz.abs() > 0.05;
    let along_x = if cursor_moved { dx.abs() >= dz.abs() } else { true };
    let segment_length = if along_x { dx.abs() } else { dz.abs() };
    // +1 so n includes both endpoint cells (anchor + cursor cell). For
    // cursor at anchor, n = 1.
    let n = (segment_length / WALL_LENGTH).round() as usize + 1;
    let raw_sign = if along_x { dx } else { dz };
    let dir_sign = if !cursor_moved || raw_sign >= 0.0 {
        1.0
    } else {
        -1.0
    };
    (along_x, dir_sign, n)
}

/// Line-tool first-click anchor. Runs `compute_placement` for `Form::Wall`
/// — the anchor is literally the position the first wall would occupy.
/// All subsequent walls in the chain mirror that center-Y.
fn anchor_from_hit(
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    registry: &ItemRegistry,
    terrain: &Terrain,
    noise: &WorldNoise,
) -> Vec3 {
    compute_placement(
        cursor_world,
        cursor_hit,
        Form::Wall,
        registry,
        placed_q,
        terrain,
        noise,
    )
}

/// Compute the wall transforms for a line from `anchor` to `cursor`.
///
/// All walls in the segment share `anchor.y` (the wall *center* Y from
/// `compute_placement` — already includes `placement_lift`). The chain
/// stays flat at the anchor's height; perpendicular existing walls in the
/// path are intersected at the same Y instead of being climbed over.
/// Vertical stacking happens at the next first-click via `anchor_from_hit`.
fn wall_segment_transforms(anchor: Vec3, cursor: Vec3) -> Vec<Transform> {
    let (along_x, dir_sign, n) = resolve_chain(anchor, cursor);
    let yaw = if along_x { 0.0 } else { std::f32::consts::FRAC_PI_2 };
    let y = anchor.y;

    (0..n)
        .map(|i| {
            let cell_offset = i as f32 * WALL_LENGTH * dir_sign;
            let pos_x = if along_x { anchor.x + cell_offset } else { anchor.x };
            let pos_z = if along_x { anchor.z } else { anchor.z + cell_offset };
            Transform::from_xyz(pos_x, y, pos_z).with_rotation(Quat::from_rotation_y(yaw))
        })
        .collect()
}

/// Continuous-mode anchor advance. After a segment is confirmed, the anchor
/// jumps to the **last placed cube** (anchor + (n-1) cells). The next
/// chain's `wall_segment_transforms` will include this cube as its first
/// position, but `is_position_occupied` skips it silently — so the new
/// chain extends from the cube's adjacent face in whichever direction the
/// cursor moves (straight ahead → row continues, perpendicular → L-bend
/// sharing the corner cube).
fn segment_end(anchor: Vec3, cursor: Vec3) -> Vec3 {
    let (along_x, dir_sign, n) = resolve_chain(anchor, cursor);
    let span = (n as f32 - 1.0) * WALL_LENGTH * dir_sign;
    if along_x {
        Vec3::new(anchor.x + span, anchor.y, anchor.z)
    } else {
        Vec3::new(anchor.x, anchor.y, anchor.z + span)
    }
}

/// Minecraft-style cube placement. The rapier raycast hit + surface normal
/// determine which adjacent cell the new piece occupies:
///
/// - **Hit terrain** → cell at the hit point's snapped XZ on terrain.
/// - **Hit a placed piece, normal pointing up** → cube directly above
///   (new piece's bottom face = hit piece's top face).
/// - **Hit a placed piece, normal pointing sideways** → cube one cell over
///   in the normal's direction at the same height (new piece's bottom =
///   hit piece's bottom).
///
/// Returns the new piece's **center** Y (what spawn_placed_building wants).
fn compute_placement(
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    form: Form,
    registry: &ItemRegistry,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    terrain: &Terrain,
    noise: &WorldNoise,
) -> Vec3 {
    let new_lift = form.placement_lift();

    if let Some(hit) = cursor_hit {
        if let Ok((tf, building)) = placed_q.get(hit.entity) {
            if let Some(hit_def) = registry.get(building.item) {
                let hit_lift = hit_def.form.placement_lift();
                let hit_top = tf.translation.y + hit_lift;
                let hit_bottom = tf.translation.y - hit_lift;

                if hit.normal.y > 0.7 {
                    // Top face — cube above (stacked).
                    return Vec3::new(tf.translation.x, hit_top + new_lift, tf.translation.z);
                }
                if hit.normal.y.abs() < 0.3 {
                    // Side face — cube adjacent in the normal's direction.
                    // .round() snaps the normal step to a unit cell offset.
                    let step =
                        Vec3::new(hit.normal.x.round(), 0.0, hit.normal.z.round());
                    return Vec3::new(
                        tf.translation.x + step.x,
                        hit_bottom + new_lift,
                        tf.translation.z + step.z,
                    );
                }
                // Slanted face (e.g. roof eave) — fall through to terrain.
            }
        }
        // Hit terrain or a non-PlacedBuilding entity — snap hit XZ to cell.
        let cx = hit.point.x.round();
        let cz = hit.point.z.round();
        let ty = terrain.height_at_or_sample(cx, cz, noise);
        return Vec3::new(cx, ty + new_lift, cz);
    }

    // No raycast hit — fall back to cursor's ground projection.
    let cx = cursor_world.x.round();
    let cz = cursor_world.z.round();
    let ty = terrain.height_at_or_sample(cx, cz, noise);
    Vec3::new(cx, ty + new_lift, cz)
}

/// Whether `pos` is already covered by a placed piece. Same XZ box +
/// generous Y window so wall-on-floor stacking isn't flagged as overlap.
fn is_position_occupied(
    pos: Vec3,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
) -> bool {
    placed_q.iter().any(|(tf, _)| {
        (tf.translation.x - pos.x).abs() < OCCUPIED_RADIUS
            && (tf.translation.z - pos.z).abs() < OCCUPIED_RADIUS
            && (tf.translation.y - pos.y).abs() < OCCUPIED_Y
    })
}

fn toggle_build_mode(
    mut commands: Commands,
    action_state: Res<ActionState<Action>>,
    build_mode: Option<Res<BuildMode>>,
    inventory: Res<Inventory>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !action_state.just_pressed(&Action::ToggleBuild) {
        return;
    }

    match build_mode {
        Some(mode) => {
            if let Some(preview) = mode.preview_entity {
                commands.entity(preview).despawn();
            }
            for ghost in &mode.line_ghosts {
                commands.entity(*ghost).despawn();
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
                line_anchor: None,
                line_ghosts: Vec::new(),
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
/// Also clears any in-progress line tool state so switching pieces never
/// leaves orphaned ghosts.
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
    for ghost in mode.line_ghosts.drain(..) {
        commands.entity(ghost).despawn();
    }
    mode.line_anchor = None;

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
    action_state: Res<ActionState<Action>>,
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

    let slot_actions = [
        Action::Hotbar1,
        Action::Hotbar2,
        Action::Hotbar3,
        Action::Hotbar4,
        Action::Hotbar5,
        Action::Hotbar6,
        Action::Hotbar7,
        Action::Hotbar8,
        Action::Hotbar9,
    ];
    let mut new_idx: Option<usize> = None;
    for (i, action) in slot_actions.iter().enumerate() {
        if action_state.just_pressed(action) && i < n {
            new_idx = Some(i);
            break;
        }
    }
    if action_state.just_pressed(&Action::HotbarNext) {
        new_idx = Some((mode.selected + 1) % n);
    } else if action_state.just_pressed(&Action::HotbarPrev) {
        new_idx = Some((mode.selected + n - 1) % n);
    }

    if let Some(idx) = new_idx {
        if let Some(item) = placeables.0.get(idx).copied() {
            if inventory.count(item) > 0 && idx != mode.selected {
                mode.selected = idx;
                if let Some(def) = registry.get(item) {
                    info!("[build] selected {} ({:?})", def.display_name, def.form);
                }
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

    if action_state.just_pressed(&Action::RotatePiece) {
        mode.rotation += std::f32::consts::FRAC_PI_2;
    }
}

const GHOST_VALID: Color = Color::srgba(0.45, 1.0, 0.55, 0.55);
const GHOST_INVALID: Color = Color::srgba(1.0, 0.35, 0.35, 0.55);
/// Far below the world — used to "hide" pooled ghost entities we don't need
/// this frame without despawning them.
const HIDE_Y: f32 = -100.0;

fn update_preview(
    mut commands: Commands,
    cursor: Res<CursorState>,
    mut build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    inventory: Res<Inventory>,
    placed_q: Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    mut previews_q: Query<
        (&mut Transform, &MeshMaterial3d<StandardMaterial>),
        With<BuildPreview>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    noise: Res<WorldNoise>,
    terrain: Res<Terrain>,
    player_query: Query<&GlobalTransform, With<Player>>,
) {
    let Some(mut mode) = build_mode else { return };
    let Some(item) = placeables.0.get(mode.selected).copied() else { return };
    let Some(def) = registry.get(item) else { return };
    let form = def.form;

    let cursor_world = if let Some(c) = cursor.cursor_world {
        c
    } else if let Ok(player_gt) = player_query.single() {
        let pos = player_gt.translation();
        let forward = player_gt.forward().as_vec3();
        pos + forward * 1.5
    } else {
        return;
    };

    if form.placement_style() == PlacementStyle::Line && mode.line_anchor.is_some() {
        update_line_preview(
            &mut commands,
            &mut mode,
            item,
            cursor_world,
            &inventory,
            &placed_q,
            &mut previews_q,
            &mut materials,
            &mut meshes,
        );
    } else {
        // Outside the line tool — make sure no orphan line ghosts linger.
        for ghost in mode.line_ghosts.drain(..) {
            commands.entity(ghost).despawn();
        }
        update_single_preview(
            &mode,
            form,
            cursor_world,
            cursor.cursor_hit,
            &registry,
            &placed_q,
            &mut previews_q,
            &mut materials,
            &terrain,
            &noise,
        );
    }
}

fn update_single_preview(
    mode: &BuildMode,
    form: Form,
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    registry: &ItemRegistry,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    previews_q: &mut Query<
        (&mut Transform, &MeshMaterial3d<StandardMaterial>),
        With<BuildPreview>,
    >,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
) {
    let Some(preview_entity) = mode.preview_entity else { return };
    let Ok((mut preview_tf, preview_mat)) = previews_q.get_mut(preview_entity) else { return };

    // Same auto-stacking rule for every form including walls — single
    // ghost shows at the column top of the cursor's cell. The line tool's
    // anchor selection uses the same logic (`anchor_from_hit`), so what
    // the player sees here matches where the first wall lands after click.
    let final_pos =
        compute_placement(cursor_world, cursor_hit, form, registry, placed_q, terrain, noise);

    preview_tf.translation = final_pos;
    preview_tf.rotation = Quat::from_rotation_y(mode.rotation);

    // Single-placement is always valid — `place_y` already stacks above any
    // existing piece in the cell, so the ghost never overlaps geometry.
    if let Some(mat) = materials.get_mut(&preview_mat.0) {
        mat.base_color = GHOST_VALID;
    }
}

#[allow(clippy::too_many_arguments)]
fn update_line_preview(
    commands: &mut Commands,
    mode: &mut BuildMode,
    item: ItemId,
    cursor_world: Vec3,
    inventory: &Inventory,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    previews_q: &mut Query<
        (&mut Transform, &MeshMaterial3d<StandardMaterial>),
        With<BuildPreview>,
    >,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    meshes: &mut ResMut<Assets<Mesh>>,
) {
    let Some(anchor) = mode.line_anchor else { return };

    // Hide the single-piece preview entity while the line tool is active —
    // the chain ghosts (always at least one — the cell where the next click
    // will land) serve as the anchor / direction visualization themselves.
    if let Some(preview) = mode.preview_entity {
        if let Ok((mut tf, _)) = previews_q.get_mut(preview) {
            if tf.translation.y > HIDE_Y * 0.5 {
                tf.translation.y = HIDE_Y;
            }
        }
    }

    let segment = wall_segment_transforms(anchor, cursor_world);

    // Pool ghost entities — spawn missing, despawn excess.
    while mode.line_ghosts.len() > segment.len() {
        if let Some(e) = mode.line_ghosts.pop() {
            commands.entity(e).despawn();
        }
    }
    while mode.line_ghosts.len() < segment.len() {
        let mesh = Form::Wall.make_mesh();
        let entity = commands
            .spawn((
                BuildPreview,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: GHOST_VALID,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })),
                Transform::from_xyz(0.0, HIDE_Y, 0.0),
            ))
            .id();
        mode.line_ghosts.push(entity);
    }

    let available = if INFINITE_RESOURCES {
        usize::MAX
    } else {
        inventory.count(item) as usize
    };
    let mut placeable_so_far = 0usize;
    for (i, tx) in segment.iter().enumerate() {
        let entity = mode.line_ghosts[i];
        let occupied = is_position_occupied(tx.translation, placed_q);

        let tint = if occupied {
            // Hide overlapping ghosts entirely — silent skip.
            None
        } else if placeable_so_far < available {
            placeable_so_far += 1;
            Some(GHOST_VALID)
        } else {
            Some(GHOST_INVALID)
        };

        if let Ok((mut tf, mat_handle)) = previews_q.get_mut(entity) {
            *tf = match tint {
                Some(_) => *tx,
                None => Transform::from_xyz(0.0, HIDE_Y, 0.0),
            };
            if let Some(c) = tint {
                if let Some(mat) = materials.get_mut(&mat_handle.0) {
                    mat.base_color = c;
                }
            }
        }
    }
}

fn place_building(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    action_state: Res<ActionState<Action>>,
    cursor: Res<CursorState>,
    crafting: Res<crate::crafting::CraftingState>,
    build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    placed_q: Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    noise: Res<WorldNoise>,
    terrain: Res<Terrain>,
) {
    let Some(mut mode) = build_mode else { return };
    if cursor.pointer_over_ui || crafting.open {
        return;
    }

    // Single-shot click only — no held-mouse stamping. Space (and gamepad
    // South via Action::Place) is treated identically to a left click.
    let click = mouse.just_pressed(MouseButton::Left)
        || action_state.just_pressed(&Action::Place);
    if !click {
        return;
    }

    let Some(item) = placeables.0.get(mode.selected).copied() else { return };
    let Some(def) = registry.get(item) else { return };
    let form = def.form;

    let Some(cursor_world) = cursor.cursor_world else { return };

    // Note: pickup is hold-LMB-for-`PICKUP_HOLD_SECS`, so brief clicks fall
    // through to placement even when the cursor sits over a placed piece.
    // That's intentional — it lets the player drop a window onto a wall, a
    // lantern onto a table, etc. Holding the click on a piece for half a
    // second still triggers pickup (handled in `pickup_held_building`).

    if form.placement_style() == PlacementStyle::Line {
        place_wall_line(
            &mut commands,
            &mut mode,
            item,
            cursor_world,
            cursor.cursor_hit,
            &registry,
            &asset_server,
            &mut inventory,
            &mut inv_events,
            &placed_q,
            &mut meshes,
            &mut materials,
            &terrain,
            &noise,
        );
    } else {
        place_single(
            &mut commands,
            &mut mode,
            item,
            form,
            cursor_world,
            cursor.cursor_hit,
            &registry,
            &asset_server,
            &mut inventory,
            &mut inv_events,
            &placed_q,
            &mut meshes,
            &mut materials,
            &terrain,
            &noise,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn place_wall_line(
    commands: &mut Commands,
    mode: &mut BuildMode,
    item: ItemId,
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
) {
    match mode.line_anchor {
        None => {
            // First click — set anchor. If the raycast hit a placed wall's
            // top face, the chain stacks at that height; otherwise it sits
            // on terrain. anchor.y carries the build base for every wall in
            // every segment of this chain (until cancelled).
            let anchor = anchor_from_hit(cursor_world, cursor_hit, placed_q, registry, terrain, noise);
            mode.line_anchor = Some(anchor);
        }
        Some(anchor) => {
            // Second click — confirm. Place placeable walls along the segment;
            // skip cells already occupied (silent corner overlap handling).
            // Segment always has at least 1 wall (default direction +X when
            // cursor sits on anchor) so a confirm-click always progresses.
            let segment = wall_segment_transforms(anchor, cursor_world);

            let mut placed_count = 0usize;
            for tx in &segment {
                if !INFINITE_RESOURCES && inventory.count(item) == 0 {
                    break;
                }
                if is_position_occupied(tx.translation, placed_q) {
                    continue;
                }
                spawn_placed_building(
                    commands,
                    registry,
                    asset_server,
                    meshes,
                    materials,
                    item,
                    *tx,
                );
                if !INFINITE_RESOURCES {
                    let entry = inventory.items.entry(item).or_insert(0);
                    *entry = entry.saturating_sub(1);
                }
                placed_count += 1;
            }
            if placed_count > 0 && !INFINITE_RESOURCES {
                inv_events.write(InventoryChanged {
                    item,
                    new_count: inventory.count(item),
                });
            }

            // Clear ghosts; update_preview will respawn the next set on the
            // following frame from the advanced anchor.
            for ghost in mode.line_ghosts.drain(..) {
                commands.entity(ghost).despawn();
            }

            // Continuous mode: anchor jumps to the segment's far edge so the
            // next click extends the chain. If the player aims along the
            // perpendicular axis, the next segment auto-rotates 90°.
            mode.line_anchor = Some(segment_end(anchor, cursor_world));

            // Out of inventory — exit build mode entirely.
            if !INFINITE_RESOURCES && inventory.count(item) == 0 {
                if let Some(preview) = mode.preview_entity {
                    commands.entity(preview).despawn();
                }
                mode.line_anchor = None;
                commands.remove_resource::<BuildMode>();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn place_single(
    commands: &mut Commands,
    mode: &mut BuildMode,
    item: ItemId,
    form: Form,
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
) {
    if !INFINITE_RESOURCES && inventory.count(item) == 0 {
        return;
    }

    // Raycast-driven placement: shared with `update_single_preview` so the
    // ghost and the click land at the exact same position. Stacks on top
    // of placed pieces when the cursor visually hits their top face;
    // places adjacent on a side-face hit; falls back to terrain otherwise.
    let pos = compute_placement(cursor_world, cursor_hit, form, registry, placed_q, terrain, noise);
    let transform =
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(mode.rotation));

    spawn_placed_building(
        commands, registry, asset_server, meshes, materials, item, transform,
    );
    if !INFINITE_RESOURCES {
        let entry = inventory.items.entry(item).or_insert(0);
        *entry = entry.saturating_sub(1);
        inv_events.write(InventoryChanged {
            item,
            new_count: inventory.count(item),
        });
        if inventory.count(item) == 0 {
            if let Some(preview) = mode.preview_entity {
                commands.entity(preview).despawn();
            }
            for ghost in mode.line_ghosts.drain(..) {
                commands.entity(ghost).despawn();
            }
            commands.remove_resource::<BuildMode>();
        }
    }
}

/// Right-click or Esc cancels in two tiers: first clears an in-progress
/// line tool anchor (so the player can re-aim a chain), then a second press
/// exits build mode entirely.
fn cancel_placing(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    build_mode: Option<ResMut<BuildMode>>,
) {
    let cancel = keyboard.just_pressed(KeyCode::Escape)
        || mouse.just_pressed(MouseButton::Right);
    if !cancel {
        return;
    }
    let Some(mut mode) = build_mode else { return };

    if mode.line_anchor.is_some() {
        mode.line_anchor = None;
        for ghost in mode.line_ghosts.drain(..) {
            commands.entity(ghost).despawn();
        }
        return;
    }

    if let Some(preview) = mode.preview_entity {
        commands.entity(preview).despawn();
    }
    for ghost in mode.line_ghosts.drain(..) {
        commands.entity(ghost).despawn();
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
        let mut mode = BuildMode {
            selected: idx,
            rotation: 0.0,
            preview_entity: None,
            line_anchor: None,
            line_ghosts: Vec::new(),
        };
        refresh_build_preview(commands, &mut mode, item, registry, asset_server, meshes, materials);
        commands.insert_resource(mode);
    }
}

/// Hold left-mouse on a placed building for `PICKUP_HOLD_SECS` and the
/// building disappears, refunding one of its item back to the inventory.
fn pickup_held_building(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<crate::input::CursorState>,
    mut commands: Commands,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    mut hold: ResMut<PickupHold>,
    placed: Query<(Entity, &Transform, &PlacedBuilding)>,
) {
    if cursor.pointer_over_ui {
        hold.target = None;
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        hold.target = None;
        return;
    }
    let Some(cursor_pos) = cursor.cursor_world else {
        hold.target = None;
        return;
    };

    let mut closest: Option<(Entity, ItemId, f32)> = None;
    for (entity, tf, building) in &placed {
        let dx = tf.translation.x - cursor_pos.x;
        let dz = tf.translation.z - cursor_pos.z;
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
/// and by save/load.
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
        let mut e = commands.spawn((
            PlacedBuilding { item },
            SceneRoot(asset_server.load(path)),
            scaled,
        ));
        collision::attach_for_form(&mut e, def.form, &transform);
        return;
    }

    let mesh = def.form.make_mesh();
    let color = def.material.base_color();
    let mut e = commands.spawn((
        PlacedBuilding { item },
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.8,
            ..default()
        })),
        scaled,
    ));
    collision::attach_for_form(&mut e, def.form, &transform);
}
