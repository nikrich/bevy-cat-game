//! Decoration mode rotation: `R` advances 15 degrees, `Shift+R` reverses,
//! `Alt+R` (or Ctrl+R on platforms without Alt) sweeps continuously while
//! held. Mutates `DecorationMode::rotation_radians`, which is read by
//! `update_preview` (ghost), `place_decoration` (spawn), and
//! `carry_follow_cursor` (Move tool).

use bevy::prelude::*;

use crate::input::CursorState;

use super::placement::{quantize_rotation, ROTATION_STEP_RADIANS};
use super::DecorationMode;

/// Continuous rotation rate (radians per second) when Alt+R is held.
/// 180 deg/s lets the player sweep half a circle in one second.
const FREE_ROTATE_RATE: f32 = std::f32::consts::PI;

pub fn rotate_decoration(
    keys: Res<ButtonInput<KeyCode>>,
    decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
    time: Res<Time>,
) {
    let Some(mut mode) = decoration_mode else { return };
    if cursor.keyboard_over_ui {
        return;
    }
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let r_held = keys.pressed(KeyCode::KeyR);
    let r_pressed = keys.just_pressed(KeyCode::KeyR);

    if alt && r_held {
        // Continuous sweep while Alt+R is held.
        let dir = if shift { -1.0 } else { 1.0 };
        mode.rotation_radians += dir * FREE_ROTATE_RATE * time.delta_secs();
    } else if r_pressed {
        // Stepped rotation by 15 degrees per press.
        let step = if shift { -ROTATION_STEP_RADIANS } else { ROTATION_STEP_RADIANS };
        mode.rotation_radians = quantize_rotation(mode.rotation_radians + step);
    }
}
