//! Terrain brushes (Phase 1 / W1.10).
//!
//! Edit mode is a play-time tool that lets the player sculpt the terrain.
//! Toggle with `T` (mutually exclusive with build mode and the crafting
//! menu). While active:
//!
//! - `1..5` selects Raise / Lower / Flatten / Smooth / Paint
//! - LMB held paints the brush at the cursor on a 100 ms tick
//! - Mouse wheel adjusts the brush radius (1.0 m -> 8.0 m)
//! - `[` / `]` cycle the active paint biome while Paint is selected
//! - A gizmo ring shows the brush footprint in the brush's tint colour
//!
//! Each tick applies one 0.25 m step at the brush centre, falling off via
//! smoothstep across the radius. Heights live in [`Terrain`]; the regen
//! system rebuilds the affected chunks' mesh and trimesh collider, so the
//! edit shows up both visually and physically on the next frame's regen
//! budget.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::building::BuildMode;
use crate::crafting::CraftingState;
use crate::input::{Action, CursorState};

use super::biome::{Biome, WorldNoise};
use super::terrain::{step_height, Terrain};

/// One painting tick per 100 ms while LMB is held — enough to feel
/// responsive without snapping all 5+ cells under the cursor up by the full
/// step every frame.
const TICK_PERIOD: f32 = 0.10;
/// Vertical step applied at the centre of the brush per tick. Matches the
/// height-quantization grid so edits stay aligned to the chunky aesthetic.
const STEP: f32 = 0.25;
const MIN_RADIUS: f32 = 1.0;
const MAX_RADIUS: f32 = 8.0;
const DEFAULT_RADIUS: f32 = 2.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrushTool {
    Raise,
    Lower,
    Flatten,
    Smooth,
    Paint,
}

impl BrushTool {
    fn tint(self) -> Color {
        match self {
            BrushTool::Raise => Color::srgb(0.45, 0.85, 0.45),
            BrushTool::Lower => Color::srgb(0.85, 0.45, 0.45),
            BrushTool::Flatten => Color::srgb(0.85, 0.80, 0.40),
            BrushTool::Smooth => Color::srgb(0.55, 0.75, 0.95),
            BrushTool::Paint => Color::srgb(0.85, 0.55, 0.85),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BrushTool::Raise => "Raise",
            BrushTool::Lower => "Lower",
            BrushTool::Flatten => "Flatten",
            BrushTool::Smooth => "Smooth",
            BrushTool::Paint => "Paint",
        }
    }
}

/// Biomes the Paint brush can apply. Ocean is excluded: painting water onto
/// land doesn't lower the height to the wading floor (and the per-chunk
/// water plane only covers cells whose PCG biome is water), so the visual
/// would be a blue tile floating above the ground. Phase-2 work can revisit
/// this once Paint optionally drives heights.
pub const PAINT_BIOMES: &[Biome] = &[
    Biome::Beach,
    Biome::Desert,
    Biome::Grassland,
    Biome::Meadow,
    Biome::Forest,
    Biome::Taiga,
    Biome::Tundra,
    Biome::Snow,
    Biome::Mountain,
];

pub fn paint_biome_label(b: Biome) -> &'static str {
    match b {
        Biome::Ocean => "Ocean",
        Biome::Beach => "Beach",
        Biome::Desert => "Desert",
        Biome::Grassland => "Grassland",
        Biome::Meadow => "Meadow",
        Biome::Forest => "Forest",
        Biome::Taiga => "Taiga",
        Biome::Tundra => "Tundra",
        Biome::Snow => "Snow",
        Biome::Mountain => "Mountain",
    }
}

#[derive(Resource)]
pub struct EditMode {
    pub active: bool,
    pub brush: BrushTool,
    pub radius: f32,
    /// Index into [`PAINT_BIOMES`] for the biome the Paint brush applies.
    /// Cycled with `[` / `]` while the Paint brush is selected. Stays put
    /// across brush switches so coming back to Paint resumes the last
    /// chosen biome.
    pub paint_biome_index: usize,
    /// Counts down each frame while LMB is held. When it hits zero we emit a
    /// painting tick and reset to [`TICK_PERIOD`]. Reset to zero on release
    /// so the next press fires immediately.
    tick_timer: f32,
    /// Captured surface Y under the cursor on LMB-press. Held for the
    /// lifetime of one press so the Flatten target stays put while the
    /// stroke sweeps the brush across the terrain (W1.10 spec).
    flatten_anchor: Option<f32>,
}

impl EditMode {
    pub fn paint_biome(&self) -> Biome {
        PAINT_BIOMES[self.paint_biome_index.min(PAINT_BIOMES.len() - 1)]
    }
}

impl Default for EditMode {
    fn default() -> Self {
        Self {
            active: false,
            brush: BrushTool::Raise,
            radius: DEFAULT_RADIUS,
            // Grassland is the most "neutral" terrain; safe default that
            // contrasts visibly against most other biomes.
            paint_biome_index: PAINT_BIOMES
                .iter()
                .position(|b| matches!(b, Biome::Grassland))
                .unwrap_or(0),
            tick_timer: 0.0,
            flatten_anchor: None,
        }
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<EditMode>().add_systems(
        Update,
        (
            toggle_edit_mode,
            switch_brush,
            cycle_paint_biome,
            adjust_radius,
            apply_brush,
            apply_footprint_flatten,
            draw_brush_preview,
        ),
    );
}

/// `T` toggles edit mode. Suppressed while the crafting menu is open or
/// build mode is active, so the keypress never collides with another mode.
fn toggle_edit_mode(
    action_state: Res<ActionState<Action>>,
    crafting: Res<CraftingState>,
    build_mode: Option<Res<BuildMode>>,
    mut edit_mode: ResMut<EditMode>,
) {
    if !action_state.just_pressed(&Action::ToggleEditTerrain) {
        return;
    }
    if crafting.open || build_mode.is_some() {
        return;
    }
    edit_mode.active = !edit_mode.active;
    edit_mode.tick_timer = 0.0;
}

/// `1..5` swap the active brush. Only consumed while edit mode is on so
/// the same keys keep working as build-mode hotbar slots when the player
/// switches back.
fn switch_brush(
    action_state: Res<ActionState<Action>>,
    mut edit_mode: ResMut<EditMode>,
) {
    if !edit_mode.active {
        return;
    }
    if action_state.just_pressed(&Action::Hotbar1) {
        edit_mode.brush = BrushTool::Raise;
    } else if action_state.just_pressed(&Action::Hotbar2) {
        edit_mode.brush = BrushTool::Lower;
    } else if action_state.just_pressed(&Action::Hotbar3) {
        edit_mode.brush = BrushTool::Flatten;
    } else if action_state.just_pressed(&Action::Hotbar4) {
        edit_mode.brush = BrushTool::Smooth;
    } else if action_state.just_pressed(&Action::Hotbar5) {
        edit_mode.brush = BrushTool::Paint;
    }
}

/// `[` / `]` cycle the paint biome while Paint is selected. Read raw
/// because the bracket keys aren't bound to leafwing actions and we don't
/// want them to do anything outside edit mode.
fn cycle_paint_biome(
    keys: Res<ButtonInput<KeyCode>>,
    mut edit_mode: ResMut<EditMode>,
) {
    if !edit_mode.active || edit_mode.brush != BrushTool::Paint {
        return;
    }
    let len = PAINT_BIOMES.len();
    if keys.just_pressed(KeyCode::BracketLeft) {
        edit_mode.paint_biome_index = (edit_mode.paint_biome_index + len - 1) % len;
    } else if keys.just_pressed(KeyCode::BracketRight) {
        edit_mode.paint_biome_index = (edit_mode.paint_biome_index + 1) % len;
    }
}

/// Mouse wheel changes the brush radius while edit mode is on. Bound to
/// raw wheel events so it doesn't fight the camera's zoom or any other
/// scroll consumer that lives outside edit mode.
fn adjust_radius(
    mut edit_mode: ResMut<EditMode>,
    mut wheel: MessageReader<MouseWheel>,
) {
    if !edit_mode.active {
        wheel.clear();
        return;
    }
    let mut delta = 0.0;
    for ev in wheel.read() {
        delta += ev.y;
    }
    if delta != 0.0 {
        edit_mode.radius = (edit_mode.radius + delta * 0.5).clamp(MIN_RADIUS, MAX_RADIUS);
    }
}

/// One tick of brush painting at the cursor world position. We sample
/// every integer-coord vertex inside the brush radius, compute a falloff,
/// and call `Terrain::set_vertex_height` for each — the resource handles
/// dirty marking for affected chunks.
///
/// Falloff is a hard core + edge fade rather than a pure smoothstep: ~70%
/// of the radius gets full force, the outer ring smoothsteps to zero. A
/// pure smoothstep meant Flatten only really flattened the centre cell;
/// the new shape gives Flatten a clearly flat footprint with a soft
/// shoulder where it meets surrounding terrain.
fn apply_brush(
    time: Res<Time>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Res<CursorState>,
    noise: Res<WorldNoise>,
    mut edit_mode: ResMut<EditMode>,
    mut terrain: ResMut<Terrain>,
) {
    if !edit_mode.active {
        return;
    }
    let lmb_held = mouse.pressed(MouseButton::Left) && !cursor.pointer_over_ui;
    if !lmb_held {
        edit_mode.tick_timer = 0.0;
        edit_mode.flatten_anchor = None;
        return;
    }

    // Capture the Flatten anchor on the *first* frame of the press, before
    // any height edits run. After that, the cursor sweeping over edits
    // we've already made won't drift the target.
    if edit_mode.flatten_anchor.is_none() {
        if let Some(world_pos) = cursor.cursor_world {
            edit_mode.flatten_anchor =
                Some(terrain.height_at_or_sample(world_pos.x, world_pos.z, &noise));
        }
    }

    edit_mode.tick_timer -= time.delta_secs();
    if edit_mode.tick_timer > 0.0 {
        return;
    }
    edit_mode.tick_timer = TICK_PERIOD;

    let Some(world_pos) = cursor.cursor_world else {
        return;
    };
    let flatten_target = edit_mode
        .flatten_anchor
        .unwrap_or_else(|| terrain.height_at_or_sample(world_pos.x, world_pos.z, &noise));

    let r = edit_mode.radius;
    let r2 = r * r;
    let core_radius = r * 0.7;
    let core_r2 = core_radius * core_radius;
    let edge_band = (r - core_radius).max(0.0001);
    let xmin = (world_pos.x - r).floor() as i32;
    let xmax = (world_pos.x + r).ceil() as i32;
    let zmin = (world_pos.z - r).floor() as i32;
    let zmax = (world_pos.z + r).ceil() as i32;

    // Smooth reads each vertex's 4 cardinal neighbours and lerps toward
    // their average. Without a pre-tick snapshot, the iteration order
    // would feed half-updated heights back into later neighbour reads,
    // turning the brush into a directional smear. Build the snapshot once
    // per tick, keyed by the world (vx, vz) cells inside the brush bounds.
    let smooth_snapshot: Option<std::collections::HashMap<(i32, i32), f32>> =
        if edit_mode.brush == BrushTool::Smooth {
            let mut map = std::collections::HashMap::new();
            for vz in (zmin - 1)..=(zmax + 1) {
                for vx in (xmin - 1)..=(xmax + 1) {
                    if let Some(h) = terrain.vertex_height(vx, vz) {
                        map.insert((vx, vz), h);
                    }
                }
            }
            Some(map)
        } else {
            None
        };

    let paint_biome = edit_mode.paint_biome();

    for vz in zmin..=zmax {
        for vx in xmin..=xmax {
            let dx = vx as f32 - world_pos.x;
            let dz = vz as f32 - world_pos.z;
            let d2 = dx * dx + dz * dz;
            if d2 > r2 {
                continue;
            }
            let falloff = if d2 <= core_r2 {
                1.0
            } else {
                let edge_t = (d2.sqrt() - core_radius) / edge_band;
                let smoothed = edge_t * edge_t * (3.0 - 2.0 * edge_t);
                1.0 - smoothed
            };
            if falloff <= 0.0 {
                continue;
            }

            match edit_mode.brush {
                BrushTool::Raise => {
                    let Some(current) = terrain.vertex_height(vx, vz) else { continue };
                    let new_h = step_height(current + STEP * falloff);
                    if (new_h - current).abs() > f32::EPSILON {
                        terrain.set_vertex_height(vx, vz, new_h);
                    }
                }
                BrushTool::Lower => {
                    let Some(current) = terrain.vertex_height(vx, vz) else { continue };
                    let new_h = step_height(current - STEP * falloff);
                    if (new_h - current).abs() > f32::EPSILON {
                        terrain.set_vertex_height(vx, vz, new_h);
                    }
                }
                BrushTool::Flatten => {
                    let Some(current) = terrain.vertex_height(vx, vz) else { continue };
                    // Lerp toward the captured anchor. Inside the core,
                    // falloff = 1.0 so cells snap straight to the target.
                    // The edge band fades gracefully into surroundings.
                    let new_h = step_height(current + (flatten_target - current) * falloff);
                    if (new_h - current).abs() > f32::EPSILON {
                        terrain.set_vertex_height(vx, vz, new_h);
                    }
                }
                BrushTool::Smooth => {
                    let snap = smooth_snapshot.as_ref().unwrap();
                    let Some(&current) = snap.get(&(vx, vz)) else { continue };
                    // Average the four cardinal neighbours; if a neighbour
                    // is missing (chunk not loaded) skip it so the average
                    // stays meaningful. This keeps the smoothing stable
                    // across chunk boundaries instead of pulling toward an
                    // arbitrary fallback height.
                    let mut sum = 0.0;
                    let mut count = 0.0;
                    for (nx, nz) in [(vx - 1, vz), (vx + 1, vz), (vx, vz - 1), (vx, vz + 1)] {
                        if let Some(&n) = snap.get(&(nx, nz)) {
                            sum += n;
                            count += 1.0;
                        }
                    }
                    if count == 0.0 {
                        continue;
                    }
                    let avg = sum / count;
                    // Same lerp shape as Flatten so the brushes feel
                    // consistent: full effect inside the core ring,
                    // smoothstep edge.
                    let new_h = step_height(current + (avg - current) * falloff);
                    if (new_h - current).abs() > f32::EPSILON {
                        terrain.set_vertex_height(vx, vz, new_h);
                    }
                }
                BrushTool::Paint => {
                    // Paint is binary inside the core, gated at half the
                    // edge falloff outside. No tapered smoothstep — biomes
                    // are ids, not floats, so a partial paint is just a
                    // hard edge anyway. Half-strength gate gives the brush
                    // a softer-looking footprint without accidental dot
                    // patterns in the edge band.
                    if falloff < 0.5 {
                        continue;
                    }
                    if terrain.vertex_biome(vx, vz) != Some(paint_biome) {
                        terrain.set_vertex_biome(vx, vz, paint_biome);
                    }
                }
            }
        }
    }
}

/// W1.11 debug hotkey: pressing `F` while edit mode is active stamps a
/// 4×4 footprint flatten under the cursor with a 2-tile smoothstep skirt.
/// Phase 2's building-placement system will call `Terrain::flatten_rect`
/// itself; this debug binding just exercises the API.
fn apply_footprint_flatten(
    keys: Res<ButtonInput<KeyCode>>,
    cursor: Res<CursorState>,
    noise: Res<WorldNoise>,
    edit_mode: Res<EditMode>,
    mut terrain: ResMut<Terrain>,
) {
    if !edit_mode.active || !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Some(world_pos) = cursor.cursor_world else {
        return;
    };
    let cx = world_pos.x.round() as i32;
    let cz = world_pos.z.round() as i32;
    // 4×4 footprint centred on the cursor (cells [cx-2..cx+1] × [cz-2..cz+1]),
    // 2-tile smoothstep skirt — matches the spec's W1.11 example.
    terrain.flatten_rect(cx - 2, cz - 2, cx + 1, cz + 1, 2, &noise);
}

fn draw_brush_preview(
    edit_mode: Res<EditMode>,
    cursor: Res<CursorState>,
    terrain: Res<Terrain>,
    noise: Res<WorldNoise>,
    mut gizmos: Gizmos,
) {
    if !edit_mode.active {
        return;
    }
    let Some(world_pos) = cursor.cursor_world else {
        return;
    };
    // Float the ring just above the surface so it doesn't z-fight the
    // chunk mesh. Sample the centre height; the ring is flat — close
    // enough at typical brush sizes.
    let centre_y = terrain.height_at_or_sample(world_pos.x, world_pos.z, &noise) + 0.05;
    let iso = Isometry3d::new(
        Vec3::new(world_pos.x, centre_y, world_pos.z),
        Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
    );
    gizmos
        .circle(iso, edit_mode.radius, edit_mode.brush.tint())
        .resolution(48);
}
