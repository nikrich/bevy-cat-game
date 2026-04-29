---
name: build-system
description: Scaffold a new ECS system with proper Bevy 0.16 patterns
argument-hint: "[system-name] e.g. 'chunk-loading', 'inventory', 'day-night'"
arguments:
  - name
allowed-tools:
  - Read
  - Write
  - Glob
  - Grep
  - Bash
user-invocable: true
---

You are scaffolding a new ECS system for bevy-cat-game using Bevy 0.16 best practices.

## Process

1. Read `.claude/memory/game-design.md` to understand where this system fits
2. Read `.claude/memory/decisions.md` for any relevant prior decisions
3. Scan existing code to understand current architecture and avoid conflicts

## Scaffold

For the system "$ARGUMENTS", create:

### 1. Module structure
```
src/{name}/
  mod.rs     -- {Name}Plugin, re-exports
  components.rs  -- Component structs (data-only)
  systems.rs     -- System functions (return Result)
  events.rs      -- Event types (if needed)
  resources.rs   -- Resource types (if needed)
```

### 2. Plugin registration
```rust
pub struct {Name}Plugin;

impl Plugin for {Name}Plugin {
    fn build(&self, app: &mut App) {
        app
            .add_event::<{Event}>()           // if events needed
            .insert_resource({Resource}::default())  // if resources needed
            .add_systems(Startup, setup_system)
            .add_systems(Update, (
                system_a,
                system_b.after(system_a),  // explicit ordering where needed
            ).run_if(in_state(GameState::Playing)));  // state-gated
    }
}
```

### 3. Patterns to follow
- Systems return `Result` (fallible systems)
- Use `Query::single()?` not `Query::single().unwrap()`
- Components are plain structs with `#[derive(Component)]` -- no methods
- Events for cross-system communication
- `Changed<T>` / `Added<T>` filters where applicable
- `DespawnOnExit(state)` for state-specific entities

### 4. Basic test
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;

    #[test]
    fn test_system_name() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, system_name);
        // setup, update, assert
    }
}
```

## After scaffolding

1. Register the new plugin in `src/main.rs`
2. Run `cargo check` to verify compilation
3. Update `.claude/memory/game-design.md` to reflect the new system
4. If this creates tech debt, log it with `/debt add`
