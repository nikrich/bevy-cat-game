use bevy::prelude::*;
use crate::edit::PlacedItem;
use crate::input::CursorHit;
use crate::items::{Form, ItemDef, ItemRegistry};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;
use super::BuildPreview;

/// Wall length in world units. The line tool stamps walls centred at
/// `anchor + (i + 0.5) * WALL_LENGTH * axis_dir` so each wall fills exactly
/// one 1 m cell along the dominant axis.
pub const WALL_LENGTH: f32 = 1.0;

/// XZ distance under which a planned wall position is considered already
/// covered by an existing placed piece. Skips that cell from both ghost and
/// placement so re-running a line over an existing wall — including the
/// shared corner cell of two perpendicular line segments — doesn't overlap.
pub const OCCUPIED_RADIUS: f32 = 0.4;
/// Y window for "two pieces overlap". Tuned so:
///   - two cubes stacked vertically (Δy = 1.0) don't flag each other,
///   - a floor (lift 0.06) on top of a 1m wall (centre y = 0.5, Δy = 0.56)
///     doesn't see the wall as an occupant — that lets the player paint
///     2nd-storey floors over a stacked wall layer,
///   - two pieces at the same Y still conflict.
pub const OCCUPIED_Y: f32 = 0.5;

/// Resolve the ghost chain's direction and length from cursor delta to
/// anchor on the **horizontal plane** (X/Z only). Vertical chains were
/// tried and reverted — terrain elevation differences and raycast hits at
/// varying heights produced false-positive vertical detections that broke
/// horizontal chains across uneven terrain. Vertical building is handled
/// by single-click face-stacking (click a cube's top face → cube above).
pub fn resolve_chain(anchor: Vec3, cursor: Vec3) -> (bool, f32, usize) {
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
pub fn anchor_from_hit(
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
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
pub fn wall_segment_transforms(anchor: Vec3, cursor: Vec3) -> Vec<Transform> {
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
pub fn segment_end(anchor: Vec3, cursor: Vec3) -> Vec3 {
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
pub fn compute_placement(
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    form: Form,
    registry: &ItemRegistry,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
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
        // Hit terrain or a non-PlacedItem entity — snap hit XZ to the
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
pub fn is_position_occupied(
    pos: Vec3,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
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
pub fn cube_target_width(def: &ItemDef) -> Option<f32> {
    match def.interior_category.as_deref() {
        // 2-cell-wide pieces: each spans two wall cubes so the door / window
        // is centred between them (footprint x = 2 → snaps to integer x,
        // i.e. cell boundary), with one cube of frame on each side.
        Some("door") | Some("doors2") | Some("window") | Some("windows2") => Some(2.0),
        _ => None,
    }
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
pub fn snap_axis(value: f32, cells: i32) -> f32 {
    if cells.rem_euclid(2) == 1 {
        value.floor() + 0.5
    } else {
        value.round()
    }
}

/// All cell centres a footprint of `cells.x × cells.y` would cover when
/// centred at `centre`. Used by the overlap check + the visual ghost.
pub fn footprint_cell_centres(centre: Vec3, cells: IVec2) -> Vec<Vec3> {
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
