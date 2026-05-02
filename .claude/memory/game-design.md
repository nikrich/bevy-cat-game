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
17. [x] UI/HUD (inventory hotbar, crafting menu, build prompt, gather prompt, brush hotbar)
18. [x] Particle effects (leaves, fireflies, snow, sand, pollen)
19. [x] Terrain editing (Raise / Lower / Flatten / Smooth / Paint brushes, edits persist)

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

## Build mode (cube-placement, DEC-020)
- B key toggles. Owns Wall, Floor, Door, Window, Roof, Fence
- Minecraft-style 1×1×1 cube placement. Walls are full cubes
- Rapier raycast from camera through cursor picks the target cell. Hit normal decides:
  - Top face → cube above (vertical stacking)
  - Side face → cube adjacent in normal direction
  - Terrain hit → cube on terrain at the hit cell
- Walls use the **line tool** (drag-to-build chain). First click sets anchor; second click fills cells from anchor to cursor; anchor advances to last placed cube so perpendicular cursor moves trace L-bends sharing the corner cube
- Floors use **paint** (hold and drag stamps tiles, whole drag = one undo)
- Doors and Windows use **Replace** (click an existing wall to swap in)
- Tools: Place / Remove. Bottom hotbar with 6-swatch piece selector
- Buildings persist across save/load (Vec3 transforms; cube-cell save format is a follow-up)

## Decoration mode (magnetic-continuous, DEC-021)
- N key toggles. Mutually exclusive with Build. Owns Bed, Chair, Lantern, Table, Bench, Campfire, Barrel, Bucket, FlowerPot, Wreath, Chest, and all `Form::Interior` items (~1000 LowPoly Interior pack)
- Magnetic-continuous placement: cursor slides the piece along its attach surface (terrain top, floor top, wall face, furniture top). v1 ships fine 0.1m grid; v2 adds anchor magnets (cell centres, wall mid-points, neighbour edges) with `Alt` to break
- Rotation: 15-degree steps (`R` / `Shift+R`); `Alt+R` for continuous
- Tools: Place / Move / Remove. Move picks up a placed piece, carries it on the cursor, drops on next click (no inventory churn)
- UI: bottom hotbar (tools + selected thumbnail + recent-picks quickbar of 8 slots, in-memory) plus right-side catalog (1000 thumbnails, search, categories)
- Hover and held pieces show subtle highlights (cyan / red / gentle pulse)
- Single shared undo / redo stack across both modes (`EditHistory`)
- Inventory consumed same as Build; `INFINITE_RESOURCES = true` cheat covers both modes during development

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

## Mountain caves (designed, DEC-024)
- Mountains and Snow caps gain a voxel interior (0.5m³ sub-grid layered under the existing 1m heightmap) with PCG cave systems
- Three climate-flavoured cave generators, classified per chunk by surrounding biomes:
  - **Alpine** -- large rounded chambers, narrow connectors, glowing crystal clusters on side walls (V1 ship)
  - **Temperate** -- tight twisty critter warrens with sleeping nooks at branch ends (Phase 4 with critter AI)
  - **Arid** -- stitched authored room templates with ruin props at named anchors (Phase 5 with template authoring tool)
- Cave entrances always carved to a visible mountain face -- the player sees a dark mouth from outside
- No mining verb. Caves are explored, not stripped
- Brush stays heightmap-only. Lowering a mountain past a cave ceiling triggers a one-shot sinkhole, opens the chamber from above
- If the cat falls into a deep chamber, the player walks out via a connected tunnel or Raises the floor -- no auto-rope
- Caves are dark; ambient is masked when the cat is overhead-occluded. Player lantern owned by separate concurrent spec
- Spec: `docs/superpowers/specs/2026-05-02-voxel-mountain-caves-design.md`

## What's left
- Music and ambient audio (per-biome sounds)
- Cat character model (retry with Blender 4.x or external GLB validation)
- Discovery journal / collection book
- More crafting recipes and item types
- Weather system (rain, wind)
- NPC villagers / traders
- More building types (walls, floors, roofs)
