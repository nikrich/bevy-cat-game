# Memory Palace

## Art direction
- [2026-04-29] Bipedal cat character: low-poly, warm tan, rounded forms, smooth shading
- [2026-04-29] World aesthetic: earthy tones, stepped terrain, chunky low-poly
- [2026-04-29] Camera: isometric orthographic, ~45 degree angle

## Game vision
- [2026-04-29] Peaceful, lifelong game -- no win/lose state
- [2026-04-29] PCG world crafting -- always new things to discover
- [2026-04-29] Player is a bipedal cat
- [2026-04-29] Core loop: explore, gather, craft, build, discover
- [2026-04-29] Target: desktop and console

## Technical notes
- [2026-04-29] Using Bevy 0.16 (latest) -- fallible systems, new spawn API, entity relationships
- [2026-04-29] Perlin noise terrain with layered octaves for natural look
- [2026-04-29] Height quantized to 0.25 steps for low-poly stepped aesthetic
- [2026-04-29] Isometric camera uses orthographic projection (FixedVertical 20.0)
- [2026-04-29] Input abstraction: GameInput resource polled in PreUpdate, all systems read from it
- [2026-04-29] Cursor-to-world raycasting for mouse-based building placement (Y=0 plane intersection)
- [2026-04-29] Rand 0.8 API: use thread_rng() and gen_range(), not rng() or random_range()

## Preferences
- [2026-04-29] No em dashes in any written content
- [2026-04-29] User provides all assets -- AI builds everything else end-to-end
- [2026-04-29] Conventional Commits for version control

## Architecture
- [2026-04-29] Chunk system: 16x16 tiles, render distance 3, max 4 chunks loaded per frame
- [2026-04-29] Biome system: WorldNoise struct holds all Perlin generators (6 noise layers from seed)
- [2026-04-29] Props: biome-aware spawning via PropAssets struct (shared meshes/materials)
- [2026-04-29] Day/night: 12-min full cycle, 6 phases, starts at 8am, moonlight at night
- [2026-04-29] Input: GameInput resource, PreUpdate polling, supports KB+mouse and gamepad
- [2026-04-29] Save/load: manual JSON serialization (no serde dependency), auto-save 30s
- [2026-04-29] Animals: spawn with chunks, 0-2 per chunk, despawn with chunk parent

## Module structure (as of session 2)
```
src/
  main.rs           -- App entry, plugin registration (11 plugins)
  input/mod.rs      -- GameInput resource, KB+mouse+gamepad polling, cursor raycasting
  camera/mod.rs     -- Isometric camera, smooth follow (GameCamera is pub)
  player/mod.rs     -- Player movement, terrain snapping, loaded position
  world/
    mod.rs          -- WorldPlugin, system registration
    biome.rs        -- WorldNoise, Biome enum, terrain colors, classification
    terrain.rs      -- Chunk terrain generation, water tiles, Tile/WaterTile components
    chunks.rs       -- ChunkManager, load/unload, ChunkLoaded event
    props.rs        -- 11 prop types, biome-aware spawning, PropSway system
    daynight.rs     -- WorldTime, sun/sky/ambient transitions
    water.rs        -- WaterRipple, ambient wave animation
  inventory/mod.rs  -- ItemKind (15 items), Inventory resource, InventoryChanged event
  gathering/mod.rs  -- NearbyGatherable, proximity detection, shrink animation
  crafting/mod.rs   -- 8 recipes, CraftingState, Tab menu
  building/mod.rs   -- BuildMode, ghost preview, grid-snapped placement
  animals/mod.rs    -- 5 animal types, wander/flee AI
  particles/mod.rs  -- 5 particle types, biome+time-of-day aware spawning
  save.rs           -- Auto-save/load, JSON format, LoadedPlayerPos resource
  ui/mod.rs         -- HUD: hotbar, crafting menu, gather prompt, build prompt
```

## External resources

## Ideas parking lot
- Discovery journal tracking new biomes, items, recipes found
- Weather system (rain in forests, sandstorms in desert)
- NPC villagers that trade items
- Outfit customization for the cat
- More building types (walls, floors, doors, roofs)
- Furniture interiors (sit on bench, sleep in bed)
