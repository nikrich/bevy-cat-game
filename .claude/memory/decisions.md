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
