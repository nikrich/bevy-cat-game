# Architectural Decision Log

## DEC-001: Use Bevy over Unity/Godot
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need a game engine where AI can build everything end-to-end via code -- no GUI editor dependency
- **Decision**: Bevy (Rust) -- 100% code-driven, ECS architecture ideal for PCG, no editor required
- **Alternatives**: Unity (editor-dependent, .meta files fragile), Godot 4 (.tscn writable but editor-preferred), React Three Fiber (lower performance ceiling)
- **Consequences**: Steeper Rust learning curve, smaller ecosystem than Unity, but full code control and great performance

## DEC-002: Isometric orthographic camera
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Top-down 3D world crafting game needs a camera style
- **Decision**: Isometric angle (~45 degrees) with orthographic projection -- standard for peaceful crafting genre (Stardew Valley, Cult of the Lamb style)
- **Alternatives**: Fixed top-down 90 degrees (loses depth), perspective iso (parallax complexity), free camera (overwhelming for peaceful game)
- **Consequences**: Movement must be rotated to align with camera, UI placement is predictable, world feels consistent at any zoom

## DEC-003: Perlin noise stepped terrain
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need PCG terrain that matches low-poly art style
- **Decision**: Layered Perlin noise with height quantized to 0.25 steps, color-mapped by elevation (sand/dirt/grass)
- **Alternatives**: Wave Function Collapse (better for structured layouts), Voronoi (better for biome boundaries), flat grid (boring)
- **Consequences**: Natural-looking terrain with chunky aesthetic, easy to extend with biomes, need chunk system for infinite world

## DEC-004: Chunk-based terrain with per-tile entities
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need infinite terrain but spawning the whole world at startup is impossible. Must balance load/unload granularity with entity count and frame hitches (ref DEC-003)
- **Decision**: 16x16 tile chunks, render distance 3 (7x7 = 49 chunks visible), max 4 chunks loaded per frame to spread cost. Each tile is a separate entity (Cuboid) parented to the chunk entity
- **Alternatives**: Single-mesh-per-chunk (better GPU perf, harder to interact with individual tiles), larger chunks (fewer entities but bigger load spikes), ECS-less mesh generation (loses Bevy query benefits)
- **Consequences**: ~12,500 tile entities at steady state -- acceptable for now. Per-tile entities make future gathering/interaction easy (just query tiles). Can migrate to baked meshes later if profiling shows entity count is the bottleneck. Chunk children auto-despawn when chunk is despawned

## DEC-005: Noise-based prop placement
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Props (trees, rocks, flowers) need to feel natural and be deterministic (same seed = same world). Must be biome-aware and despawn with chunks
- **Decision**: Secondary Perlin noise layers (seeds 137, 251) control density and variety. Biome-aware rules: grass gets trees/bushes/flowers/mushrooms, dirt gets rocks/mushrooms, sand gets sparse rocks. Props spawn as children of chunk entities
- **Alternatives**: Random scatter with RNG per chunk (less natural clustering), Wave Function Collapse (overkill for prop placement), hand-placed templates (breaks PCG promise)
- **Consequences**: Deterministic placement -- revisiting an area always looks the same. Child-of-chunk parenting means automatic cleanup on unload. Density threshold (0.55) keeps ~15-20% of tiles decorated. Easy to tune per-biome by adjusting noise thresholds

## DEC-006: Day/night cycle timing and atmosphere
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need time-of-day to create atmosphere without time pressure. Must feel peaceful -- no "you must sleep or die" mechanics. Cycle length affects how often players experience transitions
- **Decision**: Full cycle in 12 real minutes (2 in-game hours per real minute). Start at 8am (pleasant morning). Six phases: dawn 5-7, morning 7-9, day 9-16, dusk 16-18, twilight 18-20, night 20-5. Night uses dim cool-blue moonlight rather than darkness
- **Alternatives**: Longer cycle (30+ min, transitions too rare to notice), real-time clock (locks players into their timezone's time), no night (misses atmosphere opportunity)
- **Consequences**: Players see a full dawn-to-dusk cycle in a short session. Night is still playable (moonlight, not blackout) -- reinforces peaceful identity. Sky color, sun position, and ambient light all transition smoothly. Speed is adjustable via WorldTime resource for future player control

## DEC-007: Unified input abstraction for KB+mouse and gamepad
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Game needs to support both keyboard+mouse and gamepad. Building placement needs mouse cursor-to-world raycasting. Multiple systems (player, crafting, gathering, building) all read input independently
- **Decision**: Single GameInput resource populated in PreUpdate from all input sources. All game systems read GameInput, never raw keyboard/mouse/gamepad. Cursor raycasts to Y=0 plane for world position
- **Alternatives**: leafwing-input-manager crate (adds dependency, heavier), per-system raw input reading (duplicated logic, hard to add gamepad)
- **Consequences**: Clean separation of input reading from game logic. Adding new input sources (touch, rebinding) only requires changes in input/mod.rs. Slight latency (1 frame) between input and action due to PreUpdate timing

## DEC-008: Manual JSON save format without serde
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need save/load for player position, inventory, and placed buildings. Adding serde + serde_json adds compile time and dependency weight
- **Decision**: Hand-written JSON serialization and simple string-based parsing. Save format is human-readable and editable
- **Alternatives**: serde + serde_json (automatic, more robust), bincode (compact, not human-readable), RON (Rust-native but still a dependency)
- **Consequences**: Fragile parser that needs manual updates when save format changes. Works for current scope but should migrate to serde if save format grows complex (more entity types, world state, etc.)

## DEC-010: Registry-based combinatorial item system
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Hardcoded ItemKind enum has 15 items. User wants thousands of items to support beautiful houses with furniture. Authoring thousands of variants by hand is infeasible
- **Decision**: Replace ItemKind enum with `ItemId(u32)` handles into an `ItemRegistry`. Items are generated combinatorially as `(Form, Material)` pairs. Forms are visual archetypes (Chair, Wall, Lamp, ...), Materials are palette/properties (Pine, Oak, Stone, ...). Recipes are templated against MaterialFamily so one "Chair" recipe yields Pine Chair / Oak Chair / Birch Chair from the player's choice of input
- **Alternatives**: Hand-authored item enum (doesn't scale), pure procedural generation (loses authorial control), tag-only system (less data-oriented)
- **Consequences**: Cross-cutting refactor across inventory, crafting, building, gathering, save, ui. Display names generated from registry. Mesh handles cached per Form so spawn cost stays flat. Enables phases 3/4/5 (browser UI, modular houses, dyes). Breaks DEC-008 -- save must move to serde (DEC-011)

## DEC-011: Migrate save format to serde
- **Date**: 2026-04-29
- **Status**: Accepted (supersedes DEC-008)
- **Context**: DEC-008 chose hand-written JSON to avoid the serde dependency. With registry-based items (DEC-010) the save format must serialize ItemId, RecipeId, building grid cells, and per-instance tints. Hand-rolling that is fragile
- **Decision**: Adopt `serde` + `serde_json` for save/load. Keep .json on disk for human-readability. Persist items by stable string key (Form_name + Material_name) rather than numeric ItemId so registry rebuilds across sessions remain compatible
- **Alternatives**: Keep manual parser (breaks under registry), bincode (compact but unreadable), RON (Rust-native but extra dep with no human-readable advantage over JSON)
- **Consequences**: One added dependency. ~50 lines of save.rs become a few derive macros. DEBT-009 closes when migration lands

## DEC-009: Temperature/moisture biome classification
- **Date**: 2026-04-29
- **Status**: Accepted
- **Context**: Need diverse biomes that feel natural and transition smoothly. Simple height-based coloring (sand/dirt/grass) was too uniform
- **Decision**: Two additional noise layers (temperature, moisture) at large scale drive biome classification. Temperature decreases with altitude (lapse rate). Mountains use ridged noise for sharp peaks. Rivers carved by domain-warped noise near zero-crossings
- **Alternatives**: Voronoi-based biome regions (hard edges), single noise axis (limited variety), hand-painted biome map (breaks infinite PCG)
- **Consequences**: 10 distinct biomes emerge naturally. Smooth transitions at boundaries. Deterministic from seed. Temperature-altitude coupling creates snow caps automatically
