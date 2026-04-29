# Session Journal

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
