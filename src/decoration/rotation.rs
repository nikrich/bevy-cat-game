//! Decoration mode rotation. Three behaviours layered on the same key:
//!
//! - **Tap `R`** (or `Shift+R`): instant 15-degree step. Snappy for
//!   precise alignment.
//! - **Hold `R`**: after a short delay the rotation starts sweeping
//!   smoothly, ramping from a slow start speed up to a peak. Lets the
//!   player dial in any angle without flicking the key 24 times.
//! - **`Alt+R` held**: skips the ramp and sweeps at peak speed
//!   immediately. Fastest path for big rotations.
//!
//! Mutates `DecorationMode::rotation_radians`, which is read by
//! `update_preview` (ghost), `place_decoration` (spawn), and
//! `carry_follow_cursor` (Move tool).

use bevy::prelude::*;

use crate::input::CursorState;

use super::placement::{quantize_rotation, ROTATION_STEP_RADIANS};
use super::DecorationMode;

/// How long `R` must be held (after the initial tap-step) before the
/// smooth ramp kicks in. Below this threshold a held key looks like a
/// long-tap; above it the player is clearly sweeping.
const HOLD_THRESHOLD: f32 = 0.18;

/// Time taken to reach `MAX_SPEED` once the ramp has started. A short
/// ramp feels responsive; too short and a 200 ms over-press flings the
/// piece around.
const RAMP_TIME: f32 = 0.45;

/// Slow-start speed when the ramp begins. ~30 deg/s.
const MIN_SPEED: f32 = std::f32::consts::PI / 6.0;

/// Peak rotation speed at the end of the ramp (and the constant rate
/// while `Alt+R` is held). ~270 deg/s.
const MAX_SPEED: f32 = std::f32::consts::PI * 1.5;

/// Per-frame state for the held-rotation ramp. `held_secs` accumulates
/// while `R` is pressed and resets on release; the ramp progresses as
/// `held_secs` exceeds `HOLD_THRESHOLD`.
#[derive(Resource, Default)]
pub struct RotationHold {
    pub held_secs: f32,
}

pub fn rotate_decoration(
    keys: Res<ButtonInput<KeyCode>>,
    decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
    time: Res<Time>,
    mut hold: ResMut<RotationHold>,
) {
    let Some(mut mode) = decoration_mode else {
        hold.held_secs = 0.0;
        return;
    };
    if cursor.keyboard_over_ui {
        hold.held_secs = 0.0;
        return;
    }

    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let r_held = keys.pressed(KeyCode::KeyR);
    let r_pressed = keys.just_pressed(KeyCode::KeyR);
    let dir: f32 = if shift { -1.0 } else { 1.0 };

    if !r_held {
        hold.held_secs = 0.0;
        return;
    }

    if r_pressed {
        // Initial tap: snap to the next 15-degree step. Reset the hold
        // timer so a quick tap+release stays tap-only and a long press
        // measures from this frame.
        let step = dir * ROTATION_STEP_RADIANS;
        mode.rotation_radians = quantize_rotation(mode.rotation_radians + step);
        hold.held_secs = 0.0;
        return;
    }

    // Held without just_pressed -- accumulate.
    hold.held_secs += time.delta_secs();

    // Alt skips the ramp entirely and runs at peak speed.
    if alt {
        mode.rotation_radians += dir * MAX_SPEED * time.delta_secs();
        return;
    }

    // Plain hold: wait out HOLD_THRESHOLD, then lerp speed across RAMP_TIME.
    if hold.held_secs <= HOLD_THRESHOLD {
        return;
    }
    let t = ((hold.held_secs - HOLD_THRESHOLD) / RAMP_TIME).clamp(0.0, 1.0);
    let speed = MIN_SPEED + (MAX_SPEED - MIN_SPEED) * t;
    mode.rotation_radians += dir * speed * time.delta_secs();
}
