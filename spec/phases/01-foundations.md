# Phase 0 — Foundations & Engine Upgrade

> Status: Planned
> Depends on: —
> Exit criteria: Game runs on Bevy 0.18 with the spec's full crate stack, all current gameplay (terrain, props, gathering, crafting, building placeholders, animals, day/night, save) works through the new abstractions, no regressions.

## Goal

Pay the infrastructure debt now so every subsequent phase ships against a stable foundation. Upgrade Bevy, replace home-grown abstractions with the ecosystem crates the spec mandates, introduce game states, and move save to a path that supports Steam Cloud.

## Why now

Every later phase touches input, UI, save, or physics. Refactoring the foundations after building features on top of them is more painful than doing it before. Per user direction: "upgrade now to avoid headaches in the future."

## Deliverables

- Bevy 0.18+ on stable Rust
- `bevy-rapier3d` plugin loaded, no character controller yet
- `bevy_tnua` driving player movement (replaces capsule transform manipulation)
- `leafwing-input-manager` replacing `input::GameInput`
- `bevy_egui` available; pilot port: crafting menu and any new UI starts in egui
- Bevy 0.18's built-in procedural atmosphere driving the sky; `daynight.rs` keeps the time-of-day logic but stops directly mutating sky color
- `bevy_asset_loader` driving a Loading state
- `moonshine-save` replacing the current serde JSON save
- Game state machine: `Loading` → `MainMenu` → `Playing` → `Paused` → `Building` (Building is a sub-state of Playing)
- Day length set to 24 real minutes
- Save path uses an OS data dir compatible with Steam Cloud's writeable user data location

## Decisions to record

- DEC-013 — Bevy 0.18+ with the ecosystem crate stack: rapier, tnua, leafwing, egui, asset_loader, moonshine-save. Sky/atmosphere uses Bevy 0.18's first-party procedural atmosphere (drops `bevy_atmosphere`, which is stuck on Bevy 0.16 as of the 2026-04-29 crate audit). Navmesh and utility AI choices defer to Phases 1 and 5 respectively, where their replacements ship.
- DEC-014 — Game state machine adopted; `Playing` is the gameplay parent state, `Building` is a sub-state
- DEC-015 — Save format moves to moonshine-save reflection serialization; on-disk format remains JSON for human-readability
- DEC-016 — Day cycle bumped from 12 min → 24 min real time per design call

## Tech debt closed

- DEBT-003, DEBT-011 — game states
- DEBT-004 — hardcoded world seed (replaced with seed in save / new-world dialog)
- DEBT-009 — already closed, now further reinforced
- DEBT-012 — WorldNoise reconstructed every frame (cached as resource as part of the migration)

## Work breakdown

### W0.1 — Bump Bevy 0.16 → 0.18, fix breaking changes

**What:** Update `Cargo.toml`, run `cargo check`, fix breaking API changes phase by phase. Common breaks: `Query::single` returns `Result`, scheduling APIs, asset handle changes.
**Acceptance:** `cargo build` clean, `cargo clippy -- -D warnings` clean, game launches into existing scene with no behavioral regression.

### W0.2 — Cached `WorldNoise` resource

**What:** Build `WorldNoise` once at startup, store in `Resource`. Remove all `WorldNoise::new(seed)` calls in per-frame systems (`player`, `animals`, `biome`).
**Acceptance:** `grep "WorldNoise::new"` shows one constructor call site. Frame time on `cargo run --release` improves measurably (record before/after).
**Closes:** DEBT-012

### W0.3 — Adopt `bevy-rapier3d` (physics scaffold)

**What:** Add `bevy_rapier3d` with default plugin. Add ground colliders to terrain tiles (or terrain mesh once Phase 1 lands — for now, attach `Collider::cuboid` per tile). Add capsule collider + dynamic body to player.
**Acceptance:** Player falls under gravity onto terrain, can walk on it without clipping. Colliders visualize correctly with rapier debug render toggle.

### W0.4 — Adopt `bevy_tnua` for player movement

**What:** Replace direct `Transform` manipulation in `player::move_player` with a Tnua character controller. Movement reads from new input action set (W0.5) — a temporary `GameInput` shim is acceptable mid-migration.
**Acceptance:** Player walks, sprints, jumps, lands on terrain. Slope traversal works up to ~30°. No teleport-snapping artifacts.

### W0.5 — Replace `GameInput` with `leafwing-input-manager`

**What:** Define `Action` enum per spec §4.4/§4.5: `Move`, `Look`, `Jump`, `Sprint`, `Crouch`, `Interact`, `Place`, `RotatePiece`, `ToggleBuild`, `ToggleInventory`, `Hotbar1..9`, `ZoomIn`, `ZoomOut`, `OrbitCamera`. Bind keyboard+mouse default and gamepad default. Delete `input::GameInput` and migrate every consumer.
**Acceptance:** Every action listed in spec §4.4–§4.5 is bound and reachable from both KB+M and gamepad. `grep -r "GameInput"` returns zero results outside removed code. Cursor-to-world raycasting moves into a small helper system that any code can call.
**Closes:** DEC-007 superseded.

### W0.6 — Adopt `bevy_egui`, port crafting menu as pilot

**What:** Add `bevy_egui`. Re-implement the existing painted-panel crafting menu using egui with custom styling to preserve the Spiritfarer-inspired look (panel chrome via `egui::containers::Frame`, custom font via `egui::FontDefinitions`, ornament glyphs as icons). Vanilla Bevy UI for the HUD hotbar can stay until Phase 2 forces a unified rewrite.
**Acceptance:** Tab opens egui crafting menu with the same five categories and same recipe list. No visual regression vs. screenshots taken pre-migration. Input routes correctly (egui captures hover, game keeps move).

### W0.7 — Adopt Bevy 0.18 built-in atmosphere, keep day/night logic

**What:** Replace direct `ClearColor` and skybox manipulation in `daynight.rs` with Bevy 0.18's first-party procedural atmosphere (atmospheric occlusion + PBR shading shipped in 0.18). Sun position drives the atmosphere. Existing 6-phase logic now reads from a single `WorldTime` resource and writes the sun direction; atmosphere derives sky color. Note: the third-party `bevy_atmosphere` crate is *not* adopted — it is stuck on Bevy 0.16 as of the 2026-04-29 crate audit; the first-party feature replaces it cleanly and drops a dependency.
**Acceptance:** Visible day/night transition feels at least as good as current. Dawn/dusk gradients smoother than before. Moonlight at night still readable.

### W0.8 — Bump day length to 24 minutes

**What:** Update `WorldTime` cycle constant. Validate phase boundaries still feel right (dawn 5–7, morning 7–9, day 9–16, dusk 16–18, twilight 18–20, night 20–5) at the longer cadence.
**Acceptance:** A full day takes 24 ± 0.5 real minutes. Phase transitions visibly happen at the documented in-game hours.
**Closes:** DEC-006 updated.

### W0.9 — Adopt `bevy_asset_loader` and `Loading` state

**What:** Add `bevy_asset_loader`. Build an `AssetCollection` for shared meshes/materials/fonts/textures (the existing `PropAssets` becomes one collection). Asset load runs in `GameState::Loading`. Loading screen is a minimal egui panel with a progress indicator.
**Acceptance:** Game boots into a Loading screen for ≤2s on warm cache, then transitions to MainMenu. Cold-cache load completes without panics.

### W0.10 — Game state machine

**What:** Define `GameState`: `Loading`, `MainMenu`, `Playing`, `Paused`. Define sub-state `BuildState` under `Playing`: `Idle`, `Building`. Use `DespawnOnExit` for state-scoped UI/entities. Wire MainMenu to a placeholder egui screen with "New Game" / "Load Game" / "Settings" / "Quit". Pressing Esc in `Playing` enters `Paused`.
**Acceptance:** State transitions are smooth. UI scoped to a state despawns on exit. Hotbar/build UI only appears in `Playing`.
**Closes:** DEBT-003, DEBT-011.

### W0.11 — Replace `save.rs` with `moonshine-save`

**What:** Add `moonshine-save`. Mark all save-relevant components with `#[reflect(...)]` and `Save` markers. Remove the hand-written `save.rs` writer/parser. Migrate the existing JSON save format to the moonshine on-disk format (still JSON, still readable). Keep the auto-save 30s cadence and F5 manual save.
**Acceptance:** Save → quit → relaunch → load restores player position, inventory, placed buildings, world seed, current in-game time. New components added in later phases require only `#[derive(Reflect)] + #[reflect(Save)]` to persist.
**Closes:** DEC-011 superseded.

### W0.12 — Steam Cloud-friendly save path

**What:** Resolve save dir via `directories::BaseDirs::data_dir()` (or `dirs_next`), namespaced as `Cat World`. On Steam, this matches the path Steam will sync when configured. Add a `--save-dir` CLI override for testing. Document the path in `README.md`.
**Acceptance:** Saves land at the platform-correct path. On macOS that is `~/Library/Application Support/Cat World/`. `--save-dir` override works.

### W0.13 — Seed in save, new-world flow

**What:** World seed becomes part of save data, not a hardcoded `42`. New Game dialog accepts a numeric seed or empty for random.
**Acceptance:** Two saves with different seeds load into different worlds. Seed in save is preserved across save/load round-trip.
**Closes:** DEBT-004.

### W0.14 — Smoke test scene + headless test scaffold

**What:** Add `cargo test` integration test that boots `App` with all plugins, ticks 30 frames, exits cleanly. Set up `tests/` directory and add a `headless` feature gate to skip windowing in tests.
**Acceptance:** `cargo test` passes in CI-style environment. First step toward closing DEBT-006.
**Closes:** First brick of DEBT-006.

## Risks / open questions

- **Crate compatibility audit completed 2026-04-29.** Confirmed 0.18-ready: rapier3d, tnua, leafwing, egui, asset_loader, moonshine-save (last commit 2026-04-29), kira_audio. Dropped: `bevy_atmosphere` (stuck on 0.16, replaced by Bevy 0.18 built-in). Deferred to their phases: navmesh (was `oxidized_navigation`, now `bevy_landmass`, see Phase 1) and utility AI (was `big-brain`, now hand-rolled, see Phase 5; `big-brain` archived 2025-10-07).
- **Egui visual parity.** The painted-panel crafting menu may look different in egui. Acceptance is "no regression" per screenshots — set a tight tolerance and iterate if needed.

## Out of scope

- Terrain rewrite (Phase 1)
- Modular building kit (Phase 2)
- NPC cats (Phase 5)
- Cat character model and animations (deferred per user)

## Estimated effort

5–8 work-days assuming all crate versions are available. Add 1–3 days per crate that needs a 0.18 fork.
