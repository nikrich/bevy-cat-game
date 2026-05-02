# Night Torch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When `WorldTime` says it's dark, the cat holds a lit torch in its right hand. The torch fades in across dusk, sits constant through the night, fades out across dawn, casts warm `PointLight`, and emits ember particles from its flame tip.

**Architecture:** A new `DarknessFactor` resource (computed in `daynight.rs`) drives a new `src/torch/mod.rs` module. The torch attaches itself once as a child of the kitten's `mixamorig:RightHand` bone via an `Added<Name>` query, then per-frame systems update its `Visibility`, `PointLight::intensity`, and ember spawn rate from `DarknessFactor`. Embers ride the existing `particles` module via a new `ParticleKind::Ember` variant + a `pub fn spawn_ember(...)` helper.

**Tech Stack:** Bevy 0.18, no new dependencies. Asset: `assets/models/torch/torch.glb` (already on disk).

**Reference:** `docs/superpowers/specs/2026-05-02-night-torch-design.md` (DEC-025).

---

## Testing approach

Project is bin-only (`src/main.rs`, no `src/lib.rs`). Per the existing `2026-05-02-voxel-storage-substrate.md` plan and `src/decoration/physics.rs`:

- **Pure functions** (`darkness_factor`) get unit tests in `#[cfg(test)] mod tests` blocks. Run with `cargo test`.
- **Plugin / system / lifecycle work** is verified by `cargo check` (compile clean) + a manual playtest checkpoint at the end (run, scrub time forward to night, verify torch appears + lights surroundings + spawns embers).

`cargo test` runs the binary's inline test modules.

---

## File structure (target)

```
src/
  torch/
    mod.rs         # NEW — TorchPlugin, components, attach + drive systems, ember spawner
  particles/
    mod.rs         # MODIFIED — ParticleKind::Ember variant, update_particles arm, pub spawn_ember helper
  world/
    daynight.rs    # MODIFIED — DarknessFactor resource, darkness_factor pure fn + tests, compute_darkness_factor system
    mod.rs         # MODIFIED — register DarknessFactor + compute_darkness_factor
  main.rs          # MODIFIED — register TorchPlugin
```

`torch::mod` is `pub mod torch;` from `main.rs`. `TorchPlugin` is the only public export.

---

## Task 1: Pure `darkness_factor` function with tests

**Files:**
- Modify: `src/world/daynight.rs` (append at end of file)

- [ ] **Step 1: Add the failing tests first**

Open `src/world/daynight.rs`. Append at the very bottom:

```rust
/// Maps `time_of_day` (hours, 0.0..24.0) to a darkness factor in [0.0, 1.0].
///
/// 0.0 means full daylight (no torch); 1.0 means full night. The dusk and
/// dawn windows linearly ramp the factor so the torch fades in/out instead
/// of popping. Windows mirror `update_sky_color`'s 18-20h dusk and 5-7h
/// dawn lerps so the torch lights up exactly as the sky reddens.
///
/// Cave/dark-interior contributions will be folded in by ORing (taking max
/// of) this value with a cave-occupancy term in `compute_darkness_factor`.
/// Per DEC-024, no cave code exists yet.
pub fn darkness_factor(t: f32) -> f32 {
    if t < 5.0 || t >= 20.0 {
        1.0
    } else if t < 7.0 {
        1.0 - (t - 5.0) / 2.0
    } else if t < 18.0 {
        0.0
    } else {
        (t - 18.0) / 2.0
    }
}

#[cfg(test)]
mod darkness_tests {
    use super::*;

    #[test]
    fn full_night_at_midnight() {
        assert_eq!(darkness_factor(0.0), 1.0);
        assert_eq!(darkness_factor(2.5), 1.0);
        assert_eq!(darkness_factor(4.999), 1.0);
    }

    #[test]
    fn full_night_after_twenty() {
        assert_eq!(darkness_factor(20.0), 1.0);
        assert_eq!(darkness_factor(22.5), 1.0);
        assert_eq!(darkness_factor(23.999), 1.0);
    }

    #[test]
    fn dawn_ramps_one_to_zero() {
        assert!((darkness_factor(5.0) - 1.0).abs() < 1e-5);
        assert!((darkness_factor(6.0) - 0.5).abs() < 1e-5);
        assert!((darkness_factor(7.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn dusk_ramps_zero_to_one() {
        assert!((darkness_factor(18.0) - 0.0).abs() < 1e-5);
        assert!((darkness_factor(19.0) - 0.5).abs() < 1e-5);
        assert!((darkness_factor(19.999) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn full_day_between_seven_and_eighteen() {
        assert_eq!(darkness_factor(7.001), 0.0);
        assert_eq!(darkness_factor(12.0), 0.0);
        assert_eq!(darkness_factor(17.999), 0.0);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test darkness_tests`
Expected: 5 tests pass, 0 failed.

- [ ] **Step 3: Commit**

```bash
git add src/world/daynight.rs
git commit -m "feat(daynight): add darkness_factor pure function

Maps time of day to a [0, 1] factor with dusk (18-20h) and dawn (5-7h)
linear ramps. Cave-occupancy term will be ORed in later (DEC-024).
"
```

---

## Task 2: `DarknessFactor` resource + `compute_darkness_factor` system

**Files:**
- Modify: `src/world/daynight.rs` (add resource + system)
- Modify: `src/world/mod.rs:17-26` (register resource + system)

- [ ] **Step 1: Add the resource type and system to `daynight.rs`**

Open `src/world/daynight.rs`. Just below the existing `WorldTime` struct + `Default` impl (around line 20), insert:

```rust
/// Shared "is the world dark for the player" signal. Driven by `WorldTime`
/// today; future cave logic will fold in a cave-occupancy term via
/// `compute_darkness_factor`.
#[derive(Resource, Default)]
pub struct DarknessFactor(pub f32);
```

Then add the system, anywhere after `darkness_factor` from Task 1 (e.g. just above the `darkness_tests` mod):

```rust
pub fn compute_darkness_factor(
    world_time: Res<WorldTime>,
    mut darkness: ResMut<DarknessFactor>,
) {
    darkness.0 = darkness_factor(world_time.time_of_day);
}
```

- [ ] **Step 2: Register resource + system in `world/mod.rs`**

Open `src/world/mod.rs`. In the `WorldPlugin::build` body, add `init_resource` after the existing `WorldTime` registration (line 24), and add the system to the `Update` tuple alongside the other `daynight::` systems (lines 52-55):

```rust
            .init_resource::<daynight::WorldTime>()
            .init_resource::<daynight::DarknessFactor>()
            .add_message::<chunks::ChunkLoaded>()
```

```rust
                    daynight::advance_time,
                    daynight::compute_darkness_factor,
                    daynight::update_sun,
                    daynight::update_sky_color,
                    daynight::update_ambient_light,
```

- [ ] **Step 3: Run `cargo check`**

Run: `cargo check`
Expected: clean compile, no errors.

- [ ] **Step 4: Commit**

```bash
git add src/world/daynight.rs src/world/mod.rs
git commit -m "feat(daynight): add DarknessFactor resource and per-frame computation

Single source of truth for 'how dark is it for the player'. Today
reads only WorldTime; cave occupancy will be ORed in later (DEC-024).
"
```

---

## Task 3: `ParticleKind::Ember` variant + public `spawn_ember` helper

**Files:**
- Modify: `src/particles/mod.rs` (add variant, update arm, public spawn helper)

- [ ] **Step 1: Add the `Ember` variant**

Open `src/particles/mod.rs`. Update the enum (around line 25-31):

```rust
#[derive(Clone, Copy)]
enum ParticleKind {
    Leaf,
    Firefly,
    Snowflake,
    SandWisp,
    Pollen,
    Ember,
}
```

- [ ] **Step 2: Add the `update_particles` match arm**

In `update_particles` (around line 198-227), add a new match arm before the closing brace of the per-type behavior `match`. Place it after `ParticleKind::Pollen`:

```rust
            ParticleKind::Ember => {
                // Tiny lateral jitter so embers shimmer instead of rising
                // in a perfect line. Velocity already carries them up.
                transform.translation.x += (t * 5.0 + transform.translation.z).sin() * 0.05 * dt;
                transform.translation.z += (t * 5.0 + transform.translation.x).cos() * 0.05 * dt;
            }
```

- [ ] **Step 3: Add the public `spawn_ember` helper**

At the end of `src/particles/mod.rs` (after `update_particles`), add:

```rust
/// Spawn a single ember at `position`. Used by the torch module's
/// flame-tip spawner — the existing biome-driven `spawn_particles`
/// system does not produce embers.
pub fn spawn_ember(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    position: Vec3,
) {
    let mut rng = rand::thread_rng();
    let velocity = Vec3::new(
        rng.gen_range(-0.05..0.05_f32),
        rng.gen_range(0.3..0.6_f32),
        rng.gen_range(-0.05..0.05_f32),
    );
    let lifetime = rng.gen_range(0.5..1.0_f32);
    let jitter = Vec3::new(
        rng.gen_range(-0.02..0.02_f32),
        0.0,
        rng.gen_range(-0.02..0.02_f32),
    );

    let mesh = meshes.add(Sphere::new(0.02));
    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.55, 0.15),
        emissive: Color::srgb(1.5, 0.6, 0.1).into(),
        unlit: true,
        ..default()
    });

    commands.spawn((
        Particle {
            velocity,
            lifetime,
            age: 0.0,
            kind: ParticleKind::Ember,
        },
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_translation(position + jitter),
    ));
}
```

- [ ] **Step 4: Run `cargo check`**

Run: `cargo check`
Expected: clean compile.

- [ ] **Step 5: Commit**

```bash
git add src/particles/mod.rs
git commit -m "feat(particles): add Ember kind and public spawn_ember helper

Variant + update arm for upward-drifting embers with lateral shimmer.
Public helper lets the torch module spawn from the flame tip without
being driven by the biome-based spawn_particles system.
"
```

---

## Task 4: Torch module scaffold (plugin + components, no logic)

**Files:**
- Create: `src/torch/mod.rs`
- Modify: `src/main.rs:1-17` (mod declaration), `src/main.rs:46-65` (plugin registration)

- [ ] **Step 1: Create the new module file**

Create `src/torch/mod.rs`:

```rust
//! Night torch (DEC-025). The kitten holds a torch in its right hand
//! whenever the world is dark. Visibility, point-light intensity, and
//! ember spawn rate all track the shared `DarknessFactor` resource.
//!
//! The torch attaches itself once to `mixamorig:RightHand` via an
//! `Added<Name>` query — same Mixamo-name coupling we already pay for
//! animations. Per DEC-024 the cave system will contribute to
//! `DarknessFactor` later, no torch-side changes needed.

use bevy::prelude::*;

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
const TORCH_GRIP_TRANSLATION: Vec3 = Vec3::new(0.0, 0.05, 0.0);

/// Peak `PointLight::intensity` at full darkness. Scaled linearly by
/// `DarknessFactor`. Smaller than the lantern's 1.5M because handheld
/// open flame shouldn't blow out the surrounding scene.
const TORCH_LIGHT_PEAK_INTENSITY: f32 = 800_000.0;

/// Embers per second at full darkness. Scaled linearly by
/// `DarknessFactor` so they ramp in across dusk.
const EMBER_RATE_PER_SEC: f32 = 8.0;

// Stub systems — implementations land in tasks 5-7.

fn attach_torch_to_hand() {}
fn apply_torch_visibility() {}
fn apply_torch_intensity() {}
fn spawn_torch_embers() {}
```

- [ ] **Step 2: Register the module in `main.rs`**

Open `src/main.rs`. Add `mod torch;` to the module list (between `mod state;` and `mod ui;`, to keep alphabetical-ish ordering — match existing style):

```rust
mod animals;
mod building;
mod camera;
mod crafting;
mod decoration;
mod edit;
mod gathering;
mod input;
mod inventory;
mod items;
mod memory;
mod particles;
mod player;
mod save;
mod state;
mod torch;
mod ui;
mod world;
```

Then add `TorchPlugin` to the second `add_plugins` block (after `decoration::DecorationPlugin`):

```rust
        .add_plugins(ui::GameUiPlugin)
        .add_plugins(decoration::DecorationPlugin)
        .add_plugins(torch::TorchPlugin)
        .run();
```

- [ ] **Step 3: Run `cargo check`**

Run: `cargo check`
Expected: clean compile (the stubbed systems are valid empty fns).

- [ ] **Step 4: Commit**

```bash
git add src/torch/mod.rs src/main.rs
git commit -m "feat(torch): scaffold TorchPlugin with markers and stubs

Plugin, marker components (TorchHolder, Torch, TorchLight,
TorchEmberSource), tuning constants. System bodies stubbed; logic
lands in subsequent commits.
"
```

---

## Task 5: `attach_torch_to_hand` — find the bone, spawn the torch hierarchy

**Files:**
- Modify: `src/torch/mod.rs` (replace `attach_torch_to_hand` stub)

- [ ] **Step 1: Replace the stub with the real implementation**

Open `src/torch/mod.rs`. Add `use bevy::scene::SceneRoot;` is already pulled by `bevy::prelude::*`. Add `use crate::player::Player;` to the imports near the top:

```rust
use bevy::prelude::*;

use crate::player::Player;
use crate::world::daynight::DarknessFactor;
```

Replace the `attach_torch_to_hand` stub (a few lines from the bottom of the file) with:

```rust
/// Find the kitten's `mixamorig:RightHand` bone the moment its `Name`
/// component is inserted (Bevy's glTF loader does this when the scene
/// resolves), then spawn the torch as a child. Early-out once a `Torch`
/// exists so this is effectively a one-shot lookup.
///
/// The Mixamo name coupling is the same one the animation system already
/// pays — see `player::attach_kitten_animations`. If the rig ever swaps
/// off Mixamo, both this and the animations break together.
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
                    Transform::from_translation(TORCH_GRIP_TRANSLATION),
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
                        // Local position relative to the Torch entity —
                        // approximate flame-tip offset above the torch
                        // origin. // TUNE
                        Transform::from_xyz(0.0, 0.15, 0.0),
                    ));
                    torch.spawn((
                        TorchEmberSource,
                        Name::new("TorchEmberSource"),
                        // Slightly above the light so embers spawn at the
                        // visible flame tip, not the wick. // TUNE
                        Transform::from_xyz(0.0, 0.30, 0.0),
                        GlobalTransform::default(),
                    ));
                });
            });

        // We attached — stop scanning this frame.
        break;
    }
}
```

- [ ] **Step 2: Run `cargo check`**

Run: `cargo check`
Expected: clean compile.

- [ ] **Step 3: Commit**

```bash
git add src/torch/mod.rs
git commit -m "feat(torch): attach torch hierarchy to right-hand bone

Added<Name> scan finds mixamorig:RightHand on the first frame the
glTF loader resolves the kitten skeleton; spawns torch SceneRoot +
PointLight + ember source as children. One-shot via Torch existence
guard. Light intensity defaults to 0; apply_torch_intensity drives
it from DarknessFactor.
"
```

---

## Task 6: `apply_torch_visibility` + `apply_torch_intensity` — drive visuals from `DarknessFactor`

**Files:**
- Modify: `src/torch/mod.rs` (replace the two stubs)

- [ ] **Step 1: Replace `apply_torch_visibility`**

Open `src/torch/mod.rs`. Replace the `apply_torch_visibility` stub:

```rust
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
```

- [ ] **Step 2: Replace `apply_torch_intensity`**

Replace the `apply_torch_intensity` stub:

```rust
/// Scale the torch's `PointLight::intensity` linearly with
/// `DarknessFactor`. Skips the write when the factor is zero — the
/// light is invisible anyway because `apply_torch_visibility` hid the
/// whole hierarchy, but keeping intensity at zero matches what the user
/// would see if visibility were toggled off independently.
fn apply_torch_intensity(
    darkness: Res<DarknessFactor>,
    mut lights: Query<&mut PointLight, With<TorchLight>>,
) {
    let intensity = TORCH_LIGHT_PEAK_INTENSITY * darkness.0.clamp(0.0, 1.0);
    for mut light in &mut lights {
        light.intensity = intensity;
    }
}
```

- [ ] **Step 3: Run `cargo check`**

Run: `cargo check`
Expected: clean compile.

- [ ] **Step 4: Commit**

```bash
git add src/torch/mod.rs
git commit -m "feat(torch): drive visibility and light intensity from DarknessFactor

Visibility hides the whole torch in daylight; PointLight intensity
ramps linearly across dusk/dawn windows alongside the sky-color lerps.
"
```

---

## Task 7: `spawn_torch_embers` — periodic flame-tip ember spawner

**Files:**
- Modify: `src/torch/mod.rs` (replace `spawn_torch_embers` stub)

- [ ] **Step 1: Replace the stub**

Open `src/torch/mod.rs`. Replace the `spawn_torch_embers` stub:

```rust
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
            crate::particles::spawn_ember(
                &mut commands,
                meshes.as_mut(),
                materials.as_mut(),
                source_transform.translation(),
            );
        }
    }
}
```

- [ ] **Step 2: Run `cargo check`**

Run: `cargo check`
Expected: clean compile.

- [ ] **Step 3: Run all tests**

Run: `cargo test`
Expected: all tests (including the existing decoration tests + the new `darkness_tests`) pass.

- [ ] **Step 4: Commit**

```bash
git add src/torch/mod.rs
git commit -m "feat(torch): spawn embers at flame tip while torch burns

Periodic spawner reads TorchEmberSource GlobalTransform so embers
emit from the animated bone's flame tip. Rate scales with
DarknessFactor; accumulator-driven so frame-rate doesn't change
ember density.
"
```

---

## Task 8: Manual playtest checkpoint

**No code changes — this is a runtime validation step.**

- [ ] **Step 1: Run the game**

Run: `cargo run`
Wait for the world to load.

- [ ] **Step 2: Check daytime baseline**

In the default morning (`time_of_day = 8.0`):
- The cat spawns and walks normally.
- **No torch is visible** in the right hand.
- No warm point-light is illuminating the cat.
- No embers in the air around the cat.

If you see a torch during the day, `apply_torch_visibility` is wrong — re-check Task 6.

- [ ] **Step 3: Scrub time forward to night**

Open the egui edit panel (or whatever in-game time control exists — check `src/world/edit_egui.rs`). If no time control exists, temporarily change `WorldTime::default` in `src/world/daynight.rs:14` from `time_of_day: 8.0` to `time_of_day: 22.0` and re-run.

At full night (any time in `[20.0, 24.0)` or `[0.0, 5.0)`):
- The torch is visible in the cat's right hand.
- A warm orange `PointLight` illuminates terrain/props within ~6m of the cat.
- Small glowing embers stream upward from the flame tip a few times per second.
- Walking, running, jumping — all kitten animations play normally; the torch swings with the right arm.

If the torch position looks wrong (handle through palm, torch upside down, sticking sideways): tune `TORCH_GRIP_TRANSLATION` and the rotation. Add a `Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)` (or similar) to the `Transform::from_translation` call in `attach_torch_to_hand` (Task 5) and re-run. Iterate until it reads as a hand-held torch.

- [ ] **Step 4: Verify dusk/dawn fade**

Set `time_of_day` to `19.0` (mid-dusk). Expected:
- Torch visible.
- Light intensity is roughly half-strength (visibly dimmer than full night).
- Embers spawn at roughly half rate.

Set `time_of_day` to `6.0` (mid-dawn). Same expected behavior, fading the other way.

Set `time_of_day` to `7.001` and `17.999`. Torch should be hidden at both (factor == 0).

- [ ] **Step 5: Restore the default time-of-day if you changed it**

If you edited `WorldTime::default` for testing, change it back to `time_of_day: 8.0`.

- [ ] **Step 6: Run all tests one more time**

Run: `cargo test`
Expected: pass.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings (or only warnings already present on `main`).

- [ ] **Step 8: Final commit (only if Step 3 grip-tuning made changes)**

If you tuned `TORCH_GRIP_TRANSLATION` or added a rotation:

```bash
git add src/torch/mod.rs
git commit -m "feat(torch): tune grip transform from playtest

Adjust translation/rotation so torch reads as hand-held in the
kitten's right palm during all gait cycles.
"
```

If no tuning was needed, no final commit.

---

## Self-Review

**Spec coverage:**
- "Torch GLB attached to right-hand bone" → Task 5.
- "PointLight whose intensity tracks DarknessFactor" → Tasks 1, 2, 5, 6.
- "Ember particles from the flame tip" → Tasks 3, 5, 7.
- "Smooth dusk/dawn fade matching sky-color lerps" → Task 1 (windows mirror `update_sky_color`).
- "DarknessFactor as integration point for DEC-024 cave ambient mask" → Task 2 (resource), referenced in module doc comment in Task 4.
- "All animations play unchanged; torch rides the bone" → falls out of Task 5's parent-child setup; verified in Task 8.
- "Mixamo bone-name coupling documented" → Task 4 (module doc) and Task 5 (system doc).
- Out-of-scope items (inventory, toggle key, build-mode hide, fuel, per-material emissive scaling) are intentionally absent.

**Placeholder scan:** Comments contain `// TUNE` markers for the grip translation and the flame-tip offset. These are intentional iteration knobs flagged in the spec, not unwritten work. No "TBD" / "implement later" / "similar to Task N" / hand-wave references.

**Type consistency:** `DarknessFactor(pub f32)` is declared in Task 2 and read as `darkness.0` in Tasks 6 and 7 (matching). `TorchHolder`, `Torch`, `TorchLight`, `TorchEmberSource` are declared in Task 4 and used by name in Tasks 5-7. `TORCH_GRIP_TRANSLATION`, `TORCH_LIGHT_PEAK_INTENSITY`, `EMBER_RATE_PER_SEC` declared in Task 4, used in Tasks 5-7. `spawn_ember` signature in Task 3 (`commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, position: Vec3`) matches the call site in Task 7.
