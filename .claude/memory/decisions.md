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
