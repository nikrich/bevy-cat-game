use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub mod catalog_ui;
pub mod collision;
pub mod placement;
pub mod ui;

pub use crate::edit::{apply_redo, apply_undo, BuildOp, EditHistory, PieceRef};
pub use placement::{
    compute_placement, anchor_from_hit, resolve_chain,
    wall_segment_transforms, segment_end, snap_axis,
    is_position_occupied, footprint_cell_centres,
    cube_target_width, OCCUPIED_RADIUS, OCCUPIED_Y, WALL_LENGTH,
};

use leafwing_input_manager::prelude::ActionState;

use crate::input::{Action, CursorHit, CursorState};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{
    Form, InteriorCatalog, ItemId, ItemRegistry, ItemTags, PlacementStyle,
};
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
                    toggle_xray,
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

pub fn init_placeable_items(
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
    /// Pieces stamped during the current paint drag (`PlacementStyle::Paint`,
    /// e.g. floors). Flushed into a single `BuildOp::Placed` history entry
    /// when the mouse is released so one drag = one undo.
    pub paint_batch: Vec<PieceRef>,
}

impl BuildMode {
    pub fn selected_item(&self, placeables: &PlaceableItems) -> Option<ItemId> {
        placeables.0.get(self.selected).copied()
    }
}

pub use crate::edit::PlacedItem;
pub use crate::decoration::interior::InteriorSpawnRequest;

#[derive(Component)]
pub(crate) struct BuildPreview;

fn toggle_build_mode(
    mut commands: Commands,
    action_state: Res<ActionState<Action>>,
    build_mode: Option<ResMut<BuildMode>>,
    inventory: Res<Inventory>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    catalog: Res<InteriorCatalog>,
    mut history: ResMut<EditHistory>,
    cursor: Res<CursorState>,
) {
    if cursor.keyboard_over_ui {
        return;
    }
    if !action_state.just_pressed(&Action::ToggleBuild) {
        return;
    }

    match build_mode {
        Some(mut mode) => {
            flush_paint_batch(&mut mode, &mut history);
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
            // Default to the first Wall variant — cubes are the build
            // primitive in the cube-grid model, so the player almost
            // always wants Wall ready on entry. Falls back to the first
            // placeable with stock if no walls are stocked.
            let stocked = |id: &ItemId| INFINITE_RESOURCES || inventory.count(*id) > 0;
            let selected = placeables
                .0
                .iter()
                .position(|id| {
                    registry
                        .get(*id)
                        .map(|d| matches!(d.form, Form::Wall) && stocked(id))
                        .unwrap_or(false)
                })
                .or_else(|| placeables.0.iter().position(stocked))
                .unwrap_or(0);
            let mut mode = BuildMode {
                tool: BuildTool::Place,
                selected,
                rotation: 0.0,
                preview_entity: None,
                line_anchor: None,
                line_ghosts: Vec::new(),
                remove_highlight: Some(spawn_remove_highlight(&mut commands, &mut meshes, &mut materials)),
                paint_batch: Vec::new(),
            };
            if let Some(item) = placeables.0.get(selected).copied() {
                // Always spawn a ghost on entry. The previous gate on
                // `inventory.count(item) > 0` left the build mode visually
                // empty when a save loaded with depleted counts (which can
                // happen even with INFINITE_RESOURCES if the save was
                // created without the cheat). The ghost is informational —
                // the actual placement still gates on inventory when
                // INFINITE_RESOURCES is off.
                refresh_build_preview(
                    &mut commands,
                    &mut mode,
                    item,
                    &registry,
                    &asset_server,
                    &mut meshes,
                    &mut materials,
                    &catalog,
                );
            }
            // Mutual exclusion: entering build mode exits decoration mode.
            commands.remove_resource::<crate::decoration::DecorationMode>();
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
    catalog: &InteriorCatalog,
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

    // Interior items: spawn the preview as an InteriorSpawnRequest so the
    // ghost shows the actual asset (resolved async by the same system that
    // resolves placed pieces). The mesh appears once the parent GLB loads.
    if let Some(name) = &def.interior_name {
        if let Some(idx) = catalog.by_name.get(name).copied() {
            let interior = &catalog.items[idx];
            let gltf = catalog.gltf_handle(interior.source).clone();
            let (child_offset, child_scale_mul) = interior
                .aabb_local
                .map(|aabb| {
                    let (offset, mul, _eff) = crate::decoration::interior::interior_render_params(def, aabb);
                    (offset, mul)
                })
                .unwrap_or((Vec3::ZERO, Vec3::ONE));
            let preview = commands
                .spawn((
                    BuildPreview,
                    xform,
                    Visibility::Inherited,
                    InteriorSpawnRequest {
                        gltf,
                        node_name: name.clone(),
                        child_offset,
                        child_scale_mul,
                    },
                ))
                .id();
            mode.preview_entity = Some(preview);
            return;
        }
    }

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

/// `X` toggles the indoor x-ray reveal while build mode is active. We read
/// the raw key (not a leafwing action) because `X` is already bound to
/// `Action::Examine` for the cat-verbs system; intercepting it here only
/// when build mode is on keeps the two uses from clashing. Suppressed when
/// egui has keyboard focus so typing in the catalog search bar doesn't
/// accidentally toggle the reveal.
fn toggle_xray(
    keyboard: Res<ButtonInput<KeyCode>>,
    build_mode: Option<Res<BuildMode>>,
    cursor: Res<CursorState>,
    mut indoor_settings: ResMut<crate::camera::occluder_fade::IndoorRevealSettings>,
) {
    if build_mode.is_none() || cursor.keyboard_over_ui {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyX) {
        indoor_settings.enabled = !indoor_settings.enabled;
    }
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

/// Q / E (and mouse scroll) cycle the active **structural** placeable
/// (cubes / walls) while the Place tool is selected. Decorations live in
/// the decoration catalog UI, not the keyboard cycle. Item selection is
/// meaningless for Remove (clicks despawn whatever's under the cursor),
/// so we silently ignore cycling in other tools.
#[allow(clippy::too_many_arguments)]
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
    catalog: Res<InteriorCatalog>,
) {
    let Some(mode) = &mut build_mode else { return };
    if mode.tool != BuildTool::Place {
        return;
    }

    let new_idx = if action_state.just_pressed(&Action::HotbarNext) {
        next_structural(&placeables, &registry, mode.selected, 1)
    } else if action_state.just_pressed(&Action::HotbarPrev) {
        next_structural(&placeables, &registry, mode.selected, -1)
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
            &catalog,
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
#[allow(clippy::too_many_arguments)]
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
    catalog: Res<InteriorCatalog>,
) {
    let Some(mode) = &mut build_mode else { return };
    if mode.tool != BuildTool::Place {
        return;
    }

    let new_idx = if keys.just_pressed(KeyCode::BracketLeft) {
        next_structural(&placeables, &registry, mode.selected, -1)
    } else if keys.just_pressed(KeyCode::BracketRight) {
        next_structural(&placeables, &registry, mode.selected, 1)
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
            &catalog,
        );
    }
}

/// Walk `placeables` from `current` in `dir` (+1 or -1), wrapping, and
/// return the next index whose item is tagged `STRUCTURAL`. Returns
/// `None` if no structural items exist (cycle is a no-op then).
///
/// Decorations and furniture are intentionally skipped — they live in the
/// decoration catalog UI (see `building::ui::draw_decoration_catalog`),
/// not the keyboard cycle, so the structural rotation stays focused.
fn next_structural(
    placeables: &PlaceableItems,
    registry: &ItemRegistry,
    current: usize,
    dir: i32,
) -> Option<usize> {
    let n = placeables.0.len() as i32;
    if n == 0 {
        return None;
    }
    let mut i = current as i32;
    for _ in 0..n {
        i = (i + dir).rem_euclid(n);
        let idx = i as usize;
        if let Some(def) = placeables.0.get(idx).and_then(|id| registry.get(*id)) {
            if def.tags.contains(ItemTags::STRUCTURAL) {
                return Some(idx);
            }
        }
    }
    None
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
    catalog: &InteriorCatalog,
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
    refresh_build_preview(commands, mode, item, registry, asset_server, meshes, materials, catalog);
}

const GHOST_VALID: Color = Color::srgba(0.45, 1.0, 0.55, 0.55);
const GHOST_INVALID: Color = Color::srgba(1.0, 0.35, 0.35, 0.55);
/// Far below the world — used to "hide" pooled ghost entities we don't need
/// this frame without despawning them.
const HIDE_Y: f32 = -100.0;

fn update_preview(
    mut commands: Commands,
    cursor: Res<CursorState>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    inventory: Res<Inventory>,
    placed_q: Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    // Material is optional so interior previews (which use a SceneRoot or
    // an InteriorSpawnRequest with the material attached as a child) get
    // their Transform updated without us trying to tint a material they
    // don't carry on the parent entity.
    mut previews_q: Query<
        (&mut Transform, Option<&MeshMaterial3d<StandardMaterial>>),
        With<BuildPreview>,
    >,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    noise: Res<WorldNoise>,
    terrain: Res<Terrain>,
    player_query: Query<&GlobalTransform, With<Player>>,
    catalog: Res<InteriorCatalog>,
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

    // Line tool is only active while Shift is held. The moment Shift is
    // released we drop the in-progress anchor + ghosts so the next click
    // is a clean single-cube placement.
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let line_active = shift && form.placement_style() == PlacementStyle::Line;

    if !line_active {
        if mode.line_anchor.is_some() {
            mode.line_anchor = None;
        }
        for ghost in mode.line_ghosts.drain(..) {
            commands.entity(ghost).despawn();
        }
    }

    if line_active && mode.line_anchor.is_some() {
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
            item,
            form,
            cursor_world,
            cursor.cursor_hit,
            &registry,
            &placed_q,
            &mut previews_q,
            &mut materials,
            &terrain,
            &noise,
            &catalog,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn update_single_preview(
    mode: &BuildMode,
    item: ItemId,
    form: Form,
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    registry: &ItemRegistry,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    previews_q: &mut Query<
        (&mut Transform, Option<&MeshMaterial3d<StandardMaterial>>),
        With<BuildPreview>,
    >,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
    catalog: &InteriorCatalog,
) {
    let Some(preview_entity) = mode.preview_entity else { return };
    let Ok((mut preview_tf, preview_mat)) = previews_q.get_mut(preview_entity) else { return };

    let style = form.placement_style();
    // Three distinct placement modes get their own snap rules:
    //   - Replace (door / window): snap to whichever wall the cursor targets.
    //   - Interior (the 1000-asset pack): snap XZ to footprint-derived grid
    //     cells, set Y so AABB bottom rests on the surface, refuse if any
    //     footprint cell is occupied.
    //   - Everything else (cubes, walls, decorations): the legacy
    //     compute_placement single-cell stack/adjacent rule.
    let (final_pos, final_yaw, valid) = if style == PlacementStyle::Replace {
        replace_preview_anchor(cursor_hit, form, registry, placed_q).unwrap_or_else(|| {
            (Vec3::new(cursor_world.x, HIDE_Y, cursor_world.z), 0.0, false)
        })
    } else if matches!(form, Form::Interior) {
        interior_preview_anchor(
            item,
            cursor_world,
            cursor_hit,
            registry,
            placed_q,
            terrain,
            noise,
            catalog,
            mode.rotation,
        )
        .unwrap_or((Vec3::new(cursor_world.x, HIDE_Y, cursor_world.z), 0.0, false))
    } else {
        // Same auto-stacking rule for every other form — single ghost shows
        // at the column top of the cursor's cell. The line tool's anchor
        // selection uses the same logic (`anchor_from_hit`), so what the
        // player sees here matches where the first wall lands after click.
        let pos = compute_placement(
            cursor_world, cursor_hit, form, registry, placed_q, terrain, noise,
        );
        (pos, mode.rotation, true)
    };

    preview_tf.translation = final_pos;
    preview_tf.rotation = Quat::from_rotation_y(final_yaw);

    // Tint the ghost when the preview entity carries its own StandardMaterial
    // (procedural cubes / walls). Interior previews use a SceneRoot or an
    // InteriorSpawnRequest with materials on child entities — those render
    // with their authored materials, no tint.
    if let Some(mat_handle) = preview_mat {
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.base_color = if valid { GHOST_VALID } else { GHOST_INVALID };
        }
    }
}

/// Helper for `update_single_preview` — same placement rule as the
/// click-time `compute_interior_placement` plus an occupancy check, so the
/// ghost shows exactly where the click would land *and* whether the
/// footprint is clear.
#[allow(clippy::too_many_arguments)]
fn interior_preview_anchor(
    item: ItemId,
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    registry: &ItemRegistry,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    terrain: &Terrain,
    noise: &WorldNoise,
    catalog: &InteriorCatalog,
    rotation: f32,
) -> Option<(Vec3, f32, bool)> {
    let def = registry.get(item)?;
    let name = def.interior_name.as_ref()?;
    let aabb = catalog.aabb_for(name)?;
    let (pos, footprint) = crate::decoration::interior::compute_interior_placement(
        cursor_world, cursor_hit, def, aabb, placed_q, registry, terrain, noise,
    );
    let valid = crate::decoration::interior::footprint_clear(pos, footprint, placed_q, registry, crate::decoration::interior::blocking_rule_for(def));
    Some((pos, rotation, valid))
}

/// Helper for `update_single_preview`: when a Replace form is selected,
/// returns `(position, yaw, true)` if the cursor is over a wall the piece
/// can swap into; `None` otherwise.
fn replace_preview_anchor(
    cursor_hit: Option<CursorHit>,
    form: Form,
    registry: &ItemRegistry,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
) -> Option<(Vec3, f32, bool)> {
    let hit = cursor_hit?;
    let (wall_tf, wall_b) = placed_q.get(hit.entity).ok()?;
    let wall_def = registry.get(wall_b.item)?;
    if !matches!(wall_def.form, Form::Wall) {
        return None;
    }
    let wall_bottom = wall_tf.translation.y - wall_def.form.placement_lift();
    let new_y = wall_bottom + form.placement_lift();
    let yaw = wall_tf.rotation.to_euler(EulerRot::YXZ).0;
    Some((Vec3::new(wall_tf.translation.x, new_y, wall_tf.translation.z), yaw, true))
}

#[allow(clippy::too_many_arguments)]
fn update_line_preview(
    commands: &mut Commands,
    mode: &mut BuildMode,
    item: ItemId,
    cursor_world: Vec3,
    inventory: &Inventory,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    previews_q: &mut Query<
        (&mut Transform, Option<&MeshMaterial3d<StandardMaterial>>),
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
            if let (Some(c), Some(mat_handle)) = (tint, mat_handle) {
                if let Some(mat) = materials.get_mut(&mat_handle.0) {
                    mat.base_color = c;
                }
            }
        }
    }
}

/// Bundle of input + UI-state resources that `place_building` consumes.
/// Bevy's tuple SystemParam tops out at 16 args; bundling keeps room for
/// the rest of the placement state (registry, inventory, terrain, etc.).
#[derive(SystemParam)]
pub struct BuildInputs<'w> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    keyboard: Res<'w, ButtonInput<KeyCode>>,
    action: Res<'w, ActionState<Action>>,
    cursor: Res<'w, CursorState>,
    crafting: Res<'w, crate::crafting::CraftingState>,
}

fn place_building(
    mut commands: Commands,
    inputs: BuildInputs,
    build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    placed_q: Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    noise: Res<WorldNoise>,
    terrain: Res<Terrain>,
    mut history: ResMut<EditHistory>,
    catalog: Res<InteriorCatalog>,
) {
    let Some(mut mode) = build_mode else { return };
    if inputs.cursor.pointer_over_ui || inputs.crafting.open {
        // Pointer-over-UI cancels any in-progress paint drag and flushes
        // whatever was painted so far — otherwise the next mouse-up over
        // world geometry would group unrelated stamps with the canceled run.
        flush_paint_batch(&mut mode, &mut history);
        return;
    }

    let click = inputs.mouse.just_pressed(MouseButton::Left)
        || inputs.action.just_pressed(&Action::Place);
    let held = inputs.mouse.pressed(MouseButton::Left);
    let released = inputs.mouse.just_released(MouseButton::Left);

    let Some(cursor_world) = inputs.cursor.cursor_world else {
        if released {
            flush_paint_batch(&mut mode, &mut history);
        }
        return;
    };

    // Route on the active tool. Place / Remove for now; future Move, Pick,
    // and door-into-wall Replace plug in here without disturbing the rest
    // of the pipeline.
    match mode.tool {
        BuildTool::Remove => {
            if !click {
                return;
            }
            remove_clicked_piece(
                &mut commands,
                cursor_world,
                inputs.cursor.cursor_hit,
                &placed_q,
                &registry,
                &mut inventory,
                &mut inv_events,
                &mut history,
            );
        }
        BuildTool::Place => {
            let Some(item) = placeables.0.get(mode.selected).copied() else { return };
            let Some(def) = registry.get(item) else { return };
            let form = def.form;
            let style = form.placement_style();
            // Plain click = single cube. Shift+click on a line-style form
            // (walls) opens the line tool: first shift+click sets anchor,
            // second shift+click confirms the chain. Releasing Shift in
            // `update_preview` clears any in-progress anchor and ghosts.
            let shift = inputs.keyboard.pressed(KeyCode::ShiftLeft)
                || inputs.keyboard.pressed(KeyCode::ShiftRight);

            // Paint forms (floors): every frame the mouse is held, stamp at
            // the cursor cell. Skips already-occupied cells so dragging back
            // and forth doesn't double-place. The whole drag becomes one
            // undo entry, flushed on mouse release below.
            if style == PlacementStyle::Paint {
                if held {
                    paint_stamp(
                        &mut commands,
                        &mut mode,
                        item,
                        form,
                        cursor_world,
                        inputs.cursor.cursor_hit,
                        &registry,
                        &asset_server,
                        &mut inventory,
                        &mut inv_events,
                        &placed_q,
                        &mut meshes,
                        &mut materials,
                        &terrain,
                        &noise,
                        &catalog,
                    );
                }
                if released {
                    flush_paint_batch(&mut mode, &mut history);
                }
                return;
            }

            if !click {
                return;
            }
            if style == PlacementStyle::Replace {
                place_replace(
                    &mut commands,
                    item,
                    form,
                    inputs.cursor.cursor_hit,
                    &registry,
                    &asset_server,
                    &mut inventory,
                    &mut inv_events,
                    &placed_q,
                    &mut meshes,
                    &mut materials,
                    &mut history,
                    &catalog,
                );
                return;
            }
            if shift && style == PlacementStyle::Line {
                place_wall_line(
                    &mut commands,
                    &mut mode,
                    item,
                    cursor_world,
                    inputs.cursor.cursor_hit,
                    &registry,
                    &asset_server,
                    &mut inventory,
                    &mut inv_events,
                    &placed_q,
                    &mut meshes,
                    &mut materials,
                    &terrain,
                    &noise,
                    &mut history,
                    &catalog,
                );
            } else {
                place_single(
                    &mut commands,
                    &mut mode,
                    item,
                    form,
                    cursor_world,
                    inputs.cursor.cursor_hit,
                    &registry,
                    &asset_server,
                    &mut inventory,
                    &mut inv_events,
                    &placed_q,
                    &mut meshes,
                    &mut materials,
                    &terrain,
                    &noise,
                    &mut history,
                    &catalog,
                );
            }
        }
    }
}

/// Stage 2 of Phase 2: swap a placed wall for a door / window.
/// The cursor must be over a `Form::Wall` cube. The wall is despawned and
/// refunded; the new piece spawns at the wall's XZ + yaw, with Y adjusted
/// so its bottom aligns with the wall's bottom (since doors are taller and
/// windows shorter than the 1m wall cube). The whole swap is one undo.
#[allow(clippy::too_many_arguments)]
fn place_replace(
    commands: &mut Commands,
    item: ItemId,
    form: Form,
    cursor_hit: Option<CursorHit>,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    history: &mut EditHistory,
    catalog: &InteriorCatalog,
) {
    if !INFINITE_RESOURCES && inventory.count(item) == 0 {
        return;
    }
    let Some(hit) = cursor_hit else { return };
    let Ok((wall_tf, wall_building)) = placed_q.get(hit.entity) else { return };
    let Some(wall_def) = registry.get(wall_building.item) else { return };
    if !matches!(wall_def.form, Form::Wall) {
        // Doors / windows only swap into walls; other forms ignore the click.
        return;
    }

    let wall_bottom = wall_tf.translation.y - wall_def.form.placement_lift();
    let new_y = wall_bottom + form.placement_lift();
    let yaw = wall_tf.rotation.to_euler(EulerRot::YXZ).0;
    let new_transform = Transform::from_xyz(wall_tf.translation.x, new_y, wall_tf.translation.z)
        .with_rotation(Quat::from_rotation_y(yaw));

    let old_piece = PieceRef {
        item: wall_building.item,
        transform: *wall_tf,
        entity: Some(hit.entity),
    };
    commands.entity(hit.entity).despawn();
    inventory.add(wall_building.item, 1);
    inv_events.write(InventoryChanged {
        item: wall_building.item,
        new_count: inventory.count(wall_building.item),
    });

    let new_entity = spawn_placed_building(
        commands, registry, asset_server, meshes, materials, catalog, item, new_transform,
    );
    let Some(new_entity) = new_entity else { return };
    if !INFINITE_RESOURCES {
        let entry = inventory.items.entry(item).or_insert(0);
        *entry = entry.saturating_sub(1);
        inv_events.write(InventoryChanged {
            item,
            new_count: inventory.count(item),
        });
    }
    history.record(BuildOp::Replaced {
        old: PieceRef { entity: None, ..old_piece },
        new: PieceRef {
            item,
            transform: new_transform,
            entity: Some(new_entity),
        },
    });
}

/// Paint a single tile at the cursor cell if it's not already occupied.
/// Pieces accumulate in `mode.paint_batch` and are flushed into one
/// `BuildOp::Placed` entry by `flush_paint_batch` on mouse release.
#[allow(clippy::too_many_arguments)]
fn paint_stamp(
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
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
    catalog: &InteriorCatalog,
) {
    if !INFINITE_RESOURCES && inventory.count(item) == 0 {
        return;
    }
    let pos =
        compute_placement(cursor_world, cursor_hit, form, registry, placed_q, terrain, noise);
    if is_position_occupied(pos, placed_q) {
        return;
    }
    let transform =
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(mode.rotation));
    let new_entity = spawn_placed_building(
        commands, registry, asset_server, meshes, materials, catalog, item, transform,
    );
    let Some(entity) = new_entity else { return };
    mode.paint_batch.push(PieceRef {
        item,
        transform,
        entity: Some(entity),
    });
    if !INFINITE_RESOURCES {
        let entry = inventory.items.entry(item).or_insert(0);
        *entry = entry.saturating_sub(1);
        inv_events.write(InventoryChanged {
            item,
            new_count: inventory.count(item),
        });
    }
}

fn flush_paint_batch(mode: &mut BuildMode, history: &mut EditHistory) {
    if mode.paint_batch.is_empty() {
        return;
    }
    let batch = std::mem::take(&mut mode.paint_batch);
    history.record(BuildOp::Placed(batch));
}

/// Remove tool — find the placed piece under the cursor (raycast hit
/// preferred, fallback to a `PICKUP_RADIUS` proximity search on the cursor
/// ground projection), despawn it, and refund 1 of its item to inventory.
/// Always refunds, even with `INFINITE_RESOURCES` on, so the player can
/// see counts go up while testing.
#[allow(clippy::too_many_arguments)]
fn remove_clicked_piece(
    commands: &mut Commands,
    _cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    registry: &ItemRegistry,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
    history: &mut EditHistory,
) {
    // Raycast hit is the only path with cube colliders — the camera ray
    // hits the cube directly when the player visually points at it. The
    // ground-projection fallback was always a dead branch in practice.
    let Some(hit) = cursor_hit else { return };
    let Ok((tf, building)) = placed_q.get(hit.entity) else { return };
    let item = building.item;
    let transform = *tf;
    let entity = hit.entity;

    commands.entity(entity).despawn();
    inventory.add(item, 1);
    inv_events.write(InventoryChanged {
        item,
        new_count: inventory.count(item),
    });
    history.record(BuildOp::Removed(vec![PieceRef {
        item,
        transform,
        entity: None,
    }]));
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
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
    history: &mut EditHistory,
    catalog: &InteriorCatalog,
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

            let mut placed_pieces: Vec<PieceRef> = Vec::new();
            for tx in &segment {
                if !INFINITE_RESOURCES && inventory.count(item) == 0 {
                    break;
                }
                if is_position_occupied(tx.translation, placed_q) {
                    continue;
                }
                let new_entity = spawn_placed_building(
                    commands,
                    registry,
                    asset_server,
                    meshes,
                    materials,
                    catalog,
                    item,
                    *tx,
                );
                if !INFINITE_RESOURCES {
                    let entry = inventory.items.entry(item).or_insert(0);
                    *entry = entry.saturating_sub(1);
                }
                if let Some(entity) = new_entity {
                    placed_pieces.push(PieceRef {
                        item,
                        transform: *tx,
                        entity: Some(entity),
                    });
                }
            }
            let placed_count = placed_pieces.len();
            if placed_count > 0 {
                if !INFINITE_RESOURCES {
                    inv_events.write(InventoryChanged {
                        item,
                        new_count: inventory.count(item),
                    });
                }
                history.record(BuildOp::Placed(placed_pieces));
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
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
    history: &mut EditHistory,
    catalog: &InteriorCatalog,
) {
    if !INFINITE_RESOURCES && inventory.count(item) == 0 {
        return;
    }

    // Interior items (1000-asset pack) snap to the cube grid based on their
    // pre-computed AABB footprint, with a strict no-overlap check across
    // every footprint cell. Other forms use the legacy single-cell
    // raycast-driven placement (ghost and click share that path so they
    // always land at the same spot).
    let (pos, footprint_check) = if matches!(form, Form::Interior) {
        let def = match registry.get(item) {
            Some(d) => d,
            None => return,
        };
        let Some(name) = def.interior_name.as_ref() else { return };
        let Some(aabb) = catalog.aabb_for(name) else { return };
        let (pos, footprint) = crate::decoration::interior::compute_interior_placement(
            cursor_world, cursor_hit, def, aabb, placed_q, registry, terrain, noise,
        );
        if !crate::decoration::interior::footprint_clear(pos, footprint, placed_q, registry, crate::decoration::interior::blocking_rule_for(def)) {
            // Overlap — refuse silently. Ghost is already showing red.
            return;
        }
        (pos, Some(footprint))
    } else {
        let pos = compute_placement(
            cursor_world, cursor_hit, form, registry, placed_q, terrain, noise,
        );
        (pos, None)
    };
    let _ = footprint_check;
    let transform =
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(mode.rotation));

    let new_entity = spawn_placed_building(
        commands, registry, asset_server, meshes, materials, catalog, item, transform,
    );
    if let Some(entity) = new_entity {
        history.record(BuildOp::Placed(vec![PieceRef {
            item,
            transform,
            entity: Some(entity),
        }]));
    }
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
    mut history: ResMut<EditHistory>,
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

    flush_paint_batch(&mut mode, &mut history);
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

/// Spawn a placed building from a known transform. Used by `place_building`,
/// the undo/redo restore path, and save/load. Returns the spawned entity
/// so callers can record it in the build history; returns `None` if the
/// item id isn't in the registry (callers can ignore the result).
pub fn spawn_placed_building(
    commands: &mut Commands,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    catalog: &crate::items::InteriorCatalog,
    item: ItemId,
    transform: Transform,
) -> Option<Entity> {
    let def = registry.get(item)?;
    let scale = def.form.placement_scale();
    let scaled = transform.with_scale(transform.scale * Vec3::splat(scale));

    // Interior items resolve via the catalog. The mesh+material attach
    // asynchronously through `resolve_interior_spawns` once the parent
    // GLB finishes loading; the entity is spawned now with an
    // `InteriorSpawnRequest` placeholder so undo/redo and save/load see
    // the entity immediately.
    if let Some(name) = &def.interior_name {
        let item_idx = catalog.by_name.get(name).copied()?;
        let interior = &catalog.items[item_idx];
        let gltf = catalog.gltf_handle(interior.source).clone();
        let (child_offset, child_scale_mul) = interior
            .aabb_local
            .map(|aabb| {
                let (offset, mul, _eff) = crate::decoration::interior::interior_render_params(def, aabb);
                (offset, mul)
            })
            .unwrap_or((Vec3::ZERO, Vec3::ONE));
        let mut e = commands.spawn((
            PlacedItem { item },
            scaled,
            // Visibility and Transform are inherited; the children spawned
            // by resolve will inherit position from this parent entity.
            Visibility::Inherited,
            InteriorSpawnRequest {
                gltf,
                node_name: name.clone(),
                child_offset,
                child_scale_mul,
            },
        ));
        collision::attach_for_form(&mut e, def.form, &transform);
        return Some(e.id());
    }

    if let Some(path) = def.form.scene_path() {
        let mut e = commands.spawn((
            PlacedItem { item },
            SceneRoot(asset_server.load(path)),
            scaled,
        ));
        collision::attach_for_form(&mut e, def.form, &transform);
        return Some(e.id());
    }

    // Door / window: composite frame matching the wall slot exactly. The
    // parent entity carries the Transform + collider; child entities carry
    // the visual cuboids (header, jambs, sill, pane). Built here rather
    // than via `make_mesh` because they need multiple materials (frame is
    // opaque wood, window pane is translucent glass).
    if matches!(def.form, Form::Door) {
        let color = def.material.base_color();
        let mut e = commands.spawn((
            PlacedItem { item },
            scaled,
            Visibility::Inherited,
        ));
        spawn_door_visuals(&mut e, color, meshes, materials);
        collision::attach_for_form(&mut e, def.form, &transform);
        return Some(e.id());
    }
    if matches!(def.form, Form::Window) {
        let color = def.material.base_color();
        let mut e = commands.spawn((
            PlacedItem { item },
            scaled,
            Visibility::Inherited,
        ));
        spawn_window_visuals(&mut e, color, meshes, materials);
        collision::attach_for_form(&mut e, def.form, &transform);
        return Some(e.id());
    }

    let mesh = def.form.make_mesh();
    let color = def.material.base_color();
    let mut mat = StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.8,
        ..default()
    };
    // Lantern glows. Emissive in linear space — values > 1 are physically
    // valid for HDR / bloom-aware pipelines. The warm yellow is biased
    // toward red so it reads as candle-y rather than fluorescent.
    if matches!(def.form, Form::Lantern) {
        mat.emissive = LinearRgba::new(2.5, 1.6, 0.5, 1.0);
    }
    let mut e = commands.spawn((
        PlacedItem { item },
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(mat)),
        scaled,
    ));
    // Lanterns also cast a warm point light so they actually illuminate
    // their surroundings at dusk/night, not just glow on themselves.
    if matches!(def.form, Form::Lantern) {
        e.with_children(|p| {
            p.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.78, 0.45),
                    intensity: 1_500_000.0,
                    range: 8.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 0.15, 0.0),
            ));
        });
    }
    collision::attach_for_form(&mut e, def.form, &transform);
    Some(e.id())
}

/// Build a door's visual frame as three child cuboids of `parent`: header
/// across the top, plus left + right jambs. The 0.7 × 0.85 opening between
/// them is intentionally empty so the cat walks through. Frame collider
/// (header + jambs) lives on the parent via `collision::attach_for_form`.
fn spawn_door_visuals(
    parent: &mut bevy::ecs::system::EntityCommands,
    color: Color,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let frame_mat = materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.85,
        ..default()
    });
    let header_mesh = meshes.add(Cuboid::new(1.0, 0.15, 0.18));
    let jamb_mesh = meshes.add(Cuboid::new(0.15, 0.85, 0.18));

    parent.with_children(|p| {
        p.spawn((
            Mesh3d(header_mesh.clone()),
            MeshMaterial3d(frame_mat.clone()),
            Transform::from_xyz(0.0, 0.425, 0.0),
        ));
        p.spawn((
            Mesh3d(jamb_mesh.clone()),
            MeshMaterial3d(frame_mat.clone()),
            Transform::from_xyz(-0.425, -0.075, 0.0),
        ));
        p.spawn((
            Mesh3d(jamb_mesh),
            MeshMaterial3d(frame_mat),
            Transform::from_xyz(0.425, -0.075, 0.0),
        ));
    });
}

/// Build a window's visual frame as four child cuboids of `parent`
/// (header, sill, two jambs) plus a translucent pane in the centre. The
/// collider is a single solid cuboid (see `collision::attach_for_form`)
/// because the 0.6 × 0.6 frame opening is just barely the cat's capsule
/// diameter, and a glass-thin opening would let the cat squeeze through.
fn spawn_window_visuals(
    parent: &mut bevy::ecs::system::EntityCommands,
    color: Color,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    let frame_mat = materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.85,
        ..default()
    });
    let pane_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.55, 0.78, 0.92, 0.35),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.10,
        ..default()
    });
    let horiz_mesh = meshes.add(Cuboid::new(1.0, 0.20, 0.18));
    let vert_mesh = meshes.add(Cuboid::new(0.20, 0.60, 0.18));
    let pane_mesh = meshes.add(Cuboid::new(0.60, 0.60, 0.04));

    parent.with_children(|p| {
        // Header
        p.spawn((
            Mesh3d(horiz_mesh.clone()),
            MeshMaterial3d(frame_mat.clone()),
            Transform::from_xyz(0.0, 0.40, 0.0),
        ));
        // Sill
        p.spawn((
            Mesh3d(horiz_mesh),
            MeshMaterial3d(frame_mat.clone()),
            Transform::from_xyz(0.0, -0.40, 0.0),
        ));
        // Left jamb
        p.spawn((
            Mesh3d(vert_mesh.clone()),
            MeshMaterial3d(frame_mat.clone()),
            Transform::from_xyz(-0.40, 0.0, 0.0),
        ));
        // Right jamb
        p.spawn((
            Mesh3d(vert_mesh),
            MeshMaterial3d(frame_mat),
            Transform::from_xyz(0.40, 0.0, 0.0),
        ));
        // Glass pane in the centre
        p.spawn((
            Mesh3d(pane_mesh),
            MeshMaterial3d(pane_mat),
            Transform::from_xyz(0.0, 0.0, 0.0),
        ));
    });
}

