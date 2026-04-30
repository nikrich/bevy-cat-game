use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

pub mod collision;
pub mod history;
pub mod ui;

pub use history::{apply_redo, apply_undo, BuildHistory, BuildOp, PieceRef};

use leafwing_input_manager::prelude::ActionState;

use bevy::gltf::{Gltf, GltfMesh, GltfNode};

use crate::input::{Action, CursorHit, CursorState};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{
    AabbBounds, Form, InteriorCatalog, ItemId, ItemRegistry, ItemTags, PlacementStyle,
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
                    resolve_interior_spawns,
                ),
            );
        collision::register(app);
        history::register(app);
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
/// Y window for "two pieces overlap". Tuned so:
///   - two cubes stacked vertically (Δy = 1.0) don't flag each other,
///   - a floor (lift 0.06) on top of a 1m wall (centre y = 0.5, Δy = 0.56)
///     doesn't see the wall as an occupant — that lets the player paint
///     2nd-storey floors over a stacked wall layer,
///   - two pieces at the same Y still conflict.
const OCCUPIED_Y: f32 = 0.5;

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

#[derive(Component)]
pub struct PlacedBuilding {
    pub item: ItemId,
}

#[derive(Component)]
struct BuildPreview;

/// Marker for an entity whose interior mesh+material are pending GLB
/// load. `resolve_interior_spawns` polls these and once the named node
/// resolves, spawns Mesh3d+MeshMaterial3d children for each primitive.
/// Removed from the entity once resolved.
///
/// `child_offset` and `child_scale_mul` are pre-computed at spawn time
/// (when we already have the catalog AABB) so resolve doesn't need
/// registry / catalog access. They make every interior asset render with
/// its AABB centred at the parent transform — without that, GLB nodes
/// whose origin is at a corner / floor placed visibly off-grid even with
/// correct snap.
#[derive(Component)]
pub struct InteriorSpawnRequest {
    pub gltf: Handle<Gltf>,
    pub node_name: String,
    /// Local translation applied to each spawned child mesh. Cancels the
    /// asset's intrinsic origin offset (set to `-aabb.centre`).
    pub child_offset: Vec3,
    /// Per-axis scale multiplier applied on top of the GLB node's own
    /// scale. `Vec3::ONE` for most assets; doors set `x` so the world
    /// width = 1 cube cell.
    pub child_scale_mul: Vec3,
}

/// Resolve the ghost chain's direction and length from cursor delta to
/// anchor on the **horizontal plane** (X/Z only). Vertical chains were
/// tried and reverted — terrain elevation differences and raycast hits at
/// varying heights produced false-positive vertical detections that broke
/// horizontal chains across uneven terrain. Vertical building is handled
/// by single-click face-stacking (click a cube's top face → cube above).
fn resolve_chain(anchor: Vec3, cursor: Vec3) -> (bool, f32, usize) {
    let dx = cursor.x - anchor.x;
    let dz = cursor.z - anchor.z;
    let cursor_moved = dx.abs() > 0.05 || dz.abs() > 0.05;
    let along_x = if cursor_moved { dx.abs() >= dz.abs() } else { true };
    let segment_length = if along_x { dx.abs() } else { dz.abs() };
    let n = (segment_length / WALL_LENGTH).round() as usize + 1;
    let raw_sign = if along_x { dx } else { dz };
    let dir_sign = if !cursor_moved || raw_sign >= 0.0 { 1.0 } else { -1.0 };
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
        // Hit terrain or a non-PlacedBuilding entity — snap hit XZ to the
        // terrain cell *centre*. Cells span [i, i+1] (see
        // `world::terrain` quad emit), so centres are at `i + 0.5`. Walls
        // and floors snapped this way visually fill the terrain tile they
        // sit on instead of straddling the boundary.
        let cx = hit.point.x.floor() + 0.5;
        let cz = hit.point.z.floor() + 0.5;
        let ty = terrain.height_at_or_sample(cx, cz, noise);
        return Vec3::new(cx, ty + new_lift, cz);
    }

    // No raycast hit — fall back to cursor's ground projection. Same
    // half-integer snap as the terrain-hit branch.
    let cx = cursor_world.x.floor() + 0.5;
    let cz = cursor_world.z.floor() + 0.5;
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

/// Wall-mounted interior categories — the door + window pieces from the
/// LowPoly Interior pack. Their width is force-stretched to a target cube
/// width (`cube_target_width`) so they fit cleanly into a wall row.
fn cube_target_width(def: &crate::items::ItemDef) -> Option<f32> {
    match def.interior_category.as_deref() {
        // 2-cell-wide pieces: each spans two wall cubes so the door / window
        // is centred between them (footprint x = 2 → snaps to integer x,
        // i.e. cell boundary), with one cube of frame on each side.
        Some("door") | Some("doors2") | Some("window") | Some("windows2") => Some(2.0),
        _ => None,
    }
}

/// Compute the per-asset child offset (cancels intrinsic origin offset so
/// the AABB centres at the parent transform) and per-axis scale multiplier
/// (door / window categories get x stretched to fit a fixed cube width).
/// Returns `(child_offset, child_scale_mul, effective_aabb)` where the
/// effective AABB is the post-stretch AABB used for footprint and Y
/// placement.
fn interior_render_params(
    def: &crate::items::ItemDef,
    aabb: AabbBounds,
) -> (Vec3, Vec3, AabbBounds) {
    let parent_scale = def.form.placement_scale();
    let mut scale_mul = Vec3::ONE;
    let mut effective = aabb;
    if let Some(target_world_x) = cube_target_width(def) {
        if aabb.size().x > 1e-4 {
            // World X = parent_scale * scale_mul.x * aabb.size().x → target
            let target = target_world_x / (parent_scale * aabb.size().x);
            scale_mul.x = target;
            effective = AabbBounds {
                min: Vec3::new(aabb.min.x * target, aabb.min.y, aabb.min.z),
                max: Vec3::new(aabb.max.x * target, aabb.max.y, aabb.max.z),
            };
        }
    }
    // Recentre: the child meshes already get scaled by node TRS + scale_mul,
    // so we shift them by -effective.centre() to land the AABB centre at
    // the parent's local origin.
    let child_offset = -effective.center();
    (child_offset, scale_mul, effective)
}

/// Snap a 1-D position to the cube grid based on a footprint dimension.
/// Terrain cells span `[i, i+1]` so cell *centres* are at `i + 0.5`.
/// Walls / floors snap to those half-integer centres (see
/// `compute_placement`), so:
///   - **odd footprint** → centre on a cell (half-integer, e.g. `0.5`).
///   - **even footprint** → centre between cells (integer, e.g. `1.0`),
///     so the asset's left + right edges land on cell boundaries.
/// Picking the wrong parity offsets the asset by half a cell and the
/// door / window won't line up with the surrounding wall row.
fn snap_axis(value: f32, cells: i32) -> f32 {
    if cells.rem_euclid(2) == 1 {
        value.floor() + 0.5
    } else {
        value.round()
    }
}

/// All cell centres a footprint of `cells.x × cells.y` would cover when
/// centred at `centre`. Used by the overlap check + the visual ghost.
fn footprint_cell_centres(centre: Vec3, cells: IVec2) -> Vec<Vec3> {
    let off_x = (cells.x - 1) as f32 * 0.5;
    let off_z = (cells.y - 1) as f32 * 0.5;
    let mut out = Vec::with_capacity((cells.x * cells.y) as usize);
    for ix in 0..cells.x {
        for iz in 0..cells.y {
            out.push(Vec3::new(
                centre.x + (ix as f32 - off_x),
                centre.y,
                centre.z + (iz as f32 - off_z),
            ));
        }
    }
    out
}

/// What an in-progress interior placement counts as "in the way".
#[derive(Clone, Copy)]
enum BlockingRule {
    /// Default for furniture / props / decorations: every placed piece
    /// blocks except floors (which are explicit "stand-on-top" surfaces).
    AnyExceptFloor,
    /// Carpets only: walls are the only blockers. Floors, other carpets,
    /// chairs, tables, lamps — all fine to overlap (a carpet visually
    /// goes under the items in the room).
    WallsOnly,
}

fn blocking_rule_for(def: &crate::items::ItemDef) -> BlockingRule {
    if matches!(def.interior_category.as_deref(), Some("carpet")) {
        BlockingRule::WallsOnly
    } else {
        BlockingRule::AnyExceptFloor
    }
}

/// True if every cell in the footprint at `centre` is clear of placed
/// pieces under the given `rule`. See [`BlockingRule`] for which forms
/// count as blockers.
fn footprint_clear(
    centre: Vec3,
    cells: IVec2,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    registry: &ItemRegistry,
    rule: BlockingRule,
) -> bool {
    footprint_cell_centres(centre, cells).into_iter().all(|c| {
        placed_q.iter().all(|(tf, b)| {
            let def = match registry.get(b.item) {
                Some(d) => d,
                None => return true,
            };
            let blocks = match rule {
                BlockingRule::AnyExceptFloor => !matches!(def.form, Form::Floor),
                BlockingRule::WallsOnly => matches!(def.form, Form::Wall),
            };
            if !blocks {
                return true;
            }
            !((tf.translation.x - c.x).abs() < OCCUPIED_RADIUS
                && (tf.translation.z - c.z).abs() < OCCUPIED_RADIUS
                && (tf.translation.y - c.y).abs() < OCCUPIED_Y)
        })
    })
}

/// Interior-item placement that respects the asset's pre-computed AABB:
/// snap XZ to the cube grid based on the asset's footprint cell count, set
/// Y so the asset's bottom rests exactly on the hit surface (terrain, wall
/// top, or floor top). Returns the entity *centre* position.
///
/// The AABB used for footprint + Y is the **effective** AABB from
/// `interior_render_params` — that's the post-stretch AABB for door
/// categories (forced 1m wide), and the original AABB for everything else.
/// `resolve_interior_spawns` recentres the children by `-effective.centre()`
/// so the rendered AABB ends up centred on the entity, hence the Y formula
/// is `scale * size.y / 2` rather than `-min.y * scale`.
fn compute_interior_placement(
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    def: &crate::items::ItemDef,
    aabb: AabbBounds,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    registry: &ItemRegistry,
    terrain: &Terrain,
    noise: &WorldNoise,
) -> (Vec3, IVec2) {
    let scale = def.form.placement_scale();
    let (_, _, effective) = interior_render_params(def, aabb);
    let footprint = effective.footprint_cells(scale);
    let bottom_offset = scale * effective.size().y * 0.5;

    // Pick the surface XZ + Y. Walls / floors snap to their top; terrain hit
    // snaps to terrain height; otherwise fall back to the cursor's ground
    // projection. Terrain height is sampled at the *snapped* XZ so the
    // asset doesn't bob when the cursor moves within a single cell.
    let (raw_x, raw_z, surface_y_at) = if let Some(hit) = cursor_hit {
        if let Ok((tf, building)) = placed_q.get(hit.entity) {
            if let Some(hit_def) = registry.get(building.item) {
                let hit_top = tf.translation.y + hit_def.form.placement_lift();
                if hit.normal.y > 0.7 {
                    (tf.translation.x, tf.translation.z, Some(hit_top))
                } else {
                    (hit.point.x, hit.point.z, None)
                }
            } else {
                (hit.point.x, hit.point.z, None)
            }
        } else {
            (hit.point.x, hit.point.z, None)
        }
    } else {
        (cursor_world.x, cursor_world.z, None)
    };

    // Wall-element categories (door / window) snap *both* axes to cell
    // boundaries (integer XZ). Z is forced to integer regardless of the
    // asset's natural depth — even though footprint.z is usually 1 (so
    // `snap_axis` would give a cell centre), the door / window itself only
    // makes sense sitting on the line where two perpendicular walls would
    // meet, so we override.
    let force_integer_z = cube_target_width(def).is_some();
    let snap_x = snap_axis(raw_x, footprint.x);
    let snap_z = if force_integer_z {
        raw_z.round()
    } else {
        snap_axis(raw_z, footprint.y)
    };
    let surface_y = surface_y_at
        .unwrap_or_else(|| terrain.height_at_or_sample(snap_x, snap_z, noise));
    (Vec3::new(snap_x, surface_y + bottom_offset, snap_z), footprint)
}

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
    mut history: ResMut<BuildHistory>,
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
                    let (offset, mul, _eff) = interior_render_params(def, aabb);
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
    placed_q: Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
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
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
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
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    terrain: &Terrain,
    noise: &WorldNoise,
    catalog: &InteriorCatalog,
    rotation: f32,
) -> Option<(Vec3, f32, bool)> {
    let def = registry.get(item)?;
    let name = def.interior_name.as_ref()?;
    let aabb = catalog.aabb_for(name)?;
    let (pos, footprint) = compute_interior_placement(
        cursor_world, cursor_hit, def, aabb, placed_q, registry, terrain, noise,
    );
    let valid = footprint_clear(pos, footprint, placed_q, registry, blocking_rule_for(def));
    Some((pos, rotation, valid))
}

/// Helper for `update_single_preview`: when a Replace form is selected,
/// returns `(position, yaw, true)` if the cursor is over a wall the piece
/// can swap into; `None` otherwise.
fn replace_preview_anchor(
    cursor_hit: Option<CursorHit>,
    form: Form,
    registry: &ItemRegistry,
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
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
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
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
    placed_q: Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    noise: Res<WorldNoise>,
    terrain: Res<Terrain>,
    mut history: ResMut<BuildHistory>,
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
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    history: &mut BuildHistory,
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
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
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

fn flush_paint_batch(mode: &mut BuildMode, history: &mut BuildHistory) {
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
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    registry: &ItemRegistry,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
    history: &mut BuildHistory,
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
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
    history: &mut BuildHistory,
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
    placed_q: &Query<(&Transform, &PlacedBuilding), Without<BuildPreview>>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    terrain: &Terrain,
    noise: &WorldNoise,
    history: &mut BuildHistory,
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
        let (pos, footprint) = compute_interior_placement(
            cursor_world, cursor_hit, def, aabb, placed_q, registry, terrain, noise,
        );
        if !footprint_clear(pos, footprint, placed_q, registry, blocking_rule_for(def)) {
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
    mut history: ResMut<BuildHistory>,
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
                let (offset, mul, _eff) = interior_render_params(def, aabb);
                (offset, mul)
            })
            .unwrap_or((Vec3::ZERO, Vec3::ONE));
        let mut e = commands.spawn((
            PlacedBuilding { item },
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
            PlacedBuilding { item },
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
            PlacedBuilding { item },
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
            PlacedBuilding { item },
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
        PlacedBuilding { item },
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

/// Async resolver for `InteriorSpawnRequest` — once the parent GLB is
/// loaded and the named node's mesh asset is ready, spawn one
/// Mesh3d+MeshMaterial3d child per primitive on the placed entity, then
/// remove the request component. The node's local transform is ignored;
/// the placed entity already carries the world transform from placement.
fn resolve_interior_spawns(
    mut commands: Commands,
    requests: Query<(Entity, &InteriorSpawnRequest)>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
) {
    for (entity, req) in &requests {
        let Some(gltf) = gltfs.get(&req.gltf) else { continue };
        let Some(node_handle) = gltf.named_nodes.get(req.node_name.as_str()) else {
            warn!("[interior] node '{}' not in parent GLB", req.node_name);
            commands.entity(entity).remove::<InteriorSpawnRequest>();
            continue;
        };
        let Some(node) = gltf_nodes.get(node_handle) else { continue };
        let Some(mesh_handle) = node.mesh.as_ref() else {
            // Some nodes are bare transforms with no mesh — nothing to render.
            commands.entity(entity).remove::<InteriorSpawnRequest>();
            continue;
        };
        let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) else { continue };

        // Preserve the node's rotation + scale so per-item authoring
        // intent (e.g. plant.008 has scale 0.108) carries through.
        // Translation comes from the source scene's grid layout — we
        // explicitly drop it. `child_offset` re-shifts so the asset's AABB
        // centre lands at the parent transform; `child_scale_mul` adds the
        // door-width stretch on top of the node scale.
        let local_tf = Transform {
            translation: req.child_offset,
            rotation: node.transform.rotation,
            scale: node.transform.scale * req.child_scale_mul,
        };
        commands.entity(entity).with_children(|parent| {
            for prim in &gltf_mesh.primitives {
                let mat = prim
                    .material
                    .clone()
                    .unwrap_or_default();
                parent.spawn((
                    Mesh3d(prim.mesh.clone()),
                    MeshMaterial3d(mat),
                    local_tf,
                ));
            }
        });
        commands.entity(entity).remove::<InteriorSpawnRequest>();
    }
}
