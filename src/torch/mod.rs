//! Night torch (DEC-025). The kitten holds a torch in its right hand
//! whenever the world is dark. Visibility, point-light intensity, and
//! ember spawn rate all track the shared `DarknessFactor` resource.
//!
//! The torch attaches itself once to `mixamorig:RightHand` via an
//! `Added<Name>` query — same Mixamo-name coupling we already pay for
//! animations. Per DEC-024 the cave system will contribute to
//! `DarknessFactor` later, no torch-side changes needed.

use bevy::prelude::*;

use crate::player::Player;
use crate::world::daynight::DarknessFactor;

pub struct TorchPlugin;

impl Plugin for TorchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                attach_torch_to_hand,
                apply_torch_visibility,
                apply_torch_intensity,
                spawn_torch_embers,
            ),
        );
    }
}

/// Marker on the `mixamorig:RightHand` bone once the torch has been
/// parented to it. Prevents `attach_torch_to_hand` from re-attaching.
#[derive(Component)]
struct TorchHolder;

/// Marker on the torch `SceneRoot` entity. One per game session.
#[derive(Component)]
struct Torch;

/// Marker on the `PointLight` child of `Torch`.
#[derive(Component)]
struct TorchLight;

/// Marker on the empty entity positioned at the flame tip; its
/// `GlobalTransform::translation` is read by `spawn_torch_embers`.
#[derive(Component)]
struct TorchEmberSource;

/// Local transform of the torch entity relative to the right-hand bone.
/// `// TUNE` — Mixamo right-hand bone is wrist-aligned; expect to rotate
/// roughly 90° around X to make the handle stand upright in the palm,
/// then nudge the translation. Iterate with `cargo run`.
const TORCH_GRIP: Transform = Transform {
    translation: Vec3::new(0.0, 0.05, 0.0),
    rotation: Quat::IDENTITY,
    scale: Vec3::ONE,
};

/// Peak `PointLight::intensity` at full darkness. Scaled linearly by
/// `DarknessFactor`. Smaller than the lantern's 1.5M because handheld
/// open flame shouldn't blow out the surrounding scene.
const TORCH_LIGHT_PEAK_INTENSITY: f32 = 800_000.0;

/// Embers per second at full darkness. Scaled linearly by
/// `DarknessFactor` so they ramp in across dusk.
const EMBER_RATE_PER_SEC: f32 = 8.0;

// Stub systems — implementations land in tasks 5-7.

/// Find the kitten's `mixamorig:RightHand` bone the moment its `Name`
/// component is inserted (Bevy's glTF loader does this when the scene
/// resolves), then spawn the torch as a child. Early-out once a `Torch`
/// exists so this is effectively a one-shot lookup.
///
/// The Mixamo name coupling is the same one the animation system already
/// pays -- see `player::attach_kitten_animations`. If the rig ever swaps
/// off Mixamo, both this and the animations break together.
///
/// Known limitation: if the player ever respawns mid-frame, the old
/// `Torch`'s deferred despawn and the new bone's `Added<Name>` event can
/// overlap, causing the attach to skip and the new kitten to spawn
/// torchless. Today the cat never dies, so this is not exercised. If
/// respawn ever lands, swap the guard for a per-bone `Without<TorchHolder>`
/// filter on `new_names` and key existence off the bone, not the torch.
fn attach_torch_to_hand(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    new_names: Query<(Entity, &Name), Added<Name>>,
    existing_torch: Query<(), With<Torch>>,
    players: Query<(), With<Player>>,
) {
    if !existing_torch.is_empty() || players.is_empty() {
        return;
    }

    for (entity, name) in &new_names {
        if name.as_str() != "mixamorig:RightHand" {
            continue;
        }

        commands
            .entity(entity)
            .insert(TorchHolder)
            .with_children(|hand| {
                hand.spawn((
                    Torch,
                    Name::new("Torch"),
                    SceneRoot(asset_server.load("models/torch/torch.glb#Scene0")),
                    TORCH_GRIP,
                    Visibility::default(),
                ))
                .with_children(|torch| {
                    torch.spawn((
                        TorchLight,
                        Name::new("TorchLight"),
                        PointLight {
                            color: Color::srgb(1.0, 0.55, 0.20),
                            intensity: 0.0, // driven by apply_torch_intensity
                            range: 6.0,
                            shadows_enabled: false,
                            ..default()
                        },
                        // Local position relative to the Torch entity --
                        // approximate flame-tip offset above the torch
                        // origin. // TUNE
                        Transform::from_xyz(0.0, 0.15, 0.0),
                        Visibility::default(),
                    ));
                    torch.spawn((
                        TorchEmberSource,
                        Name::new("TorchEmberSource"),
                        // Slightly above the light so embers spawn at the
                        // visible flame tip, not the wick. // TUNE
                        Transform::from_xyz(0.0, 0.30, 0.0),
                    ));
                });
            });

        // We attached -- stop scanning this frame.
        break;
    }
}
/// Hide the entire torch hierarchy at full daylight; show it whenever
/// `DarknessFactor > 0`. `Visibility::Inherited` lets the bone's own
/// inherited visibility still apply (e.g. if the kitten visual is ever
/// hidden as a whole).
fn apply_torch_visibility(
    darkness: Res<DarknessFactor>,
    mut torches: Query<&mut Visibility, With<Torch>>,
) {
    let want_hidden = darkness.0 <= 0.0;
    for mut visibility in &mut torches {
        let target = if want_hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != target {
            *visibility = target;
        }
    }
}
/// Scale the torch's `PointLight::intensity` linearly with
/// `DarknessFactor`. Writes every frame -- at most one entity, so the
/// cost is negligible. `clamp` guards against a future cave-occupancy
/// term (DEC-024) pushing the factor above 1.0.
fn apply_torch_intensity(
    darkness: Res<DarknessFactor>,
    mut lights: Query<&mut PointLight, With<TorchLight>>,
) {
    let intensity = TORCH_LIGHT_PEAK_INTENSITY * darkness.0.clamp(0.0, 1.0);
    for mut light in &mut lights {
        light.intensity = intensity;
    }
}
/// Spawn embers at the torch's flame tip while it's burning. Rate scales
/// with `DarknessFactor` so the ramp matches the light fade. Reads
/// `GlobalTransform` so the bone's animated motion (idle bob, run-cycle
/// arm swing) carries the spawn point naturally.
fn spawn_torch_embers(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    darkness: Res<DarknessFactor>,
    time: Res<Time>,
    sources: Query<&GlobalTransform, With<TorchEmberSource>>,
    particles: Query<(), With<crate::particles::Particle>>,
    mut accumulator: Local<f32>,
) {
    let factor = darkness.0.clamp(0.0, 1.0);
    if factor <= 0.0 {
        *accumulator = 0.0;
        return;
    }

    let rate = EMBER_RATE_PER_SEC * factor;
    *accumulator += rate * time.delta_secs();

    while *accumulator >= 1.0 {
        *accumulator -= 1.0;
        for source_transform in &sources {
            // Re-sample the live count inside the source loop so a
            // multi-source future doesn't silently over-spawn past the
            // cap on a single accumulator tick. Today there's only one
            // source so this is a no-op.
            let particle_count = particles.iter().count();
            crate::particles::spawn_ember(
                &mut commands,
                meshes.as_mut(),
                materials.as_mut(),
                source_transform.translation(),
                particle_count,
            );
        }
    }
}
