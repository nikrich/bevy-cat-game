# Living Game Design Document

## Core identity
A peaceful, lifelong world-crafting game where you play as a bipedal cat exploring an infinite procedurally generated world. No enemies, no time pressure -- just discovery, crafting, and building.

## Player character
- Bipedal cat (low-poly, warm tan, smooth shading)
- Walks, runs, carries items
- Future: emotes, sitting, sleeping, outfit customization

## World generation
- **Terrain**: Perlin noise, stepped heights, biome coloring
- **Biomes**: Grassland, forest, desert, beach, mountain, meadow (planned)
- **Props**: Trees, rocks, flowers, mushrooms, bushes (planned)
- **Water**: Rivers, ponds, ocean edges (planned)
- **Structures**: Ruins, caves, special landmarks (planned)
- **Seasons**: Day/night cycle, weather (planned)

## Core systems (priority order)
1. [x] Basic terrain generation
2. [x] Player movement (WASD)
3. [x] Isometric camera follow
4. [ ] Chunk-based infinite terrain
5. [ ] Cat character model integration
6. [ ] Props/decoration spawning (trees, rocks, flowers)
7. [ ] Inventory system
8. [ ] Gathering (pick up items from world)
9. [ ] Crafting system
10. [ ] Building/placement system
11. [ ] Save/load world state
12. [ ] Day/night cycle
13. [ ] Biome system
14. [ ] Water and terrain features
15. [ ] NPC animals
16. [ ] Music and ambient audio
17. [ ] UI/HUD
18. [ ] Particle effects (leaves, fireflies, rain)

## Crafting philosophy
- Simple recipes: combine 2-3 gathered materials
- Discovery-based -- try combinations, unlock recipes
- Categories: tools, furniture, decorations, food, clothing

## Building philosophy
- Grid-snapped placement on terrain
- Rotate and stack objects
- No structural integrity -- creative freedom
- Buildings are cosmetic/organizational, not survival-critical

## Discovery philosophy
- New biomes reveal new materials and recipes
- Rare procedural landmarks reward exploration
- No map markers -- you find things by wandering
- Journal/collection book tracks discoveries
