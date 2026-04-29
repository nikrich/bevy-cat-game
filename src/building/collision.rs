//! Building collisions:
//! - Walls / doors / windows: a lateral block in the wall's local XZ frame.
//!   The cat is pushed out along the shortest penetration each frame.
//! - Floors / roofs: piggyback on `PropCollision` (the same component rocks
//!   and boulders use), so `snap_to_terrain` lifts the cat onto them.

use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, RigidBody};

use crate::camera::occluder_fade::NoOcclude;
use crate::crafting::CraftingState;
use crate::items::Form;
use crate::player::Player;
use crate::world::props::PropCollision;

const PLAYER_RADIUS: f32 = 0.30;
/// Capsule3d::new(0.3, 0.8) extends 0.7 above and below its centre. We use
/// that to decide whether the player vertically overlaps a wall.
const PLAYER_HALF_HEIGHT: f32 = 0.70;

/// Lateral collision against a wall-shaped placeable. Stored half-extents are
/// in the wall's local frame: `x` along its length, `y` along its thickness.
/// The world-space rotation comes from the entity's `GlobalTransform`.
#[derive(Component, Debug, Clone, Copy)]
pub struct WallCollision {
    pub half_extents: Vec2,
    pub bottom_y: f32,
    pub top_y: f32,
}

pub fn register(app: &mut App) {
    // Wall collision is now handled by rapier: walls spawn with their own
    // `Collider` (see `attach_for_form`) and rapier resolves penetration
    // against the player rigid body. The hand-rolled `push_player_out_of_walls`
    // system would fight rapier by mutating Transform directly, so it's
    // unregistered. Kept in source for reference until wall colliders are
    // proven out under playtest (W0.3 / DEBT-016).
    let _ = app;
    let _ = push_player_out_of_walls;
}

/// Attach the right collision component for `form` to a freshly-spawned
/// placed building. Called from `spawn_placed_building` so save-load and
/// runtime placement agree on collisions.
pub fn attach_for_form(entity: &mut EntityCommands, form: Form, transform: &Transform) {
    let pos = transform.translation;
    match form {
        Form::Floor => {
            // 1.0 x 0.12 x 1.0 cuboid centred on transform.y. Cat stands on
            // top, so it's never an occluder. PropCollision stays so the
            // existing examine/lookup code keeps working.
            entity.insert(PropCollision {
                top_y: pos.y + 0.06,
                radius: 0.71,
            });
            entity.insert(NoOcclude);
            entity.insert((Collider::cuboid(0.5, 0.06, 0.5), RigidBody::Fixed));
        }
        Form::Roof => {
            entity.insert(PropCollision {
                top_y: pos.y + 0.09,
                radius: 0.85,
            });
            entity.insert(NoOcclude);
            entity.insert((Collider::cuboid(0.6, 0.09, 0.6), RigidBody::Fixed));
        }
        Form::Wall => {
            // 1.0 x 1.6 x 0.15 in local frame; rapier picks up the entity's
            // rotation so the cuboid lines up with the painted wall.
            entity.insert(WallCollision {
                half_extents: Vec2::new(0.5, 0.075),
                bottom_y: pos.y - 0.8,
                top_y: pos.y + 0.8,
            });
            entity.insert((Collider::cuboid(0.5, 0.8, 0.075), RigidBody::Fixed));
        }
        Form::Door => {
            entity.insert(WallCollision {
                half_extents: Vec2::new(0.45, 0.06),
                bottom_y: pos.y - 0.85,
                top_y: pos.y + 0.85,
            });
            entity.insert((Collider::cuboid(0.45, 0.85, 0.06), RigidBody::Fixed));
        }
        Form::Window => {
            entity.insert(WallCollision {
                half_extents: Vec2::new(0.45, 0.06),
                bottom_y: pos.y - 0.4,
                top_y: pos.y + 0.4,
            });
            entity.insert((Collider::cuboid(0.45, 0.4, 0.06), RigidBody::Fixed));
        }
        _ => {}
    }
}

/// Resolve any wall overlap by translating the player out along the shortest
/// penetration axis in the wall's local XZ frame. Runs after `move_player`
/// so the resolution is the last word on this frame's player position.
fn push_player_out_of_walls(
    crafting: Res<CraftingState>,
    walls: Query<(&GlobalTransform, &WallCollision)>,
    mut player_q: Query<&mut Transform, With<Player>>,
) {
    if crafting.open {
        return;
    }
    let Ok(mut p_tf) = player_q.single_mut() else { return };

    for (wt, wall) in &walls {
        let wall_pos = wt.translation();
        let (yaw, _, _) = wt.rotation().to_euler(EulerRot::YXZ);

        let p_top = p_tf.translation.y + PLAYER_HALF_HEIGHT;
        let p_bot = p_tf.translation.y - PLAYER_HALF_HEIGHT;
        if p_top < wall.bottom_y || p_bot > wall.top_y {
            continue;
        }

        // Express the player's XZ relative to the wall in the wall's local
        // frame (rotation by -yaw).
        let dx = p_tf.translation.x - wall_pos.x;
        let dz = p_tf.translation.z - wall_pos.z;
        let cos = yaw.cos();
        let sin = yaw.sin();
        let local_x = dx * cos + dz * sin;
        let local_z = -dx * sin + dz * cos;

        let ext_x = wall.half_extents.x + PLAYER_RADIUS;
        let ext_z = wall.half_extents.y + PLAYER_RADIUS;
        let pen_x = ext_x - local_x.abs();
        let pen_z = ext_z - local_z.abs();
        if pen_x <= 0.0 || pen_z <= 0.0 {
            continue;
        }

        // Push along the smaller penetration axis so the cat slides along
        // the wall instead of stopping dead.
        let push_local = if pen_z < pen_x {
            Vec2::new(0.0, pen_z * local_z.signum())
        } else {
            Vec2::new(pen_x * local_x.signum(), 0.0)
        };
        let push_x = push_local.x * cos - push_local.y * sin;
        let push_z = push_local.x * sin + push_local.y * cos;
        p_tf.translation.x += push_x;
        p_tf.translation.z += push_z;
    }
}
