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

use std::time::Duration;

use bevy::prelude::*;
use bevy_rapier3d::prelude::{
    Collider, GravityScale, LockedAxes, RigidBody, Velocity,
};
use bevy_tnua::TnuaScheme;
use bevy_tnua::builtins::{TnuaBuiltinJump, TnuaBuiltinJumpConfig, TnuaBuiltinWalkConfig};
use bevy_tnua::prelude::{
    TnuaBuiltinWalk, TnuaConfig, TnuaController, TnuaUserControlsSystems,
};
use bevy_tnua_rapier3d::prelude::TnuaRapier3dSensorShape;
use leafwing_input_manager::prelude::ActionState;

use crate::crafting::CraftingState;
use crate::camera::CameraOrbit;
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
            .add_systems(Startup, (spawn_player, load_kitten_animations))
            .add_systems(
                Update,
                (
                    apply_loaded_position,
                    pose_player,
                    attach_kitten_animations,
                    drive_kitten_animation,
                ),
            )
            .add_systems(Update, drive_player.in_set(TnuaUserControlsSystems));
    }
}

/// Animation graph + node indices for the player kitten. Built once at
/// startup and reused every time a fresh `AnimationPlayer` shows up under
/// the kitten scene (e.g. on hot reload or if the visual is respawned).
#[derive(Resource)]
struct KittenAnimations {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    run: AnimationNodeIndex,
    jump: AnimationNodeIndex,
    sneak: AnimationNodeIndex,
    pickup: AnimationNodeIndex,
    swim: AnimationNodeIndex,
}

#[derive(Component)]
pub struct Player;

/// Top speed in m/s — reached when Sprint is held. The basis caps motion at
/// this value; walking just underdrives the basis via `WALK_FACTOR`.
const RUN_SPEED: f32 = 8.0;
/// Walk-mode factor on RUN_SPEED. 0.625 puts walking at 5 m/s, matching the
/// previous default before sprint-as-run replaced sprint-as-stalk.
const WALK_FACTOR: f32 = 0.625;
/// Animation playback rate for the Mixamo Walk clip. The clip's natural
/// foot-pace was authored for ~1.5 m/s; our walk is 5 m/s (RUN_SPEED *
/// WALK_FACTOR), so the feet need to cycle ~3x faster to avoid sliding.
/// Tune by feel.
const WALK_ANIM_SPEED: f32 = 2.0;
/// Animation playback rate for the Mixamo Run clip. Mixamo Run cycles at
/// ~3.5 m/s natural; our run is 8 m/s, so ~2.3x. Tune by feel.
const RUN_ANIM_SPEED: f32 = 2.3;
/// Pickup clip is sped up 3× so the cat snaps the prop instead of doing a
/// languid Mixamo squat. PICKUP_DURATION is scaled to match.
const PICKUP_ANIM_SPEED: f32 = 3.0;
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
    asset_server: Res<AssetServer>,
    mut control_scheme_configs: ResMut<Assets<ControlSchemeConfig>>,
) {
    // Capsule3d::new(0.3, 0.8) -> radius 0.3, segment length 0.8, total
    // height 0.8 + 2*0.3 = 1.4. The bottom is 0.7 below the entity's centre,
    // so float_height must be > 0.7 for tnua to hold the cat off the ground;
    // 1.0 keeps the whole capsule in air with a 0.3 hover gap that absorbs
    // small terrain bumps.
    //
    // The visual kitten (kitten 12, orange-with-frog-hat) is a child rather
    // than living on the Player entity directly so we can offset its feet
    // down by `float_height` — that way the model walks on the ground while
    // the physics capsule hovers. The child still inherits the player's
    // scale, so `pose_player`'s squash/stretch verbs visibly bend the
    // kitten without touching its local transform.
    commands.spawn((
        Player,
        Transform::from_xyz(0.0, 5.0, 0.0),
        Visibility::default(),
        // Physics: dynamic body, capsule collider matching the visual mesh,
        // rotation locked so the cat stays upright (tnua re-orients slowly
        // through `desired_forward` rather than letting torque tumble it).
        RigidBody::Dynamic,
        Collider::capsule_y(0.4, 0.3),
        LockedAxes::ROTATION_LOCKED,
        GravityScale(1.0),
        // Read by `drive_kitten_animation` so the gait clip tracks actual
        // motion, not just input intent — e.g. holding Sprint into a wall
        // would otherwise loop the run cycle while the cat stands still.
        Velocity::default(),
        // Tnua controller + walk config. `step_offset` is generous so the
        // 0.25-stepped chunky terrain reads as a slope, not a wall. Phase 1
        // can tighten this once the terrain mesh is smooth.
        TnuaController::<ControlScheme>::default(),
        TnuaConfig::<ControlScheme>(control_scheme_configs.add(
            ControlSchemeConfig {
                basis: TnuaBuiltinWalkConfig {
                    // `speed` is the m/s the cat reaches when `desired_motion`
                    // is a unit vector. The control system feeds a factor in
                    // [0, 1] (1.0 sprinting, WALK_FACTOR walking), so the cap
                    // here is the run top speed.
                    speed: RUN_SPEED,
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
        Children::spawn(Spawn((
            Name::new("Kitten Visual"),
            SceneRoot(asset_server.load("models/kittens_animated/kitten_12.glb#Scene0")),
            Transform::from_xyz(0.0, -1.0, 0.0)
                .with_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
        ))),
    ));
}

/// Push leafwing's Move axis into the Tnua walk basis. Iso-rotated so
/// "up on stick" walks toward the back of the camera. Default movement is
/// walk; holding Sprint goes to full RUN_SPEED.
fn drive_player(
    action_state: Res<ActionState<Action>>,
    cursor: Res<CursorState>,
    crafting: Res<CraftingState>,
    orbit: Res<CameraOrbit>,
    build_mode: Option<Res<crate::building::BuildMode>>,
    mut query: Query<(&Transform, &mut TnuaController<ControlScheme>), With<Player>>,
) -> Result {
    let (transform, mut controller) = query.single_mut()?;
    controller.initiate_action_feeding();

    let dir2 = if crafting.open {
        Vec2::ZERO
    } else {
        iso_movement(&action_state, &orbit)
    };
    // `desired_motion` is direction-times-factor in [0, 1]; the basis config
    // already owns the m/s cap via `speed` (RUN_SPEED). Sprint goes to 1.0
    // (full run), default uses WALK_FACTOR so walking sits at ~5 m/s.
    let factor = if action_state.pressed(&Action::Sprint) {
        1.0
    } else {
        WALK_FACTOR
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
/// down toward a curl; while pressing C (mark) it stretches up. Eases back
/// when buttons release. Pure visual; reads `CatVerbState` for hold progress
/// so the cat actually moves through the action rather than snapping at the
/// end. Run is conveyed by the Run animation clip itself, not by a pose
/// modifier here.
fn pose_player(
    _action_state: Res<ActionState<Action>>,
    verbs: Res<CatVerbState>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Player>>,
) -> Result {
    let mut transform = query.single_mut()?;

    let nap_amt = verbs.nap_fraction();
    let mark_amt = verbs.mark_fraction();

    let target_y = 1.0 - 0.45 * nap_amt + 0.15 * mark_amt;
    let target_xz = 1.0 + 0.08 * nap_amt;

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

/// The manual Blender export now ships six clips, alphabetised by name:
/// Idle / Walk / Jump / PickUp / Run / Swim (`#Animation0..5`). Sneak
/// still falls back to idle until that clip is added.
fn load_kitten_animations(
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut commands: Commands,
) {
    let path = "models/kittens_animated/kitten_12.glb";
    let idle_clip = asset_server.load::<AnimationClip>(format!("{path}#Animation0"));
    let walk_clip = asset_server.load::<AnimationClip>(format!("{path}#Animation1"));
    let jump_clip = asset_server.load::<AnimationClip>(format!("{path}#Animation2"));
    let pickup_clip = asset_server.load::<AnimationClip>(format!("{path}#Animation3"));
    let run_clip = asset_server.load::<AnimationClip>(format!("{path}#Animation4"));
    let swim_clip = asset_server.load::<AnimationClip>(format!("{path}#Animation5"));
    let (graph, indices) = AnimationGraph::from_clips([
        idle_clip.clone(),
        walk_clip,
        run_clip,
        jump_clip,
        idle_clip,
        pickup_clip,
        swim_clip,
    ]);
    commands.insert_resource(KittenAnimations {
        graph: graphs.add(graph),
        idle: indices[0],
        walk: indices[1],
        run: indices[2],
        jump: indices[3],
        sneak: indices[4],
        pickup: indices[5],
        swim: indices[6],
    });
}

/// Bevy's glTF loader inserts an `AnimationPlayer` deep inside the spawned
/// scene the first time it resolves. Catch that moment and bolt our
/// animation graph + transitions onto the same entity, then start the
/// kitten on Idle so it has a pose before the first input arrives.
fn attach_kitten_animations(
    mut commands: Commands,
    mut new_players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
    anims: Res<KittenAnimations>,
) {
    for (entity, mut player) in &mut new_players {
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut player, anims.idle, Duration::ZERO)
            .repeat();
        commands.entity(entity).insert((
            AnimationGraphHandle(anims.graph.clone()),
            transitions,
        ));
    }
}

/// Pick the kitten animation that best matches the player's current state.
///
/// Priority (top wins): pickup latch (after a `GatherEvent`) → Jump
/// (airborne) → Swim (cat submerged) → Run (speed > RUN_GATE) →
/// Walk (speed > WALK_GATE) → Idle. Driven by measured horizontal
/// velocity, not input intent, so holding Sprint into a wall keeps Idle.
/// Sneak is loaded but unused — reserved for a stealth input.
fn drive_kitten_animation(
    time: Res<Time>,
    anims: Res<KittenAnimations>,
    mut gather_events: MessageReader<crate::gathering::GatherEvent>,
    player_query: Query<
        (&TnuaController<ControlScheme>, &Velocity, &Transform),
        With<Player>,
    >,
    mut anim_query: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut current: Local<Option<AnimationNodeIndex>>,
    mut pickup_remaining: Local<f32>,
) -> Result {
    let _ = anims.sneak; // reserved — wire when stealth input lands.

    let Ok((controller, velocity, transform)) = player_query.single() else {
        return Ok(());
    };

    // Latch the pickup clip for one play-through whenever a GatherEvent
    // fires. The event is one-shot; the timer is what keeps the cat in the
    // pickup pose for the full clip length before falling back to gait.
    if !gather_events.is_empty() {
        gather_events.clear();
        *pickup_remaining = PICKUP_DURATION;
    }
    *pickup_remaining = (*pickup_remaining - time.delta_secs()).max(0.0);
    let picking_up = *pickup_remaining > 0.0;

    let horizontal_speed = Vec3::new(velocity.linvel.x, 0.0, velocity.linvel.z).length();
    let airborne = controller.is_airborne().unwrap_or(false);

    // Cat is "swimming" when its capsule centre dips below the water plane
    // by more than the float height — i.e. the floor it's hovering above
    // is genuinely submerged, not just a wet shoreline tile.
    let water_surface_y = world_water_surface_y();
    let in_water = transform.translation.y < water_surface_y + SWIM_DEPTH_THRESHOLD;

    // Gates pick the gait by actual speed. RUN_GATE sits between WALK peak
    // (5 m/s) and RUN cap (8 m/s); WALK_GATE filters out drift/jitter so a
    // stationary cat doesn't twitch into walk on tiny pushes.
    const WALK_GATE: f32 = 0.4;
    const RUN_GATE: f32 = 6.0;

    let target = if picking_up {
        anims.pickup
    } else if airborne {
        anims.jump
    } else if in_water {
        anims.swim
    } else if horizontal_speed > RUN_GATE {
        anims.run
    } else if horizontal_speed > WALK_GATE {
        anims.walk
    } else {
        anims.idle
    };

    // Tie clip playback rate to the gait so the Mixamo cycle matches our
    // movement speed (otherwise feet slide visibly at 5 m/s walk / 8 m/s run).
    let speed = if target == anims.walk {
        WALK_ANIM_SPEED
    } else if target == anims.run {
        RUN_ANIM_SPEED
    } else if target == anims.pickup {
        PICKUP_ANIM_SPEED
    } else {
        1.0
    };

    for (mut player, mut transitions) in &mut anim_query {
        if *current != Some(target) {
            // Pickup is one-shot; everything else loops.
            let active = transitions.play(&mut player, target, Duration::from_millis(200));
            if target != anims.pickup {
                active.repeat();
            }
        }
        if let Some(active) = player.animation_mut(target) {
            active.set_speed(speed);
        }
    }
    *current = Some(target);
    Ok(())
}

/// Length the pickup clip latches for after a GatherEvent fires. Scaled
/// down to match `PICKUP_ANIM_SPEED` (1.2 s clip ÷ 3× speed = 0.4 s).
const PICKUP_DURATION: f32 = 0.4;
/// How far below the water surface the cat's capsule centre must sit before
/// the swim animation kicks in. Larger = needs to be more submerged. The
/// capsule centre rests `float_height` (1.0) above the seafloor, so on a
/// tile 1.0 below water surface the centre is exactly at the surface; with
/// threshold 0.5 the seafloor needs to be ≥1.5 below water for swim to fire.
const SWIM_DEPTH_THRESHOLD: f32 = 0.5;

/// Match `world::water::water_y` (private). Sea level × half step + bias.
/// Computed inline rather than threading the resource so the animation
/// system stays standalone. If the water module's formula changes, mirror it
/// here.
fn world_water_surface_y() -> f32 {
    use crate::world::biome::SEA_LEVEL;
    use crate::world::terrain::step_height;
    step_height(SEA_LEVEL) * 0.5 - 0.15
}
