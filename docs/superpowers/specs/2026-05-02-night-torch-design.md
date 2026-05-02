# Night Torch — Design

**Date:** 2026-05-02
**Status:** Spec, awaiting implementation plan
**Owner:** nikrich
**ADR:** DEC-025
**Roadmap position:** **Night Torch workstream** (parallel to numbered Phase 0–7 EA roadmap; see `spec/phases/00-index.md` § Parallel workstreams). Not on the EA critical path. Couples with the Worldcraft Expansion workstream (DEC-024) via the shared `DarknessFactor` resource: caves contribute a cave-occupancy term, torch reads the resulting maximum.

## Goal

When the world is dark, the cat holds a lit torch in its right hand that emits warm light into the world. The torch fades in across dusk, sits constant through the night, and fades out across dawn. The same darkness signal is later reused for caves and other dark interiors.

## Scope

### In scope
- Torch GLB attached to the kitten's right-hand bone, riding all skinned animation.
- Real `PointLight` whose intensity tracks a shared `DarknessFactor`.
- Ember particles spawning from the flame tip while the torch is lit.
- Smooth dusk/dawn fade matching the existing sky-color lerp windows.

### Out of scope
- Inventory / craftable torch item — the torch is automatic, not picked up.
- Toggle key — torch on/off is a function of darkness, not input.
- Cave detection — `DarknessFactor` is wired so cave-in-darkness can be ORed in later. DEC-024 (voxel mountain caves spec, same date) plans an "ambient mask" contribution; this resource is the integration point. No cave code exists yet.
- Torch fuel, burnout, lighting other things on fire.
- Hiding the torch while in build mode.
- Per-material emissive scaling — the GLB's baked emissive flame stays constant; only the `PointLight` and ember spawn rate respond to darkness.

## User-facing behavior

| Time of day | Torch state |
|---|---|
| 07:00 – 18:00 | Torch hidden. No light, no embers. |
| 18:00 – 20:00 (dusk) | Torch visible, intensity ramps 0 → 800k, ember rate ramps 0 → 8/s linearly across the window. Flame mesh glows constantly via baked emissive. |
| 20:00 – 05:00 (night) | Torch fully lit. Intensity 800k, embers 8/s. |
| 05:00 – 07:00 (dawn) | Reverse ramp: intensity 800k → 0, embers 8/s → 0. |

The cat's animations (idle/walk/run/jump/swim/pickup) play unchanged; the torch rides the right-hand bone, so it swings naturally with arm motion.

## Architecture

### New module: `src/torch/`

```
src/torch/
  mod.rs   -- TorchPlugin, components, attach + drive systems, ember spawner
```

Plugin registration in `main.rs` between `particles` and `save`.

### Shared resource (in `world::daynight`)

```rust
#[derive(Resource, Default)]
pub struct DarknessFactor(pub f32);  // 0.0 = bright day, 1.0 = full dark
```

Computed every frame by a new `compute_darkness_factor` system in `daynight.rs`. Today reads `WorldTime` only; future cave logic ORs in cave occupancy. All torch systems and any future darkness-gated systems read this single source.

### Components (`torch::mod`)

```rust
#[derive(Component)] struct TorchHolder;       // marker on the right-hand bone
#[derive(Component)] struct Torch;             // marker on the torch SceneRoot
#[derive(Component)] struct TorchLight;        // marker on the PointLight child
#[derive(Component)] struct TorchEmberSource;  // anchor entity at flame tip for ember spawning
```

### Hierarchy after attachment

```
Player
  └── Kitten Visual (SceneRoot)
        └── ... skeleton ...
              └── mixamorig:RightHand  (+ TorchHolder)
                    └── Torch (SceneRoot of torch.glb, with grip Transform)
                          ├── TorchLight (PointLight)
                          └── TorchEmberSource (empty Transform at flame tip)
```

### Systems (all in `Update`)

1. **`compute_darkness_factor`** (in `daynight.rs`) — reads `WorldTime`, writes `DarknessFactor`. Runs alongside the existing `advance_time` / `update_sun` / `update_sky_color` set.
2. **`attach_torch_to_hand`** (in `torch::mod`) — early-outs if a `Torch` entity already exists. Otherwise scans for entities with `Name == "mixamorig:RightHand"` that descend from a `Player`. On first match, inserts `TorchHolder` and spawns the torch hierarchy as a child.
3. **`apply_torch_visibility`** — sets `Torch` visibility to `Hidden` when `DarknessFactor == 0`, `Inherited` otherwise.
4. **`apply_torch_intensity`** — scales `TorchLight`'s `PointLight::intensity` by `DarknessFactor` against a peak constant.
5. **`spawn_torch_embers`** — periodic spawner reading `TorchEmberSource`'s `GlobalTransform`. Skips when `DarknessFactor == 0`. Spawn rate scales with factor.

### Particle module change (`particles::mod`)

- New `ParticleKind::Ember` variant.
- New match arm in the `(velocity, lifetime, mesh, color, emissive)` block of `spawn_particles` — but: ember spawning is *not* driven by the existing biome-based spawner. The torch module owns its own spawner; the variant exists so `update_particles` knows how to animate them.
- New match arm in `update_particles` for `ParticleKind::Ember` — small `t`-driven jitter on x/z, no horizontal sway. Existing tail-fade scaling handles death-shrink.

## Numbers

### Darkness factor

```rust
fn darkness_factor(t: f32) -> f32 {
    if t < 5.0 || t >= 20.0 { 1.0 }
    else if t < 7.0          { 1.0 - (t - 5.0) / 2.0 }
    else if t < 18.0         { 0.0 }
    else                     { (t - 18.0) / 2.0 }
}
```

Windows mirror `update_sky_color`'s dusk (16-20) and dawn (5-7) lerps. Torch fade-in begins at 18:00 (after dusk's mid-point) so it lights up as the sky turns red-to-twilight.

### PointLight

| Field | Value | Reasoning |
|---|---|---|
| `color` | `srgb(1.0, 0.55, 0.20)` | Hotter/redder than the lantern's `(1.0, 0.78, 0.45)` because this is open flame, not enclosed glass. |
| `intensity` | `800_000.0 * factor` | Peak 0.8M — handheld, mobile, shouldn't blow out the scene. Lantern uses 1.5M for comparison. |
| `range` | `6.0` | Smaller than 8m lantern; handheld feels intimate. |
| `shadows_enabled` | `false` | Mobile shadowed lights are expensive; matches existing lantern decision. |
| Local `Transform` | `from_xyz(0.0, 0.15, 0.0)` | The PointLight's local position **relative to the `Torch` entity** (not the bone). Approximate flame-tip offset above the torch origin; tune with `cargo run`. |

### Ember particles

| Field | Value |
|---|---|
| Mesh | `Sphere::new(0.02)` |
| Base color | `srgb(1.0, 0.55, 0.15)` |
| Emissive | `srgb(1.5, 0.6, 0.1)`, `unlit = true` |
| Velocity | `(rng -0.05..0.05, rng 0.3..0.6, rng -0.05..0.05)` |
| Lifetime | `rng 0.5..1.0` seconds |
| Spawn rate | 8 / second at `factor == 1.0`, scaled linearly. Cap at the existing `MAX_PARTICLES` global. |
| Spawn position | `TorchEmberSource` `GlobalTransform::translation` + tiny `(rng -0.02..0.02)` jitter on x/z. |
| Update behaviour | Inherits `update_particles` movement; ember arm adds `(t * 5.0).sin() * 0.05 * dt` jitter to x and z. No horizontal sway. |

### Grip transform (placeholder, requires visual tune)

```rust
const TORCH_GRIP: Transform = Transform {
    translation: Vec3::new(0.0, 0.05, 0.0),
    rotation: Quat::IDENTITY,
    scale: Vec3::ONE,
};
```

Mixamo right-hand bone is oriented along the wrist axis. Expect a ~90° rotation correction after first run. Marked `// TUNE` in code.

## Asset

`assets/models/torch/torch.glb` — single mesh node "Cube", four materials. `Material.003` and `Material.004` carry baked emissive flame (`emissiveStrength` 1.9 and 1.5 respectively). Loaded as `SceneRoot(asset_server.load("models/torch/torch.glb#Scene0"))`.

## Edge cases

- **Async scene load.** `Added<Name>` queries fire whenever the GLB scene resolves, so `attach_torch_to_hand` works whether the kitten loads in frame 1 or frame 30. Early-out on `Torch` existence prevents double-attach if the system runs across multiple frames before the bone appears.
- **Player respawn / hot reload.** Old torch despawns with the parent kitten. New scene's bones generate new `Added<Name>` events, attach re-fires.
- **Animation hides the hand off-screen.** Torch is a child of the bone — it follows. No special case.
- **Build mode.** The cat faces the cursor; torch rides the body. No interaction.
- **Save/load.** Torch is purely a function of `WorldTime`. Nothing to persist.
- **Time skip / load at night.** First frame with `DarknessFactor > 0` flips visibility on; if the torch hasn't attached yet (scene still loading) it pops in when the bone appears. Acceptable — same one-frame pop the cat itself has.

## Files touched

| Path | Action | LOC est. |
|---|---|---|
| `src/torch/mod.rs` | New | ~150 |
| `src/main.rs` | Add `mod torch;` and `TorchPlugin` to the second `add_plugins` block | +2 |
| `src/world/daynight.rs` | Add `DarknessFactor` resource, `darkness_factor` helper, `compute_darkness_factor` system, register both | +25 |
| `src/particles/mod.rs` | Add `ParticleKind::Ember` variant + `update_particles` match arm | +20 |

## Risks / known unknowns

- **Mixamo bone-name coupling.** `mixamorig:RightHand` is hard-coded. The codebase already pays this cost for animations, so the blast radius is shared. Documented in the module doc comment.
- **Grip transform requires iteration.** First run will look wrong; tuning is a normal part of the pass.
- **Emissive on the flame is constant during dusk.** Reads as "flame is always there, the world responds to it at night." If this looks bad, follow-up work can scale emissive too — out of scope for v1.
- **Embers are a separate spawner from the biome-driven one** but share the `MAX_PARTICLES` cap. At 8/s ember rate, embers can exhaust the cap in a heavy biome (fireflies + embers). Acceptable for now; if it bites, raise the cap or split caps per kind.

## Future hooks

- **Caves (DEC-024).** When the voxel cave system lands, extend `compute_darkness_factor` to take the maximum of the night-driven value and the cave system's ambient mask. Single change site, no torch-side changes.
- **Different torches.** Replace the GLB path with a value sourced from the inventory / equipped item.
- **Toggle.** Add an `Action::ToggleTorch` and AND its state into `apply_torch_visibility` + `apply_torch_intensity`.
- **Particle pool exhaustion.** If embers + biome particles starve each other, give embers their own cap.
