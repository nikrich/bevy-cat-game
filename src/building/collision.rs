//! Collision attachment for placed building pieces. Walls/doors/windows
//! get a rapier cuboid collider; floors/roofs additionally get the
//! `PropCollision` marker so `snap_to_terrain` lifts the cat onto them.
//!
//! Penetration resolution is handled by rapier — there's no hand-rolled
//! "push the cat out of walls" system anymore (the historical
//! `WallCollision` + `push_player_out_of_walls` pair was removed once the
//! cube migration proved out the rapier-only collision path).

use bevy::prelude::*;
use bevy_rapier3d::prelude::{Collider, RigidBody};

use crate::camera::occluder_fade::NoOcclude;
use crate::items::Form;
use crate::world::props::PropCollision;

pub fn register(_app: &mut App) {
    // No systems — rapier owns collision resolution. Function kept for the
    // call site in `BuildingPlugin::build` and so future hooks have a home.
}

/// Attach the right collision components for `form` to a freshly-spawned
/// placed building. Called from `spawn_placed_building` so save-load and
/// runtime placement agree on collisions.
pub fn attach_for_form(entity: &mut EntityCommands, form: Form, transform: &Transform) {
    let pos = transform.translation;
    match form {
        Form::Floor => {
            // 1.0 x 0.12 x 1.0 cuboid centred on transform.y. Cat stands on
            // top, so it's never an occluder. PropCollision keeps the
            // existing examine/lookup code working.
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
            // Full 1×1×1 cube — Minecraft block. Rotation is irrelevant for
            // a symmetric cube but the line tool still rotates the entity;
            // collider stays cubic regardless.
            entity.insert((Collider::cuboid(0.5, 0.5, 0.5), RigidBody::Fixed));
        }
        Form::Door => {
            // 0.9 × 1.7 × 0.12 — won't be migrated to a cube until the
            // door-into-wall replacement flow lands (Stage 2 of Phase 2).
            entity.insert((Collider::cuboid(0.45, 0.85, 0.06), RigidBody::Fixed));
        }
        Form::Window => {
            // 0.9 × 0.8 × 0.12 — same Stage 2 caveat as Door.
            entity.insert((Collider::cuboid(0.45, 0.4, 0.06), RigidBody::Fixed));
        }
        Form::Interior => {
            // Default 1m cube collider for runtime-loaded interior items.
            // Per-item AABB-derived colliders would be tighter; tune in a
            // polish pass once we sample the loaded mesh bounds.
            entity.insert((Collider::cuboid(0.5, 0.5, 0.5), RigidBody::Fixed));
        }
        _ => {}
    }
}
