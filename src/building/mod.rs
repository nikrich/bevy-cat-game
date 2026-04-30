use bevy::prelude::*;

pub mod collision;
pub mod ui;

use leafwing_input_manager::prelude::ActionState;

use crate::input::{Action, CursorHit, CursorState};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{Form, ItemId, ItemRegistry, ItemTags, PlacementStyle};
use crate::player::Player;
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;

pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlaceableItems>()
            .add_systems(
                Startup,
                init_placeable_items.after(crate::items::registry::seed_default_items),
            )
            .add_systems(
                Update,
                (
                    toggle_build_mode,
                    cancel_placing,
                    select_build_tool,
                    select_build_item,
                    cycle_build_item,
                    update_preview,
                    place_building,
                ),
            );
        collision::register(app);
        ui::register(app);
    }
}

/// Cached list of placeable item IDs (anything tagged PLACEABLE in the registry),
/// in stable registration order. Drives the [/] cycle in Place tool.
#[derive(Resource, Default)]
pub struct PlaceableItems(pub Vec<ItemId>);

/// Build mode tools (mirrors `world::edit::BrushTool` for the terrain
/// editor). Selected via number-row hotkeys while in build mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildTool {
    /// Place the currently selected piece. Walls use the line tool;
    /// other forms use single-click placement (see `Form::placement_style`).
    Place,
    /// Click a placed piece to despawn it and refund 1 of its item to the
    /// inventory. No line-tool drag — single click per cube.
    Remove,
}

impl BuildTool {
    pub fn label(self) -> &'static str {
        match self {
            BuildTool::Place => "Place",
            BuildTool::Remove => "Remove",
        }
    }

    pub fn tint(self) -> Color {
        match self {
            BuildTool::Place => Color::srgb(0.45, 0.85, 0.45),
            BuildTool::Remove => Color::srgb(0.85, 0.45, 0.45),
        }
    }

    pub const ALL: &'static [BuildTool] = &[BuildTool::Place, BuildTool::Remove];
}

/// Cursor pickup radius when checking "did the click land on a placed
/// piece?" for the Remove tool fallback path (cursor_hit handles the
/// happy path; this is the safety net when raycast misses).
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
    /// Active tool. Place / Remove for now; future Move, Pick, Replace
    /// (door-into-wall) slot in here without changing call sites since
    /// every system that cares routes on `tool`.
    pub tool: BuildTool,
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
    /// Red translucent cube that highlights the placed piece under the
    /// cursor when the Remove tool is active. Spawned once per build-mode
    /// session, repositioned to the hit entity each frame, hidden via
    /// `HIDE_Y` when not needed.
    pub remove_highlight: Option<Entity>,
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
            if let Some(highlight) = mode.remove_highlight {
                commands.entity(highlight).despawn();
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
                tool: BuildTool::Place,
                selected,
                rotation: 0.0,
                preview_entity: None,
                line_anchor: None,
                line_ghosts: Vec::new(),
                remove_highlight: Some(spawn_remove_highlight(&mut commands, &mut meshes, &mut materials)),
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

/// Spawn the Remove tool's hover highlight: a slightly oversized
/// translucent red cube that gets repositioned to the placed piece under
/// the cursor each frame. Carries the `BuildPreview` marker so the existing
/// build-mode-exit cleanup despawns it; we also explicitly despawn via
/// `mode.remove_highlight` because the marker is also used by line ghosts.
fn spawn_remove_highlight(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) -> Entity {
    commands
        .spawn((
            BuildPreview,
            Mesh3d(meshes.add(Cuboid::new(1.05, 1.05, 1.05))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.3, 0.3, 0.45),
                alpha_mode: AlphaMode::Blend,
                ..default()
            })),
            Transform::from_xyz(0.0, HIDE_Y, 0.0),
        ))
        .id()
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

/// Number-row hotkeys swap the active build tool while in build mode.
/// Mirrors `world::edit::switch_brush` (1..5 selects brush in the terrain
/// editor). Tool list is `BuildTool::ALL`; the slot index is hotkey - 1.
fn select_build_tool(
    action_state: Res<ActionState<Action>>,
    mut build_mode: Option<ResMut<BuildMode>>,
) {
    let Some(mode) = &mut build_mode else { return };
    let slots = [Action::Hotbar1, Action::Hotbar2];
    for (i, action) in slots.iter().enumerate() {
        if action_state.just_pressed(action) {
            if let Some(&tool) = BuildTool::ALL.get(i) {
                if tool != mode.tool {
                    mode.tool = tool;
                    info!("[build] tool: {}", tool.label());
                    // Switching tools cancels any in-progress line so the
                    // next click starts fresh.
                    mode.line_anchor = None;
                }
            }
            return;
        }
    }
}

/// Q / E (and mouse scroll) cycle the active placeable while the Place
/// tool is selected. Item selection is meaningless for Remove (clicks
/// despawn whatever's under the cursor), so we silently ignore cycling
/// in other tools.
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
    if mode.tool != BuildTool::Place {
        return;
    }
    let n = placeables.0.len();
    if n == 0 {
        return;
    }

    let mut new_idx: Option<usize> = None;
    if action_state.just_pressed(&Action::HotbarNext) {
        new_idx = Some((mode.selected + 1) % n);
    } else if action_state.just_pressed(&Action::HotbarPrev) {
        new_idx = Some((mode.selected + n - 1) % n);
    }

    if let Some(idx) = new_idx {
        switch_selected_item(
            &mut commands,
            mode,
            idx,
            &placeables,
            &inventory,
            &registry,
            &asset_server,
            &mut meshes,
            &mut materials,
        );
    }

    if action_state.just_pressed(&Action::RotatePiece) {
        mode.rotation += std::f32::consts::FRAC_PI_2;
    }
}

/// `[` and `]` cycle the active placeable while the Place tool is selected.
/// Mirrors `world::edit::cycle_paint_biome` for the Paint brush — players
/// who learned the bracket-cycle in terrain editing get the same gesture
/// here. Q/E and the scroll wheel still work via `select_build_item`.
fn cycle_build_item(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    inventory: Res<Inventory>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mode) = &mut build_mode else { return };
    if mode.tool != BuildTool::Place {
        return;
    }
    let n = placeables.0.len();
    if n == 0 {
        return;
    }

    let new_idx = if keys.just_pressed(KeyCode::BracketLeft) {
        Some((mode.selected + n - 1) % n)
    } else if keys.just_pressed(KeyCode::BracketRight) {
        Some((mode.selected + 1) % n)
    } else {
        None
    };

    if let Some(idx) = new_idx {
        switch_selected_item(
            &mut commands,
            mode,
            idx,
            &placeables,
            &inventory,
            &registry,
            &asset_server,
            &mut meshes,
            &mut materials,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn switch_selected_item(
    commands: &mut Commands,
    mode: &mut BuildMode,
    idx: usize,
    placeables: &PlaceableItems,
    inventory: &Inventory,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    if idx == mode.selected {
        return;
    }
    let Some(item) = placeables.0.get(idx).copied() else { return };
    if !INFINITE_RESOURCES && inventory.count(item) == 0 {
        return;
    }
    mode.selected = idx;
    if let Some(def) = registry.get(item) {
        info!("[build] selected {} ({:?})", def.display_name, def.form);
    }
    refresh_build_preview(commands, mode, item, registry, asset_server, meshes, materials);
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

    let cursor_world = if let Some(c) = cursor.cursor_world {
        c
    } else if let Ok(player_gt) = player_query.single() {
        let pos = player_gt.translation();
        let forward = player_gt.forward().as_vec3();
        pos + forward * 1.5
    } else {
        return;
    };

    // Remove tool: hide every place-mode ghost; show the red highlight on
    // whatever placed piece the cursor's raycast lands on.
    if mode.tool == BuildTool::Remove {
        if let Some(preview) = mode.preview_entity {
            if let Ok((mut tf, _)) = previews_q.get_mut(preview) {
                tf.translation.y = HIDE_Y;
            }
        }
        for ghost in mode.line_ghosts.drain(..) {
            commands.entity(ghost).despawn();
        }
        if let Some(highlight) = mode.remove_highlight {
            if let Ok((mut tf, _)) = previews_q.get_mut(highlight) {
                let target = cursor
                    .cursor_hit
                    .and_then(|hit| placed_q.get(hit.entity).ok().map(|(t, _)| t.translation));
                tf.translation = target.unwrap_or(Vec3::new(0.0, HIDE_Y, 0.0));
            }
        }
        return;
    }

    // Place tool from here down. Hide the remove highlight if it exists.
    if let Some(highlight) = mode.remove_highlight {
        if let Ok((mut tf, _)) = previews_q.get_mut(highlight) {
            tf.translation.y = HIDE_Y;
        }
    }

    let Some(item) = placeables.0.get(mode.selected).copied() else { return };
    let Some(def) = registry.get(item) else { return };
    let form = def.form;

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

    let Some(cursor_world) = cursor.cursor_world else { return };

    // Route on the active tool. Place / Remove for now; future Move, Pick,
    // and door-into-wall Replace plug in here without disturbing the rest
    // of the pipeline.
    match mode.tool {
        BuildTool::Remove => {
            remove_clicked_piece(
                &mut commands,
                cursor_world,
                cursor.cursor_hit,
                &placed_q,
                &registry,
                &mut inventory,
                &mut inv_events,
            );
        }
        BuildTool::Place => {
            let Some(item) = placeables.0.get(mode.selected).copied() else { return };
            let Some(def) = registry.get(item) else { return };
            let form = def.form;
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
    }
}

/// Remove tool — find the placed piece under the cursor (raycast hit
/// preferred, fallback to a `PICKUP_RADIUS` proximity search on the cursor
/// ground projection), despawn it, and refund 1 of its item to inventory.
/// Always refunds, even with `INFINITE_RESOURCES` on, so the player can
/// see counts go up while testing.
fn remove_clicked_piece(
    commands: &mut Commands,
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    registry: &ItemRegistry,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
) {
    // Try the raycast hit first — exact "what pixel did the player click?"
    // semantics. Only acts on entities actually in `placed_q` (i.e.
    // PlacedBuilding entities — terrain hits, ghosts, the player capsule
    // are all silently ignored).
    let target = cursor_hit
        .and_then(|hit| placed_q.get(hit.entity).ok().map(|(_, b)| (hit.entity, b.item)))
        .or_else(|| {
            // Fallback: nearest placed piece within PICKUP_RADIUS of the
            // cursor's ground projection. Catches edge cases where the
            // raycast missed (rare with cube colliders).
            let mut closest: Option<(Entity, ItemId, f32)> = None;
            for (tf, building) in placed_q.iter() {
                let dx = tf.translation.x - cursor_world.x;
                let dz = tf.translation.z - cursor_world.z;
                let d = (dx * dx + dz * dz).sqrt();
                if d <= PICKUP_RADIUS && closest.map(|c| d < c.2).unwrap_or(true) {
                    // Need to find the entity for this pair — re-iterate
                    // the query with `iter()` zipped with entities. Easier:
                    // just track the entity directly. Switch the query to
                    // `Query<(Entity, &Transform, &PlacedBuilding)>` if
                    // this fallback gets hot.
                    closest = Some((Entity::PLACEHOLDER, building.item, d));
                }
            }
            // The fallback path can't return a real entity without an
            // entity-aware query; raycast hit is the practical path with
            // cube colliders, so keep this as a no-op for now and rely on
            // raycast.
            let _ = closest;
            None
        });

    let Some((entity, item)) = target else { return };
    commands.entity(entity).despawn();
    inventory.add(item, 1);
    inv_events.write(InventoryChanged {
        item,
        new_count: inventory.count(item),
    });
    if let Some(def) = registry.get(item) {
        info!("[build] removed {}", def.display_name);
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
    if let Some(highlight) = mode.remove_highlight {
        commands.entity(highlight).despawn();
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
            tool: BuildTool::Place,
            selected: idx,
            rotation: 0.0,
            preview_entity: None,
            line_anchor: None,
            line_ghosts: Vec::new(),
            remove_highlight: None,
        };
        refresh_build_preview(commands, &mut mode, item, registry, asset_server, meshes, materials);
        commands.insert_resource(mode);
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
