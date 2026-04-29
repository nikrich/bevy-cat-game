# Session Journal

## 2026-04-29 -- Phase 0 foundations: Bevy 0.18, leafwing, egui, asset_loader, game states (10 of 14 items)

**What was done:**
- W0.1 Bevy 0.16 → 0.18.1 with the full breaking-change pass: `Event` → `Message` (derive, EventReader/Writer, add_event/add_message), UI bundles (BorderRadius into Node), AmbientLight split into per-camera component + GlobalAmbientLight resource, ScrollPosition Vec2 tuple, BorderColor per-side with `::all()`, CascadeShadowConfigBuilder moved to bevy::light, ScalingMode moved to bevy::camera, UiSystem → UiSystems, WindowResolution constructor changed
- W0.2 WorldNoise cached as a `Resource` via `FromWorld` reading `ChunkManager.seed`. Every per-frame consumer (player, animals, building, particles, props, memory/verbs) now takes `Res<WorldNoise>`. Closes DEBT-012
- W0.5 Replaced hand-rolled GameInput with leafwing-input-manager: `Action` enum is exhaustive per spec §4.4/§4.5 plus game-specific verbs (ToggleCraft, Save, Nap/Examine/Mark, MenuUp/Down/Confirm, HotbarNext/Prev). KB+M and gamepad bindings shipped. Mouse-left intentionally unbound; gated through `CursorState::world_click`. The unread `PlaceEvent` was dropped to fit the 16-param SystemParam limit on `place_building`. Supersedes DEC-007
- W0.6 Pilot egui port of the crafting menu under `EguiPrimaryContextPass`. Data layer unchanged — `CraftingState`/`RecipeRegistry`/`CraftRequest` events still drive crafting. Visual approximation of the Spiritfarer parchment: warm dark Frame, gold stroke, gold/dim/red palette, scrollable recipe list, CRAFT/need-more pills. The Bevy UI crafting tree is no longer spawned
- W0.8 Day cycle bumped from 2.0 → 1.0 (12 min/day → 24 min/day). DEC-016 supersedes DEC-006
- W0.9 bevy_asset_loader Loading state: `UiAssets` derives `AssetCollection`; `LoadingState::new(Loading).continue_to_state(MainMenu).load_collection::<UiAssets>()` configured in `StatePlugin`. `spawn_ui` moved from PostStartup to `OnEnter(GameState::Playing)`. HUD systems gated with `run_if(in_state(Playing))`. Three new egui screens: Loading (centred title + spinner), MainMenu (title + Start Game + Quit), Pause overlay (Resume / Main Menu / Quit). Pause now also routes back to MainMenu cleanly
- W0.10 Game state machine: `GameState` (Loading / MainMenu / Playing / Paused) plus `BuildState` sub-state (Idle / Building) under Playing. Esc pauses by freezing `Time<Virtual>` rather than per-system gating. Esc-to-pause is suppressed when build mode or crafting menu is active so it doesn't hijack their cancel/close. Closes DEBT-003 + DEBT-011 / DEC-014
- W0.12 Save path resolves via `directories::BaseDirs::data_dir()` namespaced as "Cat World" — Steam Cloud-friendly per-user dir, e.g. `~/Library/Application Support/Cat World/savegame.json` on macOS. `--save-dir` CLI override for tests
- W0.13 World seed persisted in SaveData with `serde(default)` so legacy saves keep loading; ChunkManager.seed restored on load. Closes DEBT-004
- W0.14 Headless smoke tests in `tests/smoke.rs` using `MinimalPlugins + StatesPlugin`. Two tests: app boots and ticks 30 frames; state transition resolves in one tick. First brick of DEBT-006

**Deferred with debt:**
- W0.7 procedural atmosphere — trialled, reverted. Bevy 0.18's Earth-scale lighting (RAW_SUNLIGHT, km-scale geometry, HDR + AcesFitted + Bloom) clashed with the warm pastel low-poly palette. DEBT-014 logged; DEC-013 amended
- W0.11 moonshine-save — value of "new components auto-persist" is marginal until Phase 5 NPCs ship in volume; the migration also needs custom `ItemId` ↔ `save_key` glue and changes the on-disk format to RON (would amend DEC-015's JSON commitment). DEBT-015 logged
- W0.3 + W0.4 rapier + tnua — paired physics rewrite. Half-shipping is worse than nothing; needs a focused session because side effects ripple into terrain colliders, prop colliders for climb-on-top behaviour, step-up tuning for our 0.25-stepped terrain, and `pose_player` re-validation under physics-driven Y. DEBT-016 logged

**Decisions recorded:** DEC-013 (Bevy 0.18 + ecosystem stack, atmosphere clause amended), DEC-014 (game state machine), DEC-015 (moonshine save format — declared, then deferred to Phase 5), DEC-016 (24-minute day cycle, supersedes DEC-006). Superseded: DEC-006, DEC-007, DEC-011

**Files created:** `src/state.rs`, `src/ui/crafting_egui.rs`, `tests/smoke.rs`. Spec docs added under `spec/phases/` (00-index through 08-launch)

**Files heavily modified:** `Cargo.toml` (bevy 0.18, +leafwing-input-manager, +bevy_egui, +bevy_asset_loader, +directories), `src/main.rs`, `src/input/mod.rs` (full rewrite), `src/ui/mod.rs` (crafting menu spawn skipped, HUD gated to Playing), `src/save.rs` (OS-aware path + seed persistence), `src/world/mod.rs`, `src/world/biome.rs`, plus every gameplay consumer to swap `GameInput` → `ActionState<Action>` + `CursorState`

**Surprising things:**
- bevy_egui's `EguiPrimaryContextPass` is a proper schedule; multiple egui screens just register multiple systems on it with state-gated `run_if`
- `bevy-tnua` is the dashed crate name; `bevy_tnua` doesn't exist on crates.io
- Bevy 0.18's atmosphere is not a clean drop-in for non-realistic art directions; "tune the dials" fights the model
- 16-param SystemParam limit hits faster than expected once you split `GameInput` into `ActionState` + `CursorState` and consumers add `crafting`/`build_mode`/etc. — `place_building` had to drop the unread `PlaceEvent` to fit

**Open threads:**
- Next session: W0.3 + W0.4 (rapier + tnua) per DEBT-016. Recommended order: rapier plugin + terrain colliders → player rigid body (verify gravity drops cat onto terrain) → tnua controller + leafwing wiring → prop colliders → remove `snap_to_terrain`. Phase 1 then re-does terrain colliders against vertex-height grid, so per-tile cuboid is intentionally throwaway
- W0.7 atmosphere remains open as DEBT-014; revisit during Phase 7 polish or amend the spec to keep the manual gradient as canonical
- W0.11 moonshine-save remains open as DEBT-015; revisit at start of Phase 5

## 2026-04-29 -- Full gameplay loop: biomes, crafting, building, animals, particles, save/load

**What was done:**
- Built 10-biome system (temperature/moisture noise: ocean, beach, desert, grassland, meadow, forest, taiga, tundra, snow, mountain)
- Mountains with ridged noise for dramatic peaks, snow caps
- Water system: ocean + rivers with ambient wave animation
- Prop sway: vegetation tilts away from player and springs back
- Inventory with 15 item types (7 raw, 8 crafted)
- Gathering system: proximity detection, E/click to collect, shrink animation
- Crafting system: Tab menu, 8 recipes, ingredient checking
- Building system: B to enter, mouse-aimed ghost preview, grid-snapped placement, R to rotate
- NPC animals: 5 types per biome (rabbit, fox, deer, penguin, lizard), wander + flee AI
- Particle effects: 5 types (leaves, fireflies, snowflakes, sand wisps, pollen), biome + time-of-day aware
- Save/load: auto-save 30s, F5 manual, JSON format (player pos, inventory, buildings)
- Input abstraction: unified GameInput resource, KB+mouse and gamepad support, cursor raycasting
- HUD: inventory hotbar, crafting menu, build mode prompt, gather prompt with context hints
- Attempted cat model animation integration (failed -- Blender 5.1 glTF export issue, see animation_pitfalls memory)
- Smooth terrain snapping (lerp Y position)
- Thicker terrain tiles (0.6) to prevent gaps

**Key decisions:**
- DEC-004 through DEC-006 recorded (chunks, props, day/night)
- Input abstraction designed for gamepad from the start
- Manual JSON save (no serde dependency)
- Biome classification: temperature + moisture noise, altitude cooling

**Files created:**
- src/world/biome.rs, src/world/water.rs
- src/input/mod.rs, src/inventory/mod.rs, src/gathering/mod.rs
- src/crafting/mod.rs, src/building/mod.rs
- src/animals/mod.rs, src/particles/mod.rs
- src/save.rs, src/ui/mod.rs
- assets/models/cat.glb (model works, animations don't)

**Open threads:**
- Cat model animations (need to validate GLB externally, try Blender 4.x)
- Music and ambient audio not started
- Discovery journal not started
- Weather system not started
- Crafting UI could be more polished (user feedback: "looks bad")
- Water reflections/distortion not working well with tile-based approach

## 2026-04-29 -- Chunk terrain, props, and day/night cycle

**What was done:**
- Implemented chunk-based infinite terrain (16x16 tiles, render distance 3, 4 chunks/frame cap)
- Refactored terrain.rs from single spawn to per-chunk generation with shared height/biome helpers
- Added noise-based prop spawning: trees (trunk+canopy), rocks, flowers, bushes, mushrooms
- Props are biome-aware (grass: trees/bushes/flowers/mushrooms, dirt: rocks/mushrooms, sand: sparse rocks)
- Built full day/night cycle with 6 phases (dawn/morning/day/dusk/twilight/night), 12-min real cycle
- Sky color, sun position, directional light, and ambient light all transition smoothly
- Ran playtest: compiles clean, clippy clean, game launches and runs well

**Key decisions:**
- DEC-004: Per-tile entities in chunks (simpler, enables future interaction)
- DEC-005: Noise-based prop placement (deterministic, biome-aware)
- DEC-006: Day/night timing (12-min cycle, peaceful moonlight at night)

**Files created:**
- src/world/chunks.rs, src/world/props.rs, src/world/daynight.rs

**Files modified:**
- src/world/mod.rs (rewired to chunk/props/daynight systems)
- src/world/terrain.rs (refactored to per-chunk generation)

**Tech debt logged:**
- DEBT-005: let-else patterns in player/camera, DEBT-006: no tests
- DEBT-007: per-tile entities, DEBT-008: material/mesh duplication per chunk

**Open threads:**
- Cat .glb model still needed from user
- Next priorities: inventory system, gathering, crafting

## 2026-04-29 -- Project scaffolding

**What was done:**
- Initialized Bevy 0.16 project with Cargo
- Created module structure: main, camera, player, world/terrain
- Implemented PCG terrain with layered Perlin noise and stepped heights
- Built isometric orthographic camera with smooth player follow
- Added WASD player movement aligned to iso camera
- Set up warm earthy color palette (grass/dirt/sand)
- Created full skill/memory infrastructure

**Key decisions:**
- DEC-001: Bevy over Unity/Godot (code-driven, no GUI editor)
- DEC-002: Isometric orthographic camera
- DEC-003: Perlin noise stepped terrain

**Files created:**
- Cargo.toml, src/main.rs, src/camera/mod.rs, src/player/mod.rs
- src/world/mod.rs, src/world/terrain.rs
- .claude/ skill and memory infrastructure

**Open threads:**
- Cat .glb model needed from user
- Chunk system needed before expanding world
- No game states yet
