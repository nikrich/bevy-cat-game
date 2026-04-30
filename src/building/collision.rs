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
            // top. We deliberately *don't* add `NoOcclude` here so a floor
            // above the player (i.e. the upper-storey slab acting as a
            // ceiling) gets faded by the indoor reveal pass. The camera-
            // line fade in `occluder_fade` skips Form::Floor explicitly so
            // the floor under the player never goes translucent.
            entity.insert(PropCollision {
                top_y: pos.y + 0.06,
                radius: 0.71,
            });
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
            // Compound: header + 2 jambs only — the bottom 0.85 × 0.7 of the
            // 1×1×0.18 footprint is open so the cat walks through. Matches
            // the visual frame built by `spawn_door_composite`.
            let header = (
                Vec3::new(0.0, 0.425, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(0.5, 0.075, 0.09),
            );
            let left_jamb = (
                Vec3::new(-0.425, -0.075, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(0.075, 0.425, 0.09),
            );
            let right_jamb = (
                Vec3::new(0.425, -0.075, 0.0),
                Quat::IDENTITY,
                Collider::cuboid(0.075, 0.425, 0.09),
            );
            entity.insert((
                Collider::compound(vec![header, left_jamb, right_jamb]),
                RigidBody::Fixed,
            ));
        }
        Form::Window => {
            // Solid 1×1×0.18 cuboid. The composite visual reads as a window
            // (frame + pane) but cats can't squeeze through the 0.6 m gap
            // between the jambs (their capsule diameter is 0.6 m, so it'd
            // be a hairsbreadth pass-through if the collider only covered
            // the frame).
            entity.insert((Collider::cuboid(0.5, 0.5, 0.09), RigidBody::Fixed));
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
