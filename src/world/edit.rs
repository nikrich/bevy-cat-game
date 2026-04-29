//! Terrain brushes (Phase 1 / W1.10).
//!
//! Edit mode is a play-time tool that lets the player sculpt the terrain.
//! Toggle with `T` (mutually exclusive with build mode and the crafting
//! menu). While active:
//!
//! - `1 / 2 / 3` selects Raise / Lower / Flatten
//! - LMB held paints the brush at the cursor on a 100 ms tick
//! - Mouse wheel adjusts the brush radius (1.0 m -> 8.0 m)
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

use super::biome::WorldNoise;
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
}

impl BrushTool {
    fn tint(self) -> Color {
        match self {
            BrushTool::Raise => Color::srgb(0.45, 0.85, 0.45),
            BrushTool::Lower => Color::srgb(0.85, 0.45, 0.45),
            BrushTool::Flatten => Color::srgb(0.85, 0.80, 0.40),
        }
    }
}

#[derive(Resource)]
pub struct EditMode {
    pub active: bool,
    pub brush: BrushTool,
    pub radius: f32,
    /// Counts down each frame while LMB is held. When it hits zero we emit a
    /// painting tick and reset to [`TICK_PERIOD`]. Reset to zero on release
    /// so the next press fires immediately.
    tick_timer: f32,
    /// Captured surface Y under the cursor on LMB-press. Held for the
    /// lifetime of one press so the Flatten target stays put while the
    /// stroke sweeps the brush across the terrain (W1.10 spec).
    flatten_anchor: Option<f32>,
}

impl Default for EditMode {
    fn default() -> Self {
        Self {
            active: false,
            brush: BrushTool::Raise,
            radius: DEFAULT_RADIUS,
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
            adjust_radius,
            apply_brush,
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

/// `1 / 2 / 3` swap the active brush. Only consumed while edit mode is on
/// so the same keys keep working as build-mode hotbar slots when the
/// player switches back.
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

            let Some(current) = terrain.vertex_height(vx, vz) else {
                continue;
            };
            let new_h = match edit_mode.brush {
                BrushTool::Raise => step_height(current + STEP * falloff),
                BrushTool::Lower => step_height(current - STEP * falloff),
                BrushTool::Flatten => {
                    // Lerp toward the captured anchor. Inside the core,
                    // falloff = 1.0 so cells snap straight to the target.
                    // The edge band fades gracefully into surroundings.
                    step_height(current + (flatten_target - current) * falloff)
                }
            };
            if (new_h - current).abs() > f32::EPSILON {
                terrain.set_vertex_height(vx, vz, new_h);
            }
        }
    }
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
