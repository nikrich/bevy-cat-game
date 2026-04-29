# Session Journal

## 2026-04-29 -- Phase 0 closed (12 shipped + 2 accepted deferrals)

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
- W0.3 + W0.4 rapier + tnua + jump (follow-up session, DEBT-016 closed). `RapierPhysicsPlugin` + `TnuaControllerPlugin::<ControlScheme>::new(Update)` + `TnuaRapier3dPlugin::new(Update)`. Per-tile `Collider::cuboid` on terrain (intentionally throwaway, Phase 1 swaps for vertex-height trimesh). Prop colliders attached on a child entity offset to the prop's vertical centre so climb-on-top works. Per-form colliders on placed buildings (walls/doors/windows/floors/roofs) via `attach_for_form`, retiring the hand-rolled `push_player_out_of_walls`. Player: capsule rigid body + `LockedAxes::ROTATION_LOCKED` + `TnuaController` + `TnuaConfig` with `TnuaBuiltinWalkConfig { speed: 5.0, float_height: 1.0, cling_distance: 0.3, max_slope: PI/3 }` and `TnuaBuiltinJumpConfig { height: 1.6 }`. `Action::Jump` (Space + gamepad South) triggers the jump action; suppressed in build mode where Space is overloaded for placement. `snap_to_terrain` removed. Wading tuned: `floor_y = step_height(SEA_LEVEL) * 0.5 - 1.05` so capsule centre settles at y≈0 and the cat is half-submerged inside the water mesh. `init_water_ripples` switched to `try_insert` to swallow chunk-unload races that physics-driven `player_chunk` churn surfaced. Quit hard-exits via `std::process::exit(0)` (DEBT-017 — `AppExit` deadlocks under Bevy 0.18 + rapier + egui)

**Deferred with debt (acknowledged for Phase 0):**
- W0.7 procedural atmosphere — trialled and reverted; Bevy 0.18's Earth-scale lighting (RAW_SUNLIGHT, km-scale geometry, HDR + AcesFitted + Bloom) clashed with the warm pastel palette. DEC-013 amended to keep the manual `daynight::update_sky_color` gradient. Revisit only if a phase needs something the manual sky can't represent, or pair with the Phase 7 polish pass. DEBT-014
- W0.11 moonshine-save — declined after digging in: save data is dominated by *resources* (Inventory, WorldMemory, Journal, ChunkManager.seed), not entity-tagged components, so moonshine's reflection-auto-serialize win is marginal; on-disk format change JSON→RON would amend DEC-015; `ItemId`↔`save_key` glue still needed. Revisit at start of Phase 5 (NPC archetypes flip the cost-benefit). DEBT-015

**Decisions recorded:** DEC-013 (Bevy 0.18 + ecosystem stack, atmosphere clause amended to keep the manual gradient), DEC-014 (game state machine), DEC-015 (moonshine save format — declared, then deferred to Phase 5), DEC-016 (24-minute day cycle, supersedes DEC-006). Superseded: DEC-006, DEC-007, DEC-011

**Tech debt closed:** DEBT-003, DEBT-004, DEBT-011, DEBT-012, DEBT-016. Opened: DEBT-013 (Phase 0 catch-all, since superseded), DEBT-014 (atmosphere mismatch), DEBT-015 (moonshine deferred), DEBT-017 (AppExit deadlock).

**Files created:** `src/state.rs`, `src/ui/crafting_egui.rs`, `tests/smoke.rs`. Spec docs added under `spec/phases/` (00-index through 08-launch)

**Files heavily modified:** `Cargo.toml` (bevy 0.18, +leafwing-input-manager, +bevy_egui, +bevy_asset_loader, +bevy_rapier3d, +bevy-tnua, +bevy-tnua-rapier3d, +directories), `src/main.rs`, `src/input/mod.rs` (full rewrite), `src/ui/mod.rs` (crafting menu spawn skipped, HUD gated to Playing), `src/save.rs` (OS-aware path + seed persistence), `src/player/mod.rs` (full rewrite — tnua-driven), `src/world/mod.rs`, `src/world/biome.rs`, `src/world/terrain.rs` (rapier colliders + wade depth), `src/world/props.rs` (child collider entities), `src/building/collision.rs` (rapier-resolved walls), plus every gameplay consumer to swap `GameInput` → `ActionState<Action>` + `CursorState`

**Surprising things:**
- bevy_egui's `EguiPrimaryContextPass` is a proper schedule; multiple egui screens just register multiple systems on it with state-gated `run_if`
- `bevy-tnua` is the dashed crate name; `bevy_tnua` doesn't exist on crates.io
- Bevy 0.18's atmosphere is not a clean drop-in for non-realistic art directions; "tune the dials" fights the model
- 16-param SystemParam limit hits faster than expected once you split `GameInput` into `ActionState` + `CursorState` and consumers add `crafting`/`build_mode`/etc. — `place_building` had to drop the unread `PlaceEvent` to fit
- `TnuaBuiltinWalkConfig::speed` defaults to 20.0 and *multiplies* `desired_motion`. If you also pre-multiply by a unit-scale player speed, you get a 100 m/s cat. Pass a unit vector × sprint factor and let the config own the m/s
- Tnua's `TnuaScheme` derive generates `<EnumName>Config` as a sibling type in the same module — not under `bevy_tnua::controller`. Importing that path was a dead end
- The chunk unload race (entity despawned between query collection and deferred command apply) gets *much* more frequent under physics-driven player position because `player_chunk` recomputes faster while gravity/spring are settling. `try_insert` is the idiomatic fix
- Bevy 0.18 + rapier + egui can deadlock on `AppExit` shutdown — process exits cleanly via `std::process::exit(0)` but hangs on the polite path. DEBT-017

**Open threads (out of Phase 0 scope):**
- W0.7 atmosphere stays as DEBT-014; revisit during Phase 7 polish or pair with Phase 1 W1.3 shader work as a custom gradient skybox
- W0.11 moonshine-save stays as DEBT-015; revisit at start of Phase 5 when NPC archetypes flip the cost-benefit
- DEBT-017 AppExit deadlock; bisect across Bevy/rapier/egui upgrades, or re-route Quit through "auto-save + return to MainMenu"

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
