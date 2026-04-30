# Decoration Mode Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the monolithic `BuildMode` into a structural cube-grid `BuildMode` (B-key) and a magnetic-continuous `DecorationMode` (N-key), with a shared `edit/` infrastructure module for history, placed-item component, and highlight effects.

**Architecture:** Three modules with clear ownership: `src/edit/` (shared types: `EditHistory`, `PlacedItem`, `HighlightPlugin`), `src/building/` (cube-grid placement only), `src/decoration/` (surface-glide placement, Move tool, catalog UI). Mutually exclusive modes via independent toggle systems. v1 decoration placement uses a fine 0.1m grid; v2 magnet anchors land in a polish pass. Existing saves load through the renames because moonshine reflection keys on save key, not component name.

**Tech Stack:** Bevy 0.18, leafwing-input-manager, bevy_egui, bevy_rapier3d, moonshine-save. Existing crates only -- no new dependencies.

**Reference:** See `docs/superpowers/specs/2026-04-30-decoration-mode-split-design.md` for the design spec, `DEC-021` in `.claude/memory/decisions.md` for the ADR.

---

## Testing approach

The project is bin-only (`src/main.rs`, no `src/lib.rs`). Integration tests under `tests/` cannot reach game modules. So:

- **Pure functions** (snap math, angle quantize, surface-attach decision tree) get unit tests in inline `#[cfg(test)] mod tests` blocks within their source file. Run with `cargo test`.
- **Plugin / system / UI work** is verified by `cargo check` (compile clean) + manual playtest checkpoints (`cargo run` and exercise the relevant flow).

When a task uses TDD, the test goes inline. When a task is a rename or extraction, the verification is `cargo check` + `cargo test`.

**Manual playtest checkpoints** are called out at the end of each multi-task milestone. Treat them as required gates -- if a checkpoint fails, debug before moving on.

---

## File structure (target)

```
src/
  edit/                          # NEW shared infra
    mod.rs                       # EditPlugin, re-exports
    history.rs                   # EditHistory (renamed from BuildHistory), BuildOp, PieceRef
    placed_item.rs               # PlacedItem (renamed from PlacedBuilding)
    highlight.rs                 # HighlightPlugin -- hover + held tints
  building/
    mod.rs                       # BuildingPlugin, BuildMode, B-key toggle, cube placement
    placement.rs                 # compute_placement, line, paint, replace (extracted)
    ui.rs                        # bottom hotbar with 6-swatch piece selector
    collision.rs                 # unchanged
    history.rs                   # DELETED (moved to edit/)
  decoration/                    # NEW
    mod.rs                       # DecorationPlugin, DecorationMode, N-key toggle
    placement.rs                 # magnetic v1 (fine grid), surface attach
    interior.rs                  # interior asset spawn / AABB (moved from building)
    move_tool.rs                 # pick up / carry / drop
    catalog_ui.rs                # right-side 1000-thumb catalog (moved from building)
    hotbar_ui.rs                 # bottom hotbar with thumb + recent quickbar
```

`src/main.rs` plugin order: `EditPlugin` -> `BuildingPlugin` -> `DecorationPlugin`.

---

## Task 1: Scaffold `src/edit/` module (empty)

**Files:**
- Create: `src/edit/mod.rs`
- Create: `src/edit/history.rs` (placeholder)
- Create: `src/edit/placed_item.rs` (placeholder)
- Create: `src/edit/highlight.rs` (placeholder)
- Modify: `src/main.rs`

- [ ] **Step 1: Create empty module files**

`src/edit/mod.rs`:
```rust
use bevy::prelude::*;

pub mod highlight;
pub mod history;
pub mod placed_item;

pub use highlight::HighlightPlugin;
pub use history::{apply_redo, apply_undo, BuildOp, EditHistory, PieceRef};
pub use placed_item::PlacedItem;

pub struct EditPlugin;

impl Plugin for EditPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HighlightPlugin);
        history::register(app);
    }
}
```

`src/edit/history.rs`:
```rust
//! Placeholder -- contents land in Task 2.
```

`src/edit/placed_item.rs`:
```rust
//! Placeholder -- contents land in Task 4.
```

`src/edit/highlight.rs`:
```rust
use bevy::prelude::*;

pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, _app: &mut App) {
        // Wired up in Tasks 21 and 22.
    }
}
```

- [ ] **Step 2: Register `EditPlugin` in `main.rs`**

Find the line that adds `BuildingPlugin` in `src/main.rs` and add `EditPlugin` immediately before it:
```rust
.add_plugins(crate::edit::EditPlugin)
.add_plugins(crate::building::BuildingPlugin)
```

Also add `pub mod edit;` near the other top-level module declarations.

- [ ] **Step 3: Verify compile**

Run: `cargo check`
Expected: clean compile.

- [ ] **Step 4: Commit**

```bash
git add src/edit/ src/main.rs
git commit -m "refactor(edit): scaffold src/edit/ module with empty plugin"
```

---

## Task 2: Move `BuildHistory` -> `EditHistory` body into `src/edit/history.rs`

**Files:**
- Modify: `src/edit/history.rs` (now non-empty)
- Modify: `src/building/history.rs` (re-export shim)
- Modify: `src/building/mod.rs` (re-export update)

- [ ] **Step 1: Copy contents of `src/building/history.rs` into `src/edit/history.rs`**

Copy the full file contents. Then in the new `src/edit/history.rs`:
- Rename `BuildHistory` -> `EditHistory` everywhere in this file.
- Replace `use super::{spawn_placed_building, BuildMode, PlacedBuilding};` with:
  ```rust
  use crate::building::{spawn_placed_building, BuildMode};
  use super::placed_item::PlacedItem;
  ```
  (`PlacedItem` is empty for now -- a `pub struct PlacedItem;` typedef is added in Task 4. Until then, keep the old type name in the body and patch in Task 4.)

For now, leave references to `PlacedBuilding` as is -- we patch them in Task 4. Use a temporary alias at the top of `src/edit/history.rs`:
```rust
use crate::building::PlacedBuilding;
```

- [ ] **Step 2: Replace `src/building/history.rs` with re-export shim**

```rust
//! Moved to `crate::edit::history`. This shim keeps existing imports working
//! during the rename; can be deleted once all call sites are updated (Task 3).
pub use crate::edit::history::*;
pub type BuildHistory = crate::edit::history::EditHistory;
```

- [ ] **Step 3: Verify compile**

Run: `cargo check`
Expected: clean compile. Tests still pass: `cargo test`.

- [ ] **Step 4: Commit**

```bash
git add src/edit/history.rs src/building/history.rs
git commit -m "refactor(edit): move history body to edit/, leave shim in building/"
```

---

## Task 3: Update all `BuildHistory` call sites to `EditHistory`

**Files (find with `grep -rn BuildHistory --include='*.rs'`):**
- Modify: `src/building/mod.rs`
- Modify: `src/building/ui.rs`
- Modify: `src/save.rs` (if referenced)
- Modify: `src/memory/verbs.rs` (if referenced)
- Delete: `src/building/history.rs` (shim no longer needed at end)

- [ ] **Step 1: Replace all references**

For each file in the list, replace:
- `BuildHistory` -> `EditHistory`
- `crate::building::history::BuildHistory` -> `crate::edit::EditHistory`
- `crate::building::BuildHistory` -> `crate::edit::EditHistory`
- `building::history::apply_undo` etc. -> `crate::edit::apply_undo` etc.

Keep the public re-export in `src/building/mod.rs` if other code uses `crate::building::BuildHistory` -- update it to point at `crate::edit::EditHistory`.

- [ ] **Step 2: Delete the shim**

Remove `src/building/history.rs`. Remove the `pub mod history;` line in `src/building/mod.rs`. Remove any `pub use history::...` re-exports in `src/building/mod.rs` -- they live in `crate::edit` now.

- [ ] **Step 3: Verify compile**

Run: `cargo check`
Expected: clean compile.

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 4: Manual smoke check**

Run: `cargo run`
Build a wall, undo, redo. Verify Ctrl+Z / Ctrl+Shift+Z still work. Hit Esc, exit the game.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(edit): rename BuildHistory -> EditHistory across codebase"
```

---

## Task 4: Move `PlacedBuilding` -> `PlacedItem` body into `src/edit/placed_item.rs`

**Files:**
- Modify: `src/edit/placed_item.rs`
- Modify: `src/building/mod.rs` (re-export shim)

- [ ] **Step 1: Define `PlacedItem` in `src/edit/placed_item.rs`**

Replace placeholder with:
```rust
use bevy::prelude::*;
use crate::items::ItemId;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct PlacedItem {
    pub item: ItemId,
}
```

(Match the original `PlacedBuilding` derives. If the original used `#[reflect(Save)]` with moonshine, mirror it here; check `src/building/mod.rs` for the original derive set and copy verbatim.)

- [ ] **Step 2: Update `edit/history.rs` to use `PlacedItem`**

In `src/edit/history.rs`, replace the temporary `use crate::building::PlacedBuilding;` line with:
```rust
use super::placed_item::PlacedItem;
```
And replace `PlacedBuilding` -> `PlacedItem` in this file's body.

- [ ] **Step 3: Add a re-export shim in `src/building/mod.rs`**

Replace the existing `PlacedBuilding` definition with:
```rust
pub use crate::edit::PlacedItem as PlacedBuilding;
```

- [ ] **Step 4: Verify compile**

Run: `cargo check`
Expected: clean compile.

Run: `cargo test`
Expected: tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/edit/placed_item.rs src/edit/history.rs src/building/mod.rs
git commit -m "refactor(edit): move PlacedBuilding to edit::PlacedItem with shim"
```

---

## Task 5: Update all `PlacedBuilding` call sites to `PlacedItem`

**Files (find with `grep -rn PlacedBuilding --include='*.rs'`):**
- Modify: `src/building/mod.rs`
- Modify: `src/building/ui.rs`
- Modify: `src/camera/occluder_fade.rs`
- Modify: `src/input/mod.rs`
- Modify: `src/save.rs`
- Modify: `src/memory/verbs.rs`

- [ ] **Step 1: Replace references**

Across each file:
- `PlacedBuilding` -> `PlacedItem`
- `use crate::building::PlacedBuilding;` -> `use crate::edit::PlacedItem;`
- `&Query<(&Transform, &PlacedBuilding), ...>` -> `&Query<(&Transform, &PlacedItem), ...>`

If `save.rs` references `PlacedBuilding` by string (moonshine save keys on type name), confirm the save format still works -- moonshine reflection keys on the type's `TypePath`. Search for `PlacedBuilding` in any `.json` save fixture and rename if found.

- [ ] **Step 2: Remove the shim**

Delete the `pub use crate::edit::PlacedItem as PlacedBuilding;` shim in `src/building/mod.rs`.

- [ ] **Step 3: Verify compile**

Run: `cargo check`
Expected: clean compile.

Run: `cargo test`
Expected: tests pass.

- [ ] **Step 4: Manual save / load smoke check**

Run: `cargo run`
- Place a wall.
- F5 to save (or wait for auto-save).
- Quit (Esc to menu, exit).
- Run again, load the save.
- Verify the wall is still there.

If the save fails to load, `PlacedItem`'s `TypePath` differs from `PlacedBuilding`. Add a custom `TypePath` impl or a serde rename. Document the fix in the commit message.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(edit): rename PlacedBuilding -> PlacedItem across codebase"
```

---

## Task 6: Extract `building/placement.rs` from `building/mod.rs`

**Files:**
- Create: `src/building/placement.rs`
- Modify: `src/building/mod.rs`

The goal is to slim `building/mod.rs` by moving the cube-grid placement helpers into a sibling. We do not change behavior.

- [ ] **Step 1: Create `src/building/placement.rs`**

Move these items from `building/mod.rs` into `building/placement.rs`:
- `compute_placement`
- `resolve_chain`
- `wall_segment_transforms`
- `segment_end`
- `anchor_from_hit`
- `is_position_occupied`
- `cube_target_width`
- `OCCUPIED_RADIUS`, `OCCUPIED_Y`, `WALL_LENGTH` constants
- `snap_axis`
- `footprint_cell_centres`

Add `pub mod placement;` near the top of `src/building/mod.rs`. Re-export the public items via:
```rust
pub use placement::{compute_placement, anchor_from_hit, resolve_chain,
                    wall_segment_transforms, segment_end, snap_axis,
                    is_position_occupied, footprint_cell_centres,
                    cube_target_width, OCCUPIED_RADIUS, OCCUPIED_Y, WALL_LENGTH};
```

Imports inside `placement.rs`:
```rust
use bevy::prelude::*;
use crate::edit::PlacedItem;
use crate::input::CursorHit;
use crate::items::{Form, ItemRegistry};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;
use super::BuildPreview;
```

- [ ] **Step 2: Verify compile**

Run: `cargo check`
Expected: clean compile.

Run: `cargo test`
Expected: tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/building/placement.rs src/building/mod.rs
git commit -m "refactor(building): extract placement helpers to placement.rs"
```

---

## Task 7: Move interior-asset helpers to a temporary `building/interior.rs`

**Files:**
- Create: `src/building/interior.rs`
- Modify: `src/building/mod.rs`

The interior code will eventually live in `decoration/interior.rs` (Task 11). Stage it in `building/` first to keep diffs reviewable.

- [ ] **Step 1: Create `src/building/interior.rs`**

Move these from `building/mod.rs`:
- `InteriorSpawnRequest` component
- `resolve_interior_spawns` system
- `interior_render_params` helper
- `compute_interior_placement`
- `BlockingRule`, `blocking_rule_for`, `footprint_clear`

Add `pub mod interior;` to `src/building/mod.rs`.

Update the system register in `BuildingPlugin::build` to use `interior::resolve_interior_spawns`.

Imports inside `interior.rs`:
```rust
use bevy::gltf::Gltf;
use bevy::prelude::*;
use crate::edit::PlacedItem;
use crate::input::CursorHit;
use crate::items::{AabbBounds, Form, InteriorCatalog, ItemDef, ItemRegistry};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;
use super::placement::{snap_axis, cube_target_width, OCCUPIED_RADIUS, OCCUPIED_Y};
use super::BuildPreview;
```

- [ ] **Step 2: Verify compile**

Run: `cargo check`
Run: `cargo test`
Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add src/building/interior.rs src/building/mod.rs
git commit -m "refactor(building): stage interior helpers in interior.rs (pre-decoration move)"
```

---

## Task 8: Move catalog UI to a temporary `building/catalog_ui.rs`

**Files:**
- Create: `src/building/catalog_ui.rs`
- Modify: `src/building/ui.rs`

- [ ] **Step 1: Create `src/building/catalog_ui.rs`**

Move from `building/ui.rs`:
- `DecorationCatalogState` resource
- `draw_decoration_catalog` system
- `THUMB_SIZE` constant
- `category_label` helper

Update `register` in `building/ui.rs` so it adds `catalog_ui::draw_decoration_catalog` to the `EguiPrimaryContextPass` schedule.

Add `pub mod catalog_ui;` to `src/building/mod.rs` (or where the `ui` mod is declared).

- [ ] **Step 2: Verify compile**

Run: `cargo check`
Run: `cargo test`
Expected: clean.

Manual: `cargo run`, press B, confirm catalog still shows on the right.

- [ ] **Step 3: Commit**

```bash
git add src/building/catalog_ui.rs src/building/ui.rs src/building/mod.rs
git commit -m "refactor(building): stage catalog UI in catalog_ui.rs (pre-decoration move)"
```

---

## Task 9: Scaffold `src/decoration/` plugin with `N` key toggle (no-op)

**Files:**
- Create: `src/decoration/mod.rs`
- Create: `src/decoration/placement.rs` (placeholder)
- Create: `src/decoration/interior.rs` (placeholder)
- Create: `src/decoration/move_tool.rs` (placeholder)
- Create: `src/decoration/catalog_ui.rs` (placeholder)
- Create: `src/decoration/hotbar_ui.rs` (placeholder)
- Modify: `src/main.rs`
- Modify: `src/input/mod.rs` (new action)

- [ ] **Step 1: Add a new input action `ToggleDecoration`**

In `src/input/mod.rs`, mirror the existing `ToggleBuild` definition:
- Add `ToggleDecoration` to the `Action` enum.
- In the `default_input_map`, bind `KeyCode::KeyN` and an unused gamepad button (e.g. `GamepadButton::West`).

- [ ] **Step 2: Create `src/decoration/mod.rs`**

```rust
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

pub mod catalog_ui;
pub mod hotbar_ui;
pub mod interior;
pub mod move_tool;
pub mod placement;

use crate::input::{Action, CursorState};

pub struct DecorationPlugin;

impl Plugin for DecorationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, toggle_decoration_mode);
    }
}

#[derive(Resource, Default)]
pub struct DecorationMode {
    pub tool: DecorationTool,
    pub selected: usize,
    pub rotation_radians: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DecorationTool {
    #[default]
    Place,
    Move,
    Remove,
}

fn toggle_decoration_mode(
    mut commands: Commands,
    action_state: Res<ActionState<Action>>,
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
) {
    if cursor.keyboard_over_ui {
        return;
    }
    if !action_state.just_pressed(&Action::ToggleDecoration) {
        return;
    }
    match decoration_mode {
        Some(_) => commands.remove_resource::<DecorationMode>(),
        None => commands.insert_resource(DecorationMode::default()),
    }
}
```

- [ ] **Step 3: Create placeholder sub-modules**

Each of `placement.rs`, `interior.rs`, `move_tool.rs`, `catalog_ui.rs`, `hotbar_ui.rs`:
```rust
//! Placeholder -- populated in subsequent tasks.
```

- [ ] **Step 4: Mutual exclusion with `BuildMode`**

In `src/decoration/mod.rs::toggle_decoration_mode`, after inserting `DecorationMode`:
```rust
None => {
    commands.remove_resource::<crate::building::BuildMode>();
    commands.insert_resource(DecorationMode::default());
}
```

In `src/building/mod.rs::toggle_build_mode`, when inserting `BuildMode`, add:
```rust
commands.remove_resource::<crate::decoration::DecorationMode>();
```

- [ ] **Step 5: Register plugin in `main.rs`**

Add `pub mod decoration;` near other modules. Add `.add_plugins(crate::decoration::DecorationPlugin)` after `BuildingPlugin`.

- [ ] **Step 6: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`. Press `B` -- build mode on (catalog visible). Press `N` -- catalog disappears (build mode off, decoration mode on but no UI yet). Press `B` again -- catalog reappears.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(decoration): scaffold DecorationPlugin with N-key mutual-exclusive toggle"
```

---

## Task 10: Move interior helpers from `building/interior.rs` to `decoration/interior.rs`

**Files:**
- Replace: `src/decoration/interior.rs` (with content from `src/building/interior.rs`)
- Delete: `src/building/interior.rs`
- Modify: `src/building/mod.rs` (system register, imports)
- Modify: `src/edit/history.rs` (imports, since history calls into spawn helpers)
- Modify: any other call sites of these helpers

- [ ] **Step 1: Move file content**

Copy `src/building/interior.rs` content into `src/decoration/interior.rs`. Update internal imports:
- `super::placement::...` -> `crate::building::placement::...` (still using building's snap helpers for now -- this is the pre-decoration-physics state)
- `super::BuildPreview` -> `crate::building::BuildPreview`

- [ ] **Step 2: Delete `src/building/interior.rs`**

Remove the file. Remove `pub mod interior;` from `src/building/mod.rs`. Remove the `interior::resolve_interior_spawns` add_system in `BuildingPlugin` -- move it to `DecorationPlugin::build`:
```rust
app.add_systems(Update, (
    toggle_decoration_mode,
    interior::resolve_interior_spawns,
));
```

- [ ] **Step 3: Update import paths**

Search for `crate::building::interior` -> `crate::decoration::interior`. Search for `building::resolve_interior_spawns` -> `decoration::interior::resolve_interior_spawns`. Search for `BlockingRule` -> if used outside the file, qualify as `crate::decoration::interior::BlockingRule`.

Most call sites are probably inside the moved file itself, but check `src/edit/history.rs` and `src/building/mod.rs::place_building` because they spawn interior pieces.

- [ ] **Step 4: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`. Build mode (B), select an interior asset from the catalog (which is still in building/ for now), place it. Confirm it spawns and is visible.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(decoration): move interior helpers from building to decoration"
```

---

## Task 11: Move catalog UI from `building/catalog_ui.rs` to `decoration/catalog_ui.rs`

**Files:**
- Replace: `src/decoration/catalog_ui.rs`
- Delete: `src/building/catalog_ui.rs`
- Modify: `src/building/ui.rs` (drop register call)
- Modify: `src/decoration/mod.rs` (add register call)
- Modify: catalog UI itself -- gate visibility on `DecorationMode` instead of `BuildMode`

- [ ] **Step 1: Move file content**

Copy `src/building/catalog_ui.rs` to `src/decoration/catalog_ui.rs`. Inside:
- Replace `Option<ResMut<BuildMode>>` parameter with `Option<ResMut<crate::decoration::DecorationMode>>`.
- Replace `mode.tool = BuildTool::Place` with `mode.tool = crate::decoration::DecorationTool::Place`.
- Replace `mode.selected = row.idx` -- same field, no change.
- Remove `refresh_build_preview` call -- the decoration ghost preview lands in Task 14. For now, the catalog click just sets `tool` and `selected`; no preview refresh.

- [ ] **Step 2: Register in DecorationPlugin**

In `src/decoration/mod.rs`:
```rust
use bevy_egui::EguiPrimaryContextPass;

impl Plugin for DecorationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (toggle_decoration_mode, interior::resolve_interior_spawns));
        app.add_systems(EguiPrimaryContextPass, catalog_ui::draw_decoration_catalog);
    }
}
```

- [ ] **Step 3: Drop from BuildingPlugin**

In `src/building/ui.rs`, remove the `catalog_ui::draw_decoration_catalog` add_systems call. Remove `pub mod catalog_ui;` declaration.

- [ ] **Step 4: Delete the staging file**

Delete `src/building/catalog_ui.rs`.

- [ ] **Step 5: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`. Press `B` -- catalog should NOT show. Press `N` -- catalog appears on the right. Click a thumbnail -- decoration mode's `selected` updates (no preview yet -- that's Task 14).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(decoration): move catalog UI to decoration mode (gated on DecorationMode)"
```

---

## Task 12: Pure function `snap_to_fine_grid` with TDD

**Files:**
- Modify: `src/decoration/placement.rs`

- [ ] **Step 1: Write failing test**

Replace placeholder content of `src/decoration/placement.rs` with:
```rust
//! Decoration placement -- magnetic-continuous (v1: fine 0.1m grid).

/// Granularity of v1 magnetic snap. 0.1m is fine enough that the grid
/// is invisible at iso zoom but coarse enough that two pieces placed
/// "near each other" line up.
pub const FINE_GRID_STEP: f32 = 0.1;

/// Round a world-space coordinate to the nearest `FINE_GRID_STEP`.
pub fn snap_to_fine_grid(value: f32) -> f32 {
    (value / FINE_GRID_STEP).round() * FINE_GRID_STEP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_zero_to_zero() {
        assert_eq!(snap_to_fine_grid(0.0), 0.0);
    }

    #[test]
    fn rounds_to_nearest_tenth() {
        assert!((snap_to_fine_grid(0.34) - 0.3).abs() < 1e-5);
        assert!((snap_to_fine_grid(0.36) - 0.4).abs() < 1e-5);
    }

    #[test]
    fn rounds_negative_correctly() {
        assert!((snap_to_fine_grid(-0.34) + 0.3).abs() < 1e-5);
    }

    #[test]
    fn already_on_grid_unchanged() {
        assert!((snap_to_fine_grid(1.5) - 1.5).abs() < 1e-5);
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --bin bevy-cat-game decoration::placement`
Expected: all four pass (because the implementation is in the same step).

If the test runner doesn't pick up bin tests, run: `cargo test`. Filter manually for `snap_to_fine_grid`.

- [ ] **Step 3: Commit**

```bash
git add src/decoration/placement.rs
git commit -m "feat(decoration): fine-grid snap (0.1m) with unit tests"
```

---

## Task 13: Pure function `quantize_rotation` (15 deg) with TDD

**Files:**
- Modify: `src/decoration/placement.rs`

- [ ] **Step 1: Write failing test + implementation**

Append to `src/decoration/placement.rs`:
```rust
use std::f32::consts::PI;

/// 15-degree rotation step in radians for decoration mode.
pub const ROTATION_STEP_RADIANS: f32 = PI / 12.0;

/// Round `radians` to the nearest multiple of `ROTATION_STEP_RADIANS`.
/// Used by R / Shift+R when Alt is not held.
pub fn quantize_rotation(radians: f32) -> f32 {
    (radians / ROTATION_STEP_RADIANS).round() * ROTATION_STEP_RADIANS
}

#[cfg(test)]
mod rotation_tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn zero_unchanged() {
        assert!((quantize_rotation(0.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn fifteen_degrees_unchanged() {
        let fifteen = PI / 12.0;
        assert!((quantize_rotation(fifteen) - fifteen).abs() < 1e-5);
    }

    #[test]
    fn snaps_eighteen_to_fifteen() {
        let eighteen = 18.0_f32.to_radians();
        let fifteen = 15.0_f32.to_radians();
        assert!((quantize_rotation(eighteen) - fifteen).abs() < 1e-4);
    }

    #[test]
    fn snaps_thirty_to_thirty() {
        let thirty = 30.0_f32.to_radians();
        assert!((quantize_rotation(thirty) - thirty).abs() < 1e-4);
    }

    #[test]
    fn negative_quantizes() {
        let minus_fifteen = -15.0_f32.to_radians();
        let minus_eighteen = -18.0_f32.to_radians();
        assert!((quantize_rotation(minus_eighteen) - minus_fifteen).abs() < 1e-4);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: all 5 pass.

- [ ] **Step 3: Commit**

```bash
git add src/decoration/placement.rs
git commit -m "feat(decoration): 15-degree rotation quantize with unit tests"
```

---

## Task 14: `AttachSurface` enum + `pick_attach_surface` decision logic with TDD

**Files:**
- Modify: `src/decoration/placement.rs`

- [ ] **Step 1: Write the surface-picking pure function with tests**

Append to `src/decoration/placement.rs`:
```rust
use bevy::math::Vec3;

/// What surface a decoration item is attaching to. Drives Y placement
/// and (for walls) facing rotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttachSurface {
    /// Hit terrain or a non-PlacedItem entity. Y comes from terrain sample.
    Terrain { xz: Vec3 },
    /// Hit a floor's top face. Y is the floor's top.
    FloorTop { xz: Vec3, top_y: f32 },
    /// Hit a wall's side face. Item's back faces normal; Y is mid-wall.
    WallFace { point: Vec3, normal: Vec3 },
    /// Hit non-floor placed-item top face (table top, chest top).
    FurnitureTop { xz: Vec3, top_y: f32 },
}

/// Hit input shape -- decoupled from CursorHit so this stays pure.
#[derive(Clone, Copy, Debug)]
pub struct AttachInput {
    pub point: Vec3,
    pub normal: Vec3,
    pub kind: AttachInputKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AttachInputKind {
    Terrain,
    Floor { top_y: f32 },
    OtherPlaced { top_y: f32 },
}

/// Decide attach surface from a hit. Priority: floor-top -> wall-face ->
/// furniture-top -> terrain. Wall faces are detected by an upward-facing
/// normal close to horizontal (`|normal.y| < 0.3`); floors are detected
/// by `kind == Floor`.
pub fn pick_attach_surface(input: AttachInput) -> AttachSurface {
    let xz = Vec3::new(input.point.x, 0.0, input.point.z);
    match input.kind {
        AttachInputKind::Terrain => AttachSurface::Terrain { xz },
        AttachInputKind::Floor { top_y } => AttachSurface::FloorTop { xz, top_y },
        AttachInputKind::OtherPlaced { top_y } => {
            if input.normal.y.abs() < 0.3 {
                AttachSurface::WallFace { point: input.point, normal: input.normal }
            } else if input.normal.y > 0.7 {
                AttachSurface::FurnitureTop { xz, top_y }
            } else {
                AttachSurface::Terrain { xz }
            }
        }
    }
}

#[cfg(test)]
mod attach_tests {
    use super::*;
    use bevy::math::Vec3;

    fn input(point: Vec3, normal: Vec3, kind: AttachInputKind) -> AttachInput {
        AttachInput { point, normal, kind }
    }

    #[test]
    fn terrain_routes_to_terrain() {
        let r = pick_attach_surface(input(Vec3::new(1.5, 0.3, 2.5), Vec3::Y, AttachInputKind::Terrain));
        match r {
            AttachSurface::Terrain { xz } => {
                assert_eq!(xz, Vec3::new(1.5, 0.0, 2.5));
            }
            _ => panic!("expected Terrain, got {:?}", r),
        }
    }

    #[test]
    fn floor_top_routes_to_floor_top() {
        let r = pick_attach_surface(input(Vec3::new(0.5, 0.06, 0.5), Vec3::Y, AttachInputKind::Floor { top_y: 0.12 }));
        match r {
            AttachSurface::FloorTop { top_y, .. } => assert_eq!(top_y, 0.12),
            _ => panic!("expected FloorTop, got {:?}", r),
        }
    }

    #[test]
    fn wall_face_normal_routes_to_wall_face() {
        let r = pick_attach_surface(input(
            Vec3::new(1.0, 0.5, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            AttachInputKind::OtherPlaced { top_y: 1.0 },
        ));
        assert!(matches!(r, AttachSurface::WallFace { .. }));
    }

    #[test]
    fn furniture_top_routes_to_furniture_top() {
        let r = pick_attach_surface(input(
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            AttachInputKind::OtherPlaced { top_y: 0.5 },
        ));
        assert!(matches!(r, AttachSurface::FurnitureTop { .. }));
    }

    #[test]
    fn slanted_normal_falls_back_to_terrain() {
        let r = pick_attach_surface(input(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.5, 0.5, 0.0).normalize(),
            AttachInputKind::OtherPlaced { top_y: 1.0 },
        ));
        assert!(matches!(r, AttachSurface::Terrain { .. }));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: all 5 attach tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/decoration/placement.rs
git commit -m "feat(decoration): AttachSurface picker with unit tests"
```

---

## Task 15: Wire up `DecorationGhost` preview spawning

**Files:**
- Modify: `src/decoration/mod.rs`
- Modify: `src/decoration/placement.rs`

The decoration mode needs a ghost entity that follows the cursor showing where the click would land. Mirror the pattern from `building::update_preview` but with magnetic-continuous placement.

- [ ] **Step 1: Add `DecorationPreview` marker**

In `src/decoration/placement.rs`, append:
```rust
use bevy::prelude::*;

/// Marker for the decoration ghost preview entity. One at a time.
#[derive(Component)]
pub struct DecorationPreview;
```

- [ ] **Step 2: Add `update_preview` system**

In `src/decoration/placement.rs`, append (omit any sections marked TODO -- they get filled in Tasks 16-17):
```rust
use crate::edit::PlacedItem;
use crate::input::{CursorHit, CursorState};
use crate::items::{Form, ItemRegistry};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;
use super::{DecorationMode, DecorationTool};

pub fn update_preview(
    mut commands: Commands,
    mut decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
    placed_q: Query<(&Transform, &PlacedItem), Without<DecorationPreview>>,
    registry: Res<ItemRegistry>,
    terrain: Res<Terrain>,
    noise: Res<WorldNoise>,
    placeables: Res<crate::building::PlaceableItems>,
    mut preview_q: Query<(Entity, &mut Transform), With<DecorationPreview>>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut mode) = decoration_mode else {
        // Mode off -- despawn any lingering preview.
        for (e, _) in &preview_q {
            commands.entity(e).despawn();
        }
        return;
    };
    if !matches!(mode.tool, DecorationTool::Place) {
        for (e, _) in &preview_q {
            commands.entity(e).despawn();
        }
        return;
    }
    let Some(item_id) = placeables.0.get(mode.selected).copied() else { return };
    let Some(def) = registry.get(item_id) else { return };

    // Compute placement position (uses snap_to_fine_grid + pick_attach_surface).
    let pos = compute_decoration_placement(
        cursor.cursor_world.unwrap_or(Vec3::ZERO),
        cursor.cursor_hit,
        def,
        &placed_q,
        &registry,
        &terrain,
        &noise,
    );

    // Spawn or move the preview.
    if let Ok((_, mut tf)) = preview_q.single_mut() {
        tf.translation = pos;
        tf.rotation = Quat::from_rotation_y(mode.rotation_radians);
    } else {
        let mesh = meshes.add(def.form.make_mesh());
        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(0.4, 0.9, 0.6, 0.4),
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        commands.spawn((
            DecorationPreview,
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_y(mode.rotation_radians)),
        ));
    }
}

/// Top-level placement decision. Calls `pick_attach_surface` then snaps
/// XZ via `snap_to_fine_grid`. v1 -- no magnet anchors.
pub fn compute_decoration_placement(
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    def: &crate::items::ItemDef,
    placed_q: &Query<(&Transform, &PlacedItem), Without<DecorationPreview>>,
    registry: &ItemRegistry,
    terrain: &Terrain,
    noise: &WorldNoise,
) -> Vec3 {
    let lift = def.form.placement_lift();

    // Build the AttachInput from the rapier hit.
    let input = if let Some(hit) = cursor_hit {
        if let Ok((tf, building)) = placed_q.get(hit.entity) {
            let hit_def = registry.get(building.item);
            let top_y = tf.translation.y + hit_def.map(|d| d.form.placement_lift()).unwrap_or(0.0);
            let kind = if hit_def.map_or(false, |d| matches!(d.form, Form::Floor)) {
                AttachInputKind::Floor { top_y }
            } else {
                AttachInputKind::OtherPlaced { top_y }
            };
            AttachInput { point: hit.point, normal: hit.normal, kind }
        } else {
            AttachInput { point: hit.point, normal: hit.normal, kind: AttachInputKind::Terrain }
        }
    } else {
        AttachInput {
            point: cursor_world,
            normal: Vec3::Y,
            kind: AttachInputKind::Terrain,
        }
    };

    let surface = pick_attach_surface(input);
    match surface {
        AttachSurface::Terrain { xz } => {
            let x = snap_to_fine_grid(xz.x);
            let z = snap_to_fine_grid(xz.z);
            let y = terrain.height_at_or_sample(x, z, noise);
            Vec3::new(x, y + lift, z)
        }
        AttachSurface::FloorTop { xz, top_y } => {
            let x = snap_to_fine_grid(xz.x);
            let z = snap_to_fine_grid(xz.z);
            Vec3::new(x, top_y + lift, z)
        }
        AttachSurface::FurnitureTop { xz, top_y } => {
            let x = snap_to_fine_grid(xz.x);
            let z = snap_to_fine_grid(xz.z);
            Vec3::new(x, top_y + lift, z)
        }
        AttachSurface::WallFace { point, normal } => {
            // Push the item ~0.05m off the wall along the normal so it
            // doesn't z-fight with the wall surface. XZ snaps to fine grid
            // along the wall plane; Y is left at the hit height.
            let off = normal.normalize() * 0.05;
            let world = point + off;
            let x = snap_to_fine_grid(world.x);
            let z = snap_to_fine_grid(world.z);
            Vec3::new(x, world.y, z)
        }
    }
}
```

- [ ] **Step 3: Register the system**

In `src/decoration/mod.rs`, add `placement::update_preview` to the Update systems:
```rust
app.add_systems(Update, (
    toggle_decoration_mode,
    placement::update_preview,
    interior::resolve_interior_spawns,
));
```

- [ ] **Step 4: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`. Press `N`. Click a thumbnail in the catalog. Move the cursor -- a translucent green ghost of the piece should follow. Move it onto a floor -- ghost rests on the floor. Move onto terrain -- ghost rests on terrain.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(decoration): magnetic-continuous v1 ghost preview (fine grid)"
```

---

## Task 16: Place tool -- consume click, spawn the piece

**Files:**
- Create: `src/decoration/place_tool.rs`
- Modify: `src/decoration/mod.rs`

- [ ] **Step 1: Add place system**

`src/decoration/place_tool.rs`:
```rust
use bevy::prelude::*;
use crate::edit::{EditHistory, BuildOp, PieceRef, PlacedItem};
use crate::input::{CursorHit, CursorState};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{Form, InteriorCatalog, ItemRegistry};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;
use super::placement::{compute_decoration_placement, DecorationPreview};
use super::{DecorationMode, DecorationTool};

const INFINITE_RESOURCES: bool = true; // Mirror building's cheat.

#[allow(clippy::too_many_arguments)]
pub fn place_decoration(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
    placed_q: Query<(&Transform, &PlacedItem), Without<DecorationPreview>>,
    registry: Res<ItemRegistry>,
    terrain: Res<Terrain>,
    noise: Res<WorldNoise>,
    placeables: Res<crate::building::PlaceableItems>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    catalog: Res<InteriorCatalog>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    mut history: ResMut<EditHistory>,
) {
    let Some(mode) = decoration_mode else { return };
    if cursor.keyboard_over_ui || cursor.mouse_over_ui {
        return;
    }
    if !matches!(mode.tool, DecorationTool::Place) {
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(item_id) = placeables.0.get(mode.selected).copied() else { return };
    let Some(def) = registry.get(item_id) else { return };

    if !INFINITE_RESOURCES && inventory.count(item_id) == 0 {
        return;
    }

    let pos = compute_decoration_placement(
        cursor.cursor_world.unwrap_or(Vec3::ZERO),
        cursor.cursor_hit,
        def,
        &placed_q,
        &registry,
        &terrain,
        &noise,
    );
    let tf = Transform::from_translation(pos)
        .with_rotation(Quat::from_rotation_y(mode.rotation_radians));

    // Spawn -- delegate to the same path the building module uses so visual
    // and collision behavior matches.
    let entity = crate::building::spawn_placed_building(
        &mut commands,
        item_id,
        tf,
        def,
        &asset_server,
        &mut meshes,
        &mut materials,
        &catalog,
    );

    if !INFINITE_RESOURCES {
        inventory.remove(item_id, 1);
        inv_events.write(InventoryChanged);
    }

    history.record(BuildOp::Placed(vec![PieceRef {
        item: item_id,
        transform: tf,
        entity: Some(entity),
    }]));
}
```

- [ ] **Step 2: Register**

In `src/decoration/mod.rs`:
```rust
pub mod place_tool;
// ...
app.add_systems(Update, (
    toggle_decoration_mode,
    placement::update_preview,
    place_tool::place_decoration,
    interior::resolve_interior_spawns,
));
```

- [ ] **Step 3: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`. Press `N`. Click a chair in the catalog. Move cursor onto floor. Click. A chair spawns. Click again -- another chair spawns at the new cursor position (sticky). Press Ctrl+Z -- the most recently placed chair vanishes.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(decoration): Place tool spawns pieces via magnetic v1 placement"
```

---

## Task 17: Rotation hotkey (`R` / `Shift+R` / `Alt+R`)

**Files:**
- Create: `src/decoration/rotation.rs`
- Modify: `src/decoration/mod.rs`

- [ ] **Step 1: Add rotation system**

`src/decoration/rotation.rs`:
```rust
use bevy::prelude::*;
use crate::input::CursorState;
use super::placement::{quantize_rotation, ROTATION_STEP_RADIANS};
use super::DecorationMode;

/// Continuous rotation rate (radians per second) when Alt+R is held.
const FREE_ROTATE_RATE: f32 = std::f32::consts::PI; // 180 deg/s

pub fn rotate_decoration(
    keys: Res<ButtonInput<KeyCode>>,
    decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
    time: Res<Time>,
) {
    let Some(mut mode) = decoration_mode else { return };
    if cursor.keyboard_over_ui {
        return;
    }
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    if alt {
        // Continuous rotation while R / Shift+R held.
        let dir = if keys.pressed(KeyCode::KeyR) {
            if shift { -1.0 } else { 1.0 }
        } else {
            0.0
        };
        if dir != 0.0 {
            mode.rotation_radians += dir * FREE_ROTATE_RATE * time.delta_secs();
        }
    } else if keys.just_pressed(KeyCode::KeyR) {
        // Stepped rotation by 15 deg.
        let step = if shift { -ROTATION_STEP_RADIANS } else { ROTATION_STEP_RADIANS };
        mode.rotation_radians = quantize_rotation(mode.rotation_radians + step);
    }
}
```

- [ ] **Step 2: Register**

In `src/decoration/mod.rs`:
```rust
pub mod rotation;
// in build():
app.add_systems(Update, (
    toggle_decoration_mode,
    placement::update_preview,
    place_tool::place_decoration,
    rotation::rotate_decoration,
    interior::resolve_interior_spawns,
));
```

- [ ] **Step 3: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`. Press `N`, pick a chair. Press `R` -- ghost rotates 15deg. Hold `Alt+R` -- ghost rotates smoothly. Press `Shift+R` -- rotates the other way 15deg.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(decoration): R / Shift+R quantized rotation, Alt+R continuous"
```

---

## Task 18: Move tool -- pickup phase

**Files:**
- Modify: `src/decoration/move_tool.rs` (replaces placeholder)
- Modify: `src/decoration/mod.rs`

The Move tool has two states: idle (waiting for click on a placed piece) and held (carrying an entity, waiting for click to drop). Pickup runs in this task; drop in Task 19.

- [ ] **Step 1: Define Move tool resource**

In `src/decoration/move_tool.rs`:
```rust
use bevy::prelude::*;
use crate::edit::PlacedItem;
use crate::input::{CursorHit, CursorState};
use crate::items::{ItemRegistry, ItemTags};
use super::{DecorationMode, DecorationTool};

/// Carry state for the Move tool. `Some(entity)` while a piece is held.
#[derive(Resource, Default)]
pub struct MoveCarry(pub Option<Entity>);

pub fn pickup_decoration(
    mouse: Res<ButtonInput<MouseButton>>,
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    mut carry: ResMut<MoveCarry>,
    placed_q: Query<&PlacedItem>,
    registry: Res<ItemRegistry>,
) {
    let Some(mode) = decoration_mode else { return };
    if !matches!(mode.tool, DecorationTool::Move) {
        return;
    }
    if cursor.mouse_over_ui {
        return;
    }
    if carry.0.is_some() {
        // Already carrying -- drop happens in Task 19.
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(hit) = cursor.cursor_hit else { return };
    let Ok(building) = placed_q.get(hit.entity) else { return };
    let Some(def) = registry.get(building.item) else { return };

    // Only decoration items are pickable. Walls / floors / etc. belong to build's verbs.
    let is_decor = def.tags.contains(ItemTags::DECORATION)
        || def.tags.contains(ItemTags::FURNITURE);
    if !is_decor {
        return;
    }

    carry.0 = Some(hit.entity);
}
```

- [ ] **Step 2: Register**

In `src/decoration/mod.rs::DecorationPlugin::build`:
```rust
app.init_resource::<move_tool::MoveCarry>();
app.add_systems(Update, (
    toggle_decoration_mode,
    placement::update_preview,
    place_tool::place_decoration,
    move_tool::pickup_decoration,
    rotation::rotate_decoration,
    interior::resolve_interior_spawns,
));
```

- [ ] **Step 3: Verify**

Run: `cargo check`. Manual: `cargo run`. Press `N`, switch to Move tool (number 2 hotkey -- to be added in Task 22; for now switch via direct mode mutation in code or skip the manual check until 22).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(decoration): Move tool pickup phase (carry an entity, decor-only)"
```

---

## Task 19: Move tool -- drop phase + held-entity follows cursor

**Files:**
- Modify: `src/decoration/move_tool.rs`
- Modify: `src/decoration/mod.rs`

- [ ] **Step 1: Add drop system + carry follow**

Append to `src/decoration/move_tool.rs`:
```rust
use crate::edit::{EditHistory, BuildOp, PieceRef};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;
use super::placement::{compute_decoration_placement, DecorationPreview};

#[allow(clippy::too_many_arguments)]
pub fn carry_follow_cursor(
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    carry: Res<MoveCarry>,
    placed_q: Query<(&Transform, &PlacedItem), Without<DecorationPreview>>,
    registry: Res<ItemRegistry>,
    terrain: Res<Terrain>,
    noise: Res<WorldNoise>,
    mut tfs: Query<&mut Transform>,
) {
    let Some(mode) = decoration_mode else { return };
    if !matches!(mode.tool, DecorationTool::Move) {
        return;
    }
    let Some(entity) = carry.0 else { return };
    let Ok((_, building)) = placed_q.get(entity) else { return };
    let Some(def) = registry.get(building.item) else { return };

    let pos = compute_decoration_placement(
        cursor.cursor_world.unwrap_or(Vec3::ZERO),
        cursor.cursor_hit,
        def,
        &placed_q,
        &registry,
        &terrain,
        &noise,
    );
    if let Ok(mut tf) = tfs.get_mut(entity) {
        tf.translation = pos;
        tf.rotation = Quat::from_rotation_y(mode.rotation_radians);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn drop_decoration(
    mouse: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    mut carry: ResMut<MoveCarry>,
) {
    let Some(mode) = decoration_mode else { return };
    if !matches!(mode.tool, DecorationTool::Move) {
        return;
    }
    if carry.0.is_none() {
        return;
    }
    if cursor.mouse_over_ui {
        return;
    }
    // Drop on left click or Esc (cancel = drop in place).
    let drop = mouse.just_pressed(MouseButton::Left) || keyboard.just_pressed(KeyCode::Escape);
    if !drop {
        return;
    }
    carry.0 = None;
    // Note: no inventory delta, no history entry. Move is a free relocation.
    // If we want move-undo later, capture (entity, before_tf, after_tf) here.
}
```

- [ ] **Step 2: Register**

```rust
app.add_systems(Update, (
    toggle_decoration_mode,
    placement::update_preview,
    place_tool::place_decoration,
    move_tool::pickup_decoration,
    move_tool::carry_follow_cursor,
    move_tool::drop_decoration,
    rotation::rotate_decoration,
    interior::resolve_interior_spawns,
));
```

- [ ] **Step 3: Verify**

Run: `cargo check`. Run: `cargo test`. Manual playtest deferred to Task 22 when the Move-tool hotkey is wired.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(decoration): Move tool carry + drop phase"
```

---

## Task 20: Hover highlight system

**Files:**
- Modify: `src/edit/highlight.rs`

- [ ] **Step 1: Hover tint logic**

Replace `src/edit/highlight.rs` placeholder:
```rust
use bevy::prelude::*;
use std::collections::HashMap;

use crate::edit::PlacedItem;
use crate::input::CursorState;

pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoverState>();
        app.add_systems(Update, (apply_hover_highlight,));
    }
}

#[derive(Resource, Default)]
pub struct HoverState {
    /// Currently hovered entity, plus its original material handle so we
    /// can restore on un-hover.
    pub current: Option<(Entity, Handle<StandardMaterial>)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HoverIntent {
    None,
    Move,
    Remove,
}

/// Other modules push a HoverIntent here each frame to request the tint.
/// Cleared by the highlight system after each frame.
#[derive(Resource, Default)]
pub struct HoverRequest {
    pub intent: HoverIntent,
    pub entity: Option<Entity>,
}

impl Default for HoverIntent {
    fn default() -> Self {
        HoverIntent::None
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_hover_highlight(
    mut hover_state: ResMut<HoverState>,
    mut request: ResMut<HoverRequest>,
    mut tinted: Query<&mut MeshMaterial3d<StandardMaterial>, With<PlacedItem>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Determine target tint.
    let target_color = match request.intent {
        HoverIntent::Move => Some(Color::srgba(0.2, 0.85, 0.95, 1.0)),
        HoverIntent::Remove => Some(Color::srgba(0.95, 0.3, 0.3, 1.0)),
        HoverIntent::None => None,
    };

    // Restore previously hovered entity if it changed or intent is None.
    let needs_restore = hover_state.current.as_ref().map_or(false, |(e, _)| {
        request.entity != Some(*e) || target_color.is_none()
    });
    if needs_restore {
        if let Some((e, original)) = hover_state.current.take() {
            if let Ok(mut mat) = tinted.get_mut(e) {
                mat.0 = original;
            }
        }
    }

    // Apply tint to new entity if requested.
    if let (Some(color), Some(entity)) = (target_color, request.entity) {
        if hover_state.current.as_ref().map(|(e, _)| *e) != Some(entity) {
            if let Ok(mut mat) = tinted.get_mut(entity) {
                let original = mat.0.clone();
                let tinted_mat = materials.add(StandardMaterial {
                    base_color: color,
                    ..default()
                });
                mat.0 = tinted_mat;
                hover_state.current = Some((entity, original));
            }
        }
    }

    // Clear request for next frame.
    request.intent = HoverIntent::None;
    request.entity = None;
}
```

- [ ] **Step 2: Register `HoverRequest` resource in EditPlugin**

In `src/edit/mod.rs`:
```rust
impl Plugin for EditPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<highlight::HoverRequest>();
        app.add_plugins(HighlightPlugin);
        history::register(app);
    }
}
```

Add `pub use highlight::{HoverRequest, HoverIntent};` to `src/edit/mod.rs`.

- [ ] **Step 3: Have decoration push hover intent each frame**

In `src/decoration/mod.rs`, add a system:
```rust
fn push_hover_intent(
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    mut request: ResMut<crate::edit::HoverRequest>,
    placed_q: Query<&PlacedItem>,
    registry: Res<ItemRegistry>,
) {
    let Some(mode) = decoration_mode else { return };
    let Some(hit) = cursor.cursor_hit else { return };
    if placed_q.get(hit.entity).is_err() {
        return;
    }
    request.entity = Some(hit.entity);
    request.intent = match mode.tool {
        DecorationTool::Move => crate::edit::HoverIntent::Move,
        DecorationTool::Remove => crate::edit::HoverIntent::Remove,
        DecorationTool::Place => crate::edit::HoverIntent::None,
    };
}
```

Register it before `apply_hover_highlight` in the schedule. Imports: `use crate::edit::PlacedItem;` `use crate::input::CursorState;` `use crate::items::ItemRegistry;`.

- [ ] **Step 4: Verify**

Run: `cargo check`
Run: `cargo test`. Manual playtest in Task 22.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(edit): hover highlight (cyan for Move, red for Remove)"
```

---

## Task 21: Held highlight (gentle pulse on the carried piece)

**Files:**
- Modify: `src/edit/highlight.rs`

- [ ] **Step 1: Pulse logic**

Append to `src/edit/highlight.rs`:
```rust
#[derive(Resource, Default)]
pub struct HeldHighlight(pub Option<Entity>);

fn apply_held_pulse(
    held: Res<HeldHighlight>,
    time: Res<Time>,
    mut tinted: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(entity) = held.0 else { return };
    let Ok(mat_h) = tinted.get(entity) else { return };
    let Some(mat) = materials.get_mut(&mat_h.0) else { return };
    let pulse = (time.elapsed_secs() * 4.0).sin() * 0.5 + 0.5;
    mat.emissive = LinearRgba::from(Color::srgb(0.3 * pulse, 0.6 * pulse, 0.9 * pulse));
}
```

Register in HighlightPlugin:
```rust
impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoverState>();
        app.init_resource::<HeldHighlight>();
        app.add_systems(Update, (apply_hover_highlight, apply_held_pulse));
    }
}
```

- [ ] **Step 2: Decoration sets `HeldHighlight` from `MoveCarry`**

In `src/decoration/move_tool.rs`, add a system:
```rust
pub fn sync_held_highlight(
    carry: Res<MoveCarry>,
    mut held: ResMut<crate::edit::HeldHighlight>,
) {
    held.0 = carry.0;
}
```

Register in `DecorationPlugin::build`.

Add `pub use highlight::{HeldHighlight, HoverIntent, HoverRequest};` to `src/edit/mod.rs`.

- [ ] **Step 3: Verify**

Run: `cargo check`. Run: `cargo test`. Manual playtest in Task 22.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(edit): held highlight pulses emissive on carried piece"
```

---

## Task 22: Decoration hotbar UI (tools + selected thumbnail + recent quickbar)

**Files:**
- Modify: `src/decoration/hotbar_ui.rs`
- Modify: `src/decoration/mod.rs`

- [ ] **Step 1: Hotbar layout + tool hotkeys**

Replace `src/decoration/hotbar_ui.rs`:
```rust
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use leafwing_input_manager::prelude::ActionState;

use crate::edit::EditHistory;
use crate::input::{Action, CursorState};
use crate::items::{InteriorCatalog, ItemRegistry};

use super::{DecorationMode, DecorationTool};

const PARCHMENT: egui::Color32 = egui::Color32::from_rgb(54, 38, 24);
const GOLD: egui::Color32 = egui::Color32::from_rgb(220, 168, 76);
const GOLD_DIM: egui::Color32 = egui::Color32::from_rgb(140, 105, 50);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(172, 158, 130);

pub fn select_tool_hotkeys(
    action_state: Res<ActionState<Action>>,
    decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
) {
    let Some(mut mode) = decoration_mode else { return };
    if cursor.keyboard_over_ui {
        return;
    }
    let slots = [
        (Action::Hotbar1, DecorationTool::Place),
        (Action::Hotbar2, DecorationTool::Move),
        (Action::Hotbar3, DecorationTool::Remove),
    ];
    for (action, tool) in slots {
        if action_state.just_pressed(&action) {
            mode.tool = tool;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_decoration_hotbar(
    mut contexts: EguiContexts,
    decoration_mode: Option<Res<DecorationMode>>,
    placeables: Res<crate::building::PlaceableItems>,
    registry: Res<ItemRegistry>,
    history: Res<EditHistory>,
    catalog: Res<InteriorCatalog>,
) -> Result {
    let Some(mode) = decoration_mode else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let can_undo = history.can_undo();
    let can_redo = history.can_redo();

    egui::Window::new("decoration_hotbar")
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -16.0])
        .collapsible(false).resizable(false).title_bar(false)
        .frame(egui::Frame::default()
            .fill(PARCHMENT)
            .stroke(egui::Stroke::new(2.0, GOLD))
            .inner_margin(egui::Margin::symmetric(14, 10))
            .corner_radius(egui::CornerRadius::same(6)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, tool) in [DecorationTool::Place, DecorationTool::Move, DecorationTool::Remove].iter().enumerate() {
                    let active = *tool == mode.tool;
                    let label = match tool {
                        DecorationTool::Place => format!("{} Place", i+1),
                        DecorationTool::Move => format!("{} Move", i+1),
                        DecorationTool::Remove => format!("{} Remove", i+1),
                    };
                    let text = if active {
                        egui::RichText::new(label).color(GOLD).strong()
                    } else {
                        egui::RichText::new(label).color(TEXT_DIM)
                    };
                    ui.add(egui::Label::new(text).selectable(false));
                }
                ui.separator();
                let item_label = placeables.0.get(mode.selected)
                    .and_then(|id| registry.get(*id))
                    .map(|d| d.display_name.as_str())
                    .unwrap_or("(none)");
                ui.colored_label(GOLD, item_label);
                ui.separator();
                ui.colored_label(GOLD_DIM, "R rotate  |  Esc exit");
                ui.separator();
                ui.add_enabled(can_undo, egui::Label::new(egui::RichText::new("Undo").color(if can_undo { GOLD } else { TEXT_DIM })));
                ui.add_enabled(can_redo, egui::Label::new(egui::RichText::new("Redo").color(if can_redo { GOLD } else { TEXT_DIM })));
            });
        });

    Ok(())
}
```

- [ ] **Step 2: Register**

In `src/decoration/mod.rs::DecorationPlugin::build`:
```rust
use bevy_egui::EguiPrimaryContextPass;
// ...
app.add_systems(EguiPrimaryContextPass, (
    catalog_ui::draw_decoration_catalog,
    hotbar_ui::draw_decoration_hotbar,
));
app.add_systems(Update, (
    toggle_decoration_mode,
    hotbar_ui::select_tool_hotkeys,
    placement::update_preview,
    place_tool::place_decoration,
    move_tool::pickup_decoration,
    move_tool::carry_follow_cursor,
    move_tool::drop_decoration,
    move_tool::sync_held_highlight,
    rotation::rotate_decoration,
    push_hover_intent,
    interior::resolve_interior_spawns,
));
```

- [ ] **Step 3: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`. Press `N`. Hotbar shows at bottom. Press `1` -- Place active. Press `2` -- Move. Press `3` -- Remove. Pick a chair from catalog, place a few. Switch to Move (2), click a chair -- it sticks to cursor with a pulse. Click again -- it drops. Switch to Remove (3), hover a chair -- red tint. Click -- chair removed.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "feat(decoration): bottom hotbar with Place/Move/Remove tools and hotkeys"
```

---

## Task 23: Recent-picks quickbar (in-memory)

**Files:**
- Modify: `src/decoration/mod.rs`
- Modify: `src/decoration/hotbar_ui.rs`
- Modify: `src/decoration/place_tool.rs`

- [ ] **Step 1: Recent picks resource**

In `src/decoration/mod.rs`, add:
```rust
#[derive(Resource, Default)]
pub struct RecentPicks {
    /// Most-recent first. Capped at 8.
    pub items: Vec<crate::items::ItemId>,
}

const MAX_RECENT: usize = 8;

impl RecentPicks {
    pub fn record(&mut self, id: crate::items::ItemId) {
        self.items.retain(|i| *i != id);
        self.items.insert(0, id);
        if self.items.len() > MAX_RECENT {
            self.items.truncate(MAX_RECENT);
        }
    }
}
```

Register: `app.init_resource::<RecentPicks>();`.

- [ ] **Step 2: place_decoration records on placement**

After the `history.record(...)` call in `place_decoration` (`src/decoration/place_tool.rs`), insert:
```rust
recent.record(item_id);
```

Add `mut recent: ResMut<crate::decoration::RecentPicks>` to the system signature.

- [ ] **Step 3: Render quickbar in the hotbar**

In `draw_decoration_hotbar`, after the existing tool buttons, before the rotate hint, insert a new `ui.separator();` and:
```rust
let recent_items: Vec<_> = recent.items.clone();
for item_id in recent_items.iter().take(MAX_RECENT) {
    let Some(def) = registry.get(*item_id) else { continue };
    let label = egui::RichText::new(def.display_name.as_str()).color(TEXT_DIM);
    if ui.add(egui::Button::new(label).small()).clicked() {
        if let Some(idx) = placeables.0.iter().position(|id| *id == *item_id) {
            mode_select_idx = Some(idx);
        }
    }
}
```

To make this work, change the hotbar function signature to also take `Res<RecentPicks>` and `mut decoration_mode: Option<ResMut<DecorationMode>>`, and at the end of the function (after the closure), apply `mode.selected = idx` if `mode_select_idx` is `Some`. Reference `MAX_RECENT` from `super::MAX_RECENT`.

(The thumbnail-based quickbar is a polish refinement -- text labels are the v1.)

- [ ] **Step 4: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`, `N`, place 3 different chairs (e.g. chair_A, chair_B, chair_C). The hotbar quickbar shows three items, most recent first. Click chair_A in the quickbar -- selection switches.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat(decoration): in-memory recent-picks quickbar (8 items)"
```

---

## Task 24: Catalog `[` / `]` cycles within filtered list

**Files:**
- Modify: `src/decoration/catalog_ui.rs`

- [ ] **Step 1: Track filtered list, cycle keys**

In `src/decoration/catalog_ui.rs::draw_decoration_catalog`, after the search box:
- Build the filtered list of placeables (those matching `search_lower`) into a `Vec<usize>` of indices.
- Stash that into a new `Local<Option<Vec<usize>>>` (the cycle uses last frame's filter).

Outside the egui closure (separate system), add:
```rust
pub fn cycle_filtered(
    keys: Res<ButtonInput<KeyCode>>,
    decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
    state: Res<DecorationCatalogState>,
    placeables: Res<crate::building::PlaceableItems>,
    registry: Res<ItemRegistry>,
) {
    let Some(mut mode) = decoration_mode else { return };
    if cursor.keyboard_over_ui {
        return;
    }
    // Build the same filter the catalog uses.
    let q = state.search.to_lowercase();
    let filtered: Vec<usize> = placeables.0.iter().enumerate()
        .filter_map(|(i, id)| {
            let def = registry.get(*id)?;
            if !(def.tags.contains(crate::items::ItemTags::DECORATION)
                || def.tags.contains(crate::items::ItemTags::FURNITURE)) {
                return None;
            }
            if q.is_empty() || def.display_name.to_lowercase().contains(&q) {
                Some(i)
            } else {
                None
            }
        }).collect();
    if filtered.is_empty() { return; }

    let cur_pos = filtered.iter().position(|i| *i == mode.selected);
    if keys.just_pressed(KeyCode::BracketRight) {
        let next = match cur_pos {
            Some(p) => filtered[(p + 1) % filtered.len()],
            None => filtered[0],
        };
        mode.selected = next;
    } else if keys.just_pressed(KeyCode::BracketLeft) {
        let next = match cur_pos {
            Some(p) => filtered[(p + filtered.len() - 1) % filtered.len()],
            None => filtered[0],
        };
        mode.selected = next;
    }
}
```

Register `catalog_ui::cycle_filtered` in the Update systems.

- [ ] **Step 2: Verify**

Run: `cargo check`. Run: `cargo test`. Manual: `cargo run`, `N`, type "lamp" in the catalog search. Press `]` -- selection cycles through the filtered lamps. Press `[` -- cycles backwards.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat(decoration): [/] keys cycle within filtered catalog list"
```

---

## Task 25: Build hotbar -- 6-swatch piece selector

**Files:**
- Modify: `src/building/ui.rs`

- [ ] **Step 1: Replace piece-name label with swatches**

In `src/building/ui.rs::draw_build_tool_hotbar`, the `BuildTool::Place` branch currently shows a "piece: {label}" line. Replace with:
```rust
BuildTool::Place => {
    use crate::items::Form;
    let structural_forms = [Form::Wall, Form::Floor, Form::Door, Form::Window, Form::Roof, Form::Fence];
    for form in structural_forms {
        // Find the placeable index for this form.
        let idx = placeables.0.iter().enumerate()
            .find(|(_, id)| registry.get(**id).map_or(false, |d| d.form == form))
            .map(|(i, _)| i);
        let Some(i) = idx else { continue };
        let active = i == mode.selected;
        let label = form.display_noun();
        let text = if active {
            egui::RichText::new(label).color(GOLD).strong()
        } else {
            egui::RichText::new(label).color(TEXT_DIM)
        };
        // Static label for now; proper clickable swatches are a polish task.
        ui.add(egui::Label::new(text).selectable(false));
    }
}
```

Replace the `[ / ]   shift+click = line` hint with `1-6 select  shift+click line`.

For the next polish, swap `Label` for `egui::SelectableLabel` so swatches are clickable; for now the `[`/`]` and number-row selectors handle it.

- [ ] **Step 2: Restrict `cycle_build_item` to structural forms**

In `src/building/mod.rs::cycle_build_item`, filter `placeables.0` to entries whose form is in the structural set, then cycle that subset.

```rust
let structural: Vec<usize> = placeables.0.iter().enumerate()
    .filter(|(_, id)| registry.get(**id).map_or(false, |d|
        d.tags.contains(crate::items::ItemTags::STRUCTURAL)))
    .map(|(i, _)| i)
    .collect();
```

Then cycle `mode.selected` within `structural` rather than the full list.

- [ ] **Step 3: Verify**

Run: `cargo check`
Run: `cargo test`

Manual: `cargo run`. Press `B`. Hotbar now shows Wall / Floor / Door / Window / Roof / Fence in a row instead of the catalog of every placeable item. Press `]` -- cycles among those six only, never lands on a chair / interior item.

- [ ] **Step 4: Commit**

```bash
git add src/building/ui.rs src/building/mod.rs
git commit -m "feat(building): 6-swatch hotbar restricted to structural forms"
```

---

## Final checkpoint: end-to-end playtest

Run `cargo run` and walk through:

1. **Build mode round-trip.** Press `B`. Place a wall, a floor, a door, a window, a roof, a fence. Each places via the cube grid as before. Press Ctrl+Z, Ctrl+Shift+Z -- undo / redo work.

2. **Mode swap.** Press `N` while still in build mode. Build hotbar disappears, decoration hotbar + right-side catalog appear. Press `B` again -- decoration UI disappears, build hotbar returns.

3. **Decoration place.** In decoration mode, click a thumbnail. Move cursor over a floor inside the room you built -- ghost slides smoothly along the floor (0.1m grid). Click. Click again. Click multiple times -- multiple chairs spawn, sticky placement works.

4. **Decoration on terrain / wall / table.** Click a wall lamp from the catalog, hover over a wall -- ghost rotates to face the wall normal. Click a candle, hover over the table you placed -- ghost rests on the table top.

5. **Move tool.** Press `2`. Hover a chair -- cyan tint. Click -- chair pulses, follows cursor. Click again to drop.

6. **Remove tool.** Press `3`. Hover a chair -- red tint. Click -- chair vanishes, refunded to inventory.

7. **Rotation.** With `Place` selected, press `R` -- ghost rotates 15deg. `Shift+R` -- back. Hold `Alt+R` -- continuous spin.

8. **Catalog cycle.** Type "lamp" in catalog search. Press `]` -- selection cycles through lamps only.

9. **Recent quickbar.** Place 4 different decoration items. Hotbar shows them, most recent first. Click a quickbar entry -- that item is re-selected.

10. **Save / load.** F5 to save. Quit. Re-launch, load. All structural and decoration pieces are still there at their saved positions.

If any of these fail, debug before merging. Each failure traces back to one of the tasks above.

---

## Self-review

**Spec coverage:** every Stage-3 work item from `spec/phases/03-build-feel.md` has a matching task here. W2.S3.1 -> Tasks 1-5; W2.S3.2 -> Tasks 6-8; W2.S3.3 -> Task 9; W2.S3.4 -> Tasks 12-17; W2.S3.5 -> Tasks 22-24; W2.S3.6 -> Tasks 18-19; W2.S3.7 -> Tasks 20-21; W2.S3.8 -> Task 25.

**Placeholder scan:** no TBD / TODO / "fill in details" tokens. Each step has concrete code or commands.

**Type consistency:** `EditHistory`, `PlacedItem`, `DecorationMode`, `DecorationTool`, `MoveCarry`, `HoverState`, `HoverRequest`, `HeldHighlight`, `RecentPicks`, `AttachSurface`, `AttachInput` -- names used uniformly across tasks.

**Open questions deferred:** spec lists five open questions for the plan to resolve. Resolved here:
1. Surface priority order: floor -> wall-by-normal -> furniture-top -> terrain (hard-coded in `pick_attach_surface`).
2. Rotation step: 15 degrees (`PI / 12.0`).
3. Sticky placement: yes for decoration, exits on Esc / right-click.
4. Move tool eligibility: `tags.contains(DECORATION) || tags.contains(FURNITURE)`.
5. Quickbar persistence: in-memory only.
