//! Player entity, movement, and pose. W0.3 + W0.4 (DEC-013, DEBT-016) move
//! movement from direct Transform manipulation onto a `bevy_tnua` floating
//! character controller backed by `bevy_rapier3d`. Gravity now lifts/lowers
//! the cat onto terrain colliders, so the old `snap_to_terrain` raycast is
//! gone.
//!
//! Tnua's "floating capsule" model means the character hovers `float_height`
//! above whatever it's standing on. The float spring + ground-cast handles
//! step-up automatically up to `step_offset`, which we tune generously to
//! traverse our 0.25-quantised stepped terrain without snagging on tile
//! seams. Once Phase 1 lands a smooth vertex-height grid the step-up
//! tolerance can drop.

use bevy::prelude::*;
use bevy_rapier3d::prelude::{
    Collider, GravityScale, LockedAxes, RigidBody,
};
use bevy_tnua::TnuaScheme;
use bevy_tnua::builtins::{TnuaBuiltinJump, TnuaBuiltinJumpConfig, TnuaBuiltinWalkConfig};
use bevy_tnua::prelude::{
    TnuaBuiltinWalk, TnuaConfig, TnuaController, TnuaUserControlsSystems,
};
use bevy_tnua_rapier3d::prelude::TnuaRapier3dSensorShape;
use leafwing_input_manager::prelude::ActionState;

use crate::crafting::CraftingState;
use crate::input::{iso_movement, Action, CursorState};
use crate::memory::verbs::CatVerbState;
use crate::save::LoadedPlayerPos;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        // The `TnuaScheme` derive on `ControlScheme` generates a sibling
        // `ControlSchemeConfig` asset type that holds the basis (and any
        // future action) configuration. Tnua loads it via `Handle`, so we
        // register the asset and inject one config per player.
        app.init_asset::<ControlSchemeConfig>()
            .add_systems(Startup, spawn_player)
            .add_systems(Update, (apply_loaded_position, pose_player))
            .add_systems(Update, drive_player.in_set(TnuaUserControlsSystems));
    }
}

#[derive(Component)]
pub struct Player;

const PLAYER_SPEED: f32 = 5.0;
/// Stalking multiplier on PLAYER_SPEED while Shift is held. Tuned slow enough
/// that animal-AI flee triggers (Phase D) can read "the cat is creeping" from
/// velocity alone.
const STALK_SPEED_MULT: f32 = 0.4;
/// Player-facing rotation speed when turning toward the cursor in build mode.
const FACE_LERP: f32 = 12.0;

/// Tnua control scheme — `TnuaBuiltinWalk` is the basis (always-on
/// horizontal locomotion), `Jump` is an action triggered by the leafwing
/// `Jump` action. The derive macro generates a sibling `ControlSchemeConfig`
/// asset with one field per variant (`basis: TnuaBuiltinWalkConfig`,
/// `jump: TnuaBuiltinJumpConfig`).
#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum ControlScheme {
    Jump(TnuaBuiltinJump),
}

fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut control_scheme_configs: ResMut<Assets<ControlSchemeConfig>>,
) {
    let body_color = Color::srgb(0.76, 0.60, 0.42);

    // Capsule3d::new(0.3, 0.8) -> radius 0.3, segment length 0.8, total
    // height 0.8 + 2*0.3 = 1.4. The bottom is 0.7 below the entity's centre,
    // so float_height must be > 0.7 for tnua to hold the cat off the ground;
    // 1.0 keeps the whole capsule in air with a 0.3 hover gap that absorbs
    // small terrain bumps.
    commands.spawn((
        Player,
        Mesh3d(meshes.add(Mesh::from(Capsule3d::new(0.3, 0.8)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: body_color,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(0.0, 5.0, 0.0),
        // Physics: dynamic body, capsule collider matching the visual mesh,
        // rotation locked so the cat stays upright (tnua re-orients slowly
        // through `desired_forward` rather than letting torque tumble it).
        RigidBody::Dynamic,
        Collider::capsule_y(0.4, 0.3),
        LockedAxes::ROTATION_LOCKED,
        GravityScale(1.0),
        // Tnua controller + walk config. `step_offset` is generous so the
        // 0.25-stepped chunky terrain reads as a slope, not a wall. Phase 1
        // can tighten this once the terrain mesh is smooth.
        TnuaController::<ControlScheme>::default(),
        TnuaConfig::<ControlScheme>(control_scheme_configs.add(
            ControlSchemeConfig {
                basis: TnuaBuiltinWalkConfig {
                    // `speed` is the m/s the cat reaches when `desired_motion`
                    // is a unit vector. The control system feeds a factor in
                    // [0, 1] (1.0 normal, 0.4 stalking), so the cap here is
                    // the natural top speed.
                    speed: PLAYER_SPEED,
                    float_height: 1.0,
                    // Tight cling so the float spring only catches ground
                    // close under the cat. Stepping up onto stumps and out
                    // of water uses the Jump action instead of an
                    // overly-generous cling that would let the cat skim
                    // over short walls without input.
                    cling_distance: 0.3,
                    spring_strength: 400.0,
                    spring_dampening: 1.2,
                    acceleration: 50.0,
                    air_acceleration: 20.0,
                    coyote_time: 0.15,
                    free_fall_extra_gravity: 60.0,
                    tilt_offset_angvel: 5.0,
                    tilt_offset_angacl: 500.0,
                    turning_angvel: f32::INFINITY,
                    max_slope: std::f32::consts::FRAC_PI_3, // 60° tolerates stepped seams
                    ..Default::default()
                },
                jump: TnuaBuiltinJumpConfig {
                    // Height is centre-to-peak: with float_height=1.0 the
                    // cat's centre rests at y=1.0, so a 1.6 jump reaches
                    // y≈2.6 — enough to clear a one-tile beach step
                    // (~0.5) plus a small prop (~0.5) with margin.
                    height: 1.6,
                    ..Default::default()
                },
            },
        )),
        // A short cylinder under the capsule probes for ground; using a
        // sensor shape (not a ray) prevents falling between tile edges.
        TnuaRapier3dSensorShape(bevy_rapier3d::parry::shape::SharedShape::cylinder(
            0.0, 0.28,
        )),
    ));
}

/// Push leafwing's Move axis into the Tnua walk basis. Iso-rotated so
/// "up on stick" walks toward the back of the camera, matching the previous
/// movement feel. Sprint scales the desired velocity directly.
fn drive_player(
    action_state: Res<ActionState<Action>>,
    cursor: Res<CursorState>,
    crafting: Res<CraftingState>,
    build_mode: Option<Res<crate::building::BuildMode>>,
    mut query: Query<(&Transform, &mut TnuaController<ControlScheme>), With<Player>>,
) -> Result {
    let (transform, mut controller) = query.single_mut()?;
    controller.initiate_action_feeding();

    let dir2 = if crafting.open {
        Vec2::ZERO
    } else {
        iso_movement(&action_state)
    };
    // `desired_motion` is direction-times-factor in [0, 1]; the basis config
    // already owns the m/s cap via `speed`. Stalking just shrinks the factor
    // so the cat creeps at 40% top speed without a separate config.
    let factor = if action_state.pressed(&Action::Sprint) {
        STALK_SPEED_MULT
    } else {
        1.0
    };
    let desired = Vec3::new(dir2.x, 0.0, -dir2.y) * factor;

    // In build mode the cat faces the cursor; otherwise it faces the
    // movement direction (or keeps its previous facing if standing still).
    let desired_forward = if build_mode.is_some() {
        cursor.cursor_world.and_then(|cursor_pos| {
            let to_cursor = cursor_pos - transform.translation;
            Dir3::new(Vec3::new(to_cursor.x, 0.0, to_cursor.z)).ok()
        })
    } else if desired.length_squared() > 0.0001 {
        Dir3::new(desired).ok()
    } else {
        None
    };

    controller.basis = TnuaBuiltinWalk {
        desired_motion: desired,
        desired_forward,
        ..Default::default()
    };

    // Feed Jump every frame the button is held so tnua can build a full
    // jump arc; releasing early stops the action and the jump shortens
    // (variable-height jumping). Jump is suppressed while the build menu
    // is up because Space is overloaded for placement there.
    if action_state.pressed(&Action::Jump) && build_mode.is_none() {
        controller.action(ControlScheme::Jump(TnuaBuiltinJump::default()));
    }

    Ok(())
}

/// Visible feedback for the verb-holds: while pressing Z (nap) the cat scales
/// down toward a curl; while pressing C (mark) it stretches up; while holding
/// Shift (stalk) it sinks to a crouch. Eases back when buttons release. Pure
/// visual; reads `CatVerbState` for hold progress so the cat actually moves
/// through the action rather than snapping at the end.
fn pose_player(
    action_state: Res<ActionState<Action>>,
    verbs: Res<CatVerbState>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Player>>,
) -> Result {
    let mut transform = query.single_mut()?;

    let nap_amt = verbs.nap_fraction();
    let mark_amt = verbs.mark_fraction();
    let stalking = if action_state.pressed(&Action::Sprint) { 1.0 } else { 0.0 };

    let target_y = 1.0 - 0.45 * nap_amt - 0.25 * stalking + 0.15 * mark_amt;
    let target_xz = 1.0 + 0.10 * stalking + 0.08 * nap_amt;

    let lerp = (8.0 * time.delta_secs()).min(1.0);
    let s = transform.scale;
    transform.scale = Vec3::new(
        s.x + (target_xz - s.x) * lerp,
        s.y + (target_y - s.y) * lerp,
        s.z + (target_xz - s.z) * lerp,
    );

    let _ = FACE_LERP; // reserved for future smoothing pass

    Ok(())
}

fn apply_loaded_position(
    mut commands: Commands,
    loaded: Option<Res<LoadedPlayerPos>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Some(loaded) = loaded else { return };
    let Ok(mut transform) = query.single_mut() else { return };
    // Lift the loaded position a bit so the player drops onto terrain rather
    // than spawning inside it before colliders settle.
    transform.translation = loaded.0 + Vec3::Y * 1.0;
    commands.remove_resource::<LoadedPlayerPos>();
}
