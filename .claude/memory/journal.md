# Session Journal

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
