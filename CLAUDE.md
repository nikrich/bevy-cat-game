# Bevy Cat Game

A peaceful, lifelong top-down 3D world-crafting game. You play as a bipedal cat exploring a procedurally generated world -- always something new to discover, craft, and build.

## Memory palace

Project memory lives in `.claude/memory/`. Check these before starting work:
- `palace.md` -- general knowledge, art direction, external resources
- `decisions.md` -- architectural decision log (why, not what)
- `game-design.md` -- living game design document
- `tech-debt.md` -- known shortcuts and when to fix them
- `journal.md` -- session history

Use `/remember` to store, `/recall` to search, `/decision-log` to record decisions.

## Custom commands

| Command | Purpose |
|---------|---------|
| `/brainstorm [area]` | Generate game feature/mechanic ideas |
| `/challenge [approach]` | Stress-test a design or architecture decision |
| `/decision-log [record\|query]` | Record or query WHY decisions were made |
| `/game-design [update\|show]` | View or update the living game design doc |
| `/debt [add\|list\|scan]` | Track tech debt |
| `/journal` | Summarize what was done this session |
| `/remember [thing]` | Save to memory palace |
| `/recall [query]` | Search all memory |
| `/playtest` | Compile, run briefly, and validate current state |
| `/build-system [name]` | Scaffold a new ECS system with proper Bevy 0.16 patterns |

## Quick reference

```
cargo run                    # run the game
cargo check                  # fast compile check
cargo test                   # run all tests
cargo clippy                 # lint
cargo run --release          # optimized build
```

## Project structure

```
src/
  main.rs              -- App entry, window, plugin registration
  camera/mod.rs        -- Isometric camera, smooth follow
  player/mod.rs        -- Cat player, movement, animation
  world/
    mod.rs             -- WorldPlugin
    terrain.rs         -- PCG terrain (Perlin noise, biomes)
    chunks.rs          -- Chunk loading/unloading (future)
    props.rs           -- Trees, rocks, flowers (future)
assets/
  models/              -- .glb models (cat, props, buildings)
  textures/            -- Terrain, UI textures
  fonts/               -- Game fonts
  audio/               -- Music, SFX
```

## Bevy 0.16 patterns (MUST follow)

### Fallible systems -- use Result, not unwrap
```rust
fn my_system(query: Query<&Transform>) -> Result {
    let transform = query.single()?;  // returns Result in 0.16
    Ok(())
}
```

### Spawning hierarchies -- use Children::spawn
```rust
commands.spawn((
    Name::new("Parent"),
    Children::spawn((
        Spawn(Name::new("Child1")),
        Spawn((Name::new("Child2"), Transform::default())),
    )),
));
```

### Loading glTF models
```rust
commands.spawn((
    SceneRoot(asset_server.load("models/cat.glb#Scene0")),
    Transform::from_xyz(0.0, 0.0, 0.0),
));
```

### Game states
```rust
#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum GameState {
    #[default]
    Loading,
    Menu,
    Playing,
    Paused,
}
// Use DespawnOnExit(GameState::Menu) to auto-cleanup
// Use in_state(GameState::Playing) as run condition
```

### Entity relationships (NOT Parent/Children)
```rust
// Old (0.15): Parent, Children components
// New (0.16): ChildOf relationship, Children target
// ChildOf and Children are immutable -- use commands to modify hierarchy
```

## Architecture rules

1. **Plugin per domain** -- every feature is a Plugin registered in main.rs
2. **Systems return Result** -- never unwrap queries, use `?` operator
3. **Components are data-only** -- no methods on components, logic lives in systems
4. **Resources for global state** -- game settings, world seed, player inventory
5. **Events for cross-system communication** -- don't couple systems directly
6. **States for game flow** -- Loading, Menu, Playing, Paused, Building
7. **DespawnOnExit for cleanup** -- tag state-specific entities for auto-despawn

## Art direction

- Low-poly with smooth shading, warm earthy tones
- Stepped terrain for chunky aesthetic
- Palette: tans, warm greens, muted oranges, soft browns
- Matches the bipedal cat character model (warm tan, rounded forms)
- Isometric camera angle (~45 degrees)

## Conventions

- **Commits**: Conventional Commits (`feat:`, `fix:`, `chore:`, `refactor:`, `test:`)
- **No em dashes** in any written content
- **Snake_case** for Rust (enforced by compiler)
- **One system per concern** -- small, focused systems over monolithic ones
- **Test systems in isolation** -- spawn minimal App with only required plugins

## Performance guidelines

- `opt-level = 2` for dependencies in dev (already configured)
- Chunk-based terrain -- only render chunks near the player
- LOD for distant terrain (fewer polygons)
- Use `Changed<T>` and `Added<T>` filters to avoid unnecessary work
- Batch similar spawns -- don't spawn 10,000 entities in one frame
