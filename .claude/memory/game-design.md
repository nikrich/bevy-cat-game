# Living Game Design Document

## Core identity
A peaceful, lifelong world-crafting game where you play as a bipedal cat exploring an infinite procedurally generated world. No enemies, no time pressure -- just discovery, crafting, and building.

## Player character
- Bipedal cat (low-poly, warm tan, smooth shading)
- Currently a placeholder capsule -- cat.glb exists but animations don't work (Blender 5.1 export issue)
- Walks, runs, carries items
- Future: emotes, sitting, sleeping, outfit customization

## World generation
- **Terrain**: Perlin noise, stepped heights, biome coloring, thick tiles (0.6)
- **Biomes**: 10 types -- Ocean, Beach, Desert, Grassland, Meadow, Forest, Taiga, Tundra, Snow, Mountain
- **Biome system**: Temperature + moisture noise layers drive classification
- **Props**: Trees, pine trees, rocks, boulders, flowers, mushrooms, bushes, cacti, dead bushes, ice rocks, tundra grass
- **Water**: Ocean + rivers (domain-warped noise), opaque blue tiles with ambient wave animation
- **Mountains**: Ridged noise for dramatic peaks, snow caps above threshold
- **Day/night**: 6 phases, 12-min cycle, smooth sky/light transitions

## Core systems (priority order)
1. [x] Basic terrain generation
2. [x] Player movement (WASD + gamepad)
3. [x] Isometric camera follow
4. [x] Chunk-based infinite terrain (16x16 chunks, render distance 3)
5. [ ] Cat character model integration (cat.glb exists, animations blocked)
6. [x] Props/decoration spawning (biome-aware, 11 prop types)
7. [x] Inventory system (7 raw + 8 crafted items)
8. [x] Gathering (proximity + E/click, shrink animation)
9. [x] Crafting system (8 recipes, Tab menu)
10. [x] Building/placement system (mouse-aimed preview, grid-snapped)
11. [x] Save/load world state (auto-save 30s, F5 manual, JSON)
12. [x] Day/night cycle (12-min full cycle, 6 phases)
13. [x] Biome system (10 biomes, temperature/moisture noise)
14. [x] Water and terrain features (ocean, rivers, waves)
15. [x] NPC animals (5 types, wander + flee AI)
16. [ ] Music and ambient audio
17. [x] UI/HUD (inventory hotbar, crafting menu, build prompt, gather prompt)
18. [x] Particle effects (leaves, fireflies, snow, sand, pollen)

## Input architecture
- Unified GameInput resource abstraction
- Keyboard+mouse and gamepad support
- Cursor-to-world raycasting for mouse building placement
- Context-sensitive control hints in UI

## Crafting recipes
- Wood -> Plank x2
- Stone x2 -> Brick x2
- Plank x2 + Wood -> Fence
- Plank x3 + Stone -> Bench
- Stone x2 + Wood + Flower -> Lantern
- Stone + Flower x2 -> Flower Pot
- Mushroom x2 + Bush -> Stew
- Flower x3 + Bush -> Wreath

## Building
- Grid-snapped placement on terrain
- Ghost preview follows mouse cursor
- R to rotate (90 degree increments)
- Placeable items: Fence, Bench, Lantern, FlowerPot, Wreath
- Buildings persist across save/load

## NPC Animals
- Rabbits: grassland, meadow
- Foxes: forest
- Deer: taiga
- Penguins: snow, tundra
- Lizards: desert
- Wander AI with random direction changes
- Flee from player within 4 units

## Particles
- Leaves: forest, taiga (falling)
- Fireflies: forest, meadow, grassland (dusk/night, emissive glow)
- Snowflakes: snow, tundra (drifting down)
- Sand wisps: desert (wind-driven)
- Pollen: meadow, grassland (floating)

## What's left
- Music and ambient audio (per-biome sounds)
- Cat character model (retry with Blender 4.x or external GLB validation)
- Discovery journal / collection book
- More crafting recipes and item types
- Weather system (rain, wind)
- NPC villagers / traders
- More building types (walls, floors, roofs)
