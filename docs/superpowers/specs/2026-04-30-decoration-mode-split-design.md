# Decoration Mode Split from Build Mode

> Status: Designed (approved 2026-04-30)
> Extends: Phase 2 cube placement (DEC-020)
> ADR: DEC-021

## Why

Today there is one `BuildMode` covering structural placement (walls, floors, doors, windows, roofs) and decoration (furniture, ~1000 LowPoly Interior items). Two complaints surfaced during Phase 2 playtesting:

1. **Wrong physics for decoration.** Cube-grid snap forces a chair into the centre of a 1m cell. Real cozy decoration wants soft, surface-attached placement with continuous freedom along the surface (slide a lamp anywhere on a table top, not "this cell or that cell").
2. **UI clutter.** A single hotbar plus a 1000-item right-side catalog means the player flips between "I am building a wall" and "I am placing a candle" with no contextual cue. Tools, hotkeys, and the catalog all want to behave differently per task.

The fix is a hard split: two distinct modes with different verbs, different placement physics, and different UI.

## Goals

- Two mutually exclusive modes: `BuildMode` (structural, cube-grid) and `DecorationMode` (interiors, surface-glide).
- `B` toggles Build, `N` toggles Decoration. Pressing one while the other is active swaps; pressing the active mode's key again exits.
- Decoration uses **magnetic-continuous** placement: cursor moves the piece smoothly along its attach surface (terrain top, wall face, floor top, furniture top), with magnetic pull toward sensible anchors. Hold `Alt` to break the magnet.
- Decoration mode adds a **Move** verb: pick up a placed piece, carry it on the cursor, drop it elsewhere. No inventory churn.
- Rotation in decoration is 15-degree increments (`R` / `Shift+R`), `Alt` for free continuous.
- Decoration UI: right-side catalog (~1000 thumbnails, search, categories) plus a bottom hotbar with tool buttons + selected-piece thumbnail + recent-picks quickbar (8 slots).
- Build UI: bottom hotbar with tool buttons + 6-swatch piece selector (Wall, Floor, Door, Window, Roof, Fence). No right-side catalog.
- Hovered and held pieces show subtle highlights (cyan for Move, red for Remove, gentle pulse for Held).
- Single shared undo / redo stack across both modes.

## Non-goals

- AI / town crafting that supplies decoration items into player inventory. Decoration placement consumes inventory (or the dev cheat); how items get there is the AI town crafting spec.
- Player-authored recipes for the 1000 interior pack items. Stays gated by `INFINITE_RESOURCES = true` for now.
- Cube-cell save format. Save still serialises `Vec3` transforms, which works for both modes.
- Wall-mounted lamp shader fixes / Z-fighting on thin items. Polish pass.
- Multi-select / batch operations on placed pieces.

## Item assignment

| Mode | Items |
| --- | --- |
| Build | Wall, Floor, Door, Window, Roof, Fence |
| Decoration | Bed, Chest, Lantern, Chair, Table, Bench, Campfire, Barrel, Bucket, FlowerPot, Wreath, all `Form::Interior` items |

Both modes use the existing `ItemTags`:
- `STRUCTURAL` -> Build
- `FURNITURE` or `DECORATION` -> Decoration

The `PlaceableItems` resource (the union list) stays as is; each mode filters it by tag at startup.

## Magnetic-continuous placement

### v1 (ships first): fine grid, 0.1m

Cursor's surface-attached XZ rounds to 0.1m. Predictable, two lamps placed near each other line up. No magnet logic. Ships first because it is a fraction of the work and already feels significantly better than cube-snap.

### v2 (polish pass): magnetic anchors

Anchors gravitate the cursor toward sensible positions. Anchor sources:

- Cell centres (where a cube would sit).
- Wall mid-points and corners.
- Edges of nearby placed furniture (so a candle lines up with the table edge).
- Cell boundaries on terrain.

Pull falloff: linear within ~0.15m of an anchor, none beyond. Multiple anchors compose by nearest-wins. `Alt` disables the magnet (free continuous).

### Surface attach

The placement system picks an attach surface from the rapier raycast hit:

- **Hit terrain** -> terrain top, snapped XZ on the surface, Y from terrain height.
- **Hit floor (`PlacedItem` with `Form::Floor`)** -> floor top.
- **Hit wall side face (`hit.normal.y.abs() < 0.3`)** -> wall surface; item rotates so its back faces the wall normal; Y is constrained to the wall's height range.
- **Hit furniture top face (`hit.normal.y > 0.7`, non-floor)** -> that furniture's top, Y above its AABB.

Each surface type has its own snap logic but they share the same magnet anchor system.

## Module reorganisation

```
src/
  building/
    mod.rs                  # BuildingPlugin, BuildMode, B-key, cube-grid only
    placement.rs            # compute_placement, line, paint, replace
    ui.rs                   # bottom hotbar (6-swatch strip)
    collision.rs            # unchanged
  decoration/                # NEW
    mod.rs                  # DecorationPlugin, DecorationMode, N-key
    placement.rs            # magnetic-continuous, surface attach
    interior.rs             # MOVED from building (interior asset spawn / AABB)
    move_tool.rs            # pick up / carry / drop
    catalog_ui.rs           # right-side catalog (1000 thumbnails)
    hotbar_ui.rs            # bottom hotbar (tools + thumb + quickbar)
  edit/                      # NEW shared infra
    mod.rs                  # EditPlugin
    history.rs              # MOVED from building -- EditHistory (renamed BuildHistory)
    highlight.rs            # NEW HighlightPlugin (hover / held tints)
    placed_item.rs          # PlacedItem (renamed PlacedBuilding)
```

### What moves

| From | To |
| --- | --- |
| `building::BuildHistory` | `edit::EditHistory` |
| `building::PlacedBuilding` | `edit::PlacedItem` |
| `building::resolve_interior_spawns` | `decoration::interior` |
| `building::compute_interior_placement` | `decoration::placement` |
| `building::footprint_clear`, `BlockingRule` | `decoration::placement` |
| `building::ui::draw_decoration_catalog` | `decoration::catalog_ui` |
| `building::cube_target_width` | stays in `building::placement` (door / window stretch is structural) |

### Plugin registration order

`EditPlugin` first (shared types + history), then `BuildingPlugin`, then `DecorationPlugin`.

## Tools per mode

### Build mode

- **Place** (existing): cube-grid placement. `Form::placement_style()` routes between `Single`, `Line`, `Paint`, `Replace`.
- **Remove** (existing): click a placed structural piece, refund 1 to inventory.

### Decoration mode

- **Place**: magnetic-continuous on surfaces. Sticky -- placing one keeps the ghost so consecutive clicks place repeats.
- **Move**: click a placed decoration piece to pick it up; ghost follows cursor; click to drop. No inventory change. Magnet rules apply during drop placement.
- **Remove**: click a placed decoration piece, refund 1 to inventory.

Rotation: `R` / `Shift+R` advances 15 degrees (`Alt+R` continuous). Build keeps the existing 90-degree step for cube-symmetric pieces.

Move only targets pieces whose form is decoration-tagged. Build-mode pieces (walls, floors, etc.) are not pickable in decoration mode -- they belong to build's verb set. Use Build's Remove on those.

## UI

### Build hotbar (bottom centre)

```
[1 Place] [2 Remove]   |   [Wall] [Floor] [Door] [Window] [Roof] [Fence]   |   Undo  Redo  | X-ray
```

Six swatches replace the current piece-name label. Click a swatch to pick the piece. `[` / `]` cycles them.

### Decoration hotbar (bottom centre)

```
[1 Place] [2 Move] [3 Remove]   |   [thumb] piece name   |   [recent x8]   |   Undo  Redo  | X-ray
```

Centre slot shows a thumbnail + name of the currently selected piece. To the right, a strip of up to 8 thumbnails of recently-placed pieces; click any to re-select. The recent strip is in-memory only for v1 (does not persist across save / load).

### Decoration catalog (right side, existing)

Search box, categories collapsed by default, expand on filter. Click a thumbnail to select. Selecting from the catalog implicitly switches the active tool to `Place`.

### `[` / `]` in decoration mode

Cycles through the **currently filtered** catalog list, not the full one. Search + cycle is the keyboard browse loop.

## Inventory

Decoration placement consumes inventory the same way Build does. Move does not. `INFINITE_RESOURCES = true` continues to bypass the consumption check across both modes. The cheat is one `if !INFINITE_RESOURCES { ... }` guard around the inventory call -- already in place for Build, mirrored in Decoration.

The 1000 LowPoly Interior items have no crafting recipe; today inventory is implicitly infinite via the cheat. The future AI / town crafting system fills them into inventory legitimately. Decoration mode does not need to change when that lands.

## Highlights

`HighlightPlugin` adds two states to existing `PlacedItem` entities:

- **Hover**: when Move or Remove is the active tool, the piece under the cursor gets a tint applied to its material (cyan for Move, red for Remove, alpha ~0.3). Implementation: a tracked `(Entity, original_material) -> tinted_material` cache, restored when hover changes or the tool exits.
- **Held**: the piece currently being moved gets a gentle pulsing emissive tint. Implementation: same material swap, with a sin-driven alpha modulator each frame.

Both states clean up on tool exit, mode exit, and entity despawn.

## Save / migration

- `PlacedBuilding` rename to `PlacedItem`: save format keys on `ItemId`'s save key (Form + Material), not the component name. moonshine reflection picks up the renamed component automatically once `#[reflect(Save)]` is on `PlacedItem`. Existing saves load without changes on disk.
- `BuildHistory` rename to `EditHistory`: history is in-memory only, not saved. Free rename.
- Decoration mode adds no new save fields. Pieces placed via decoration use the same `PlacedItem` + `Transform` as build pieces.

## Out of scope

- AI / town crafting recipes for the interior pack.
- Wall-mounted lamp Z-fighting / shader fixes.
- Cube-cell save format.
- Decoration in-world tooltip ("what is this") UI on hover.
- Furniture-on-furniture stacking beyond the simple AABB-top check.
- Multi-select / batch operations (W2.10 in the original phase 2 spec; defer to Phase 3 polish).

## Open questions for the implementation plan

- **Surface priority order for the magnet.** Probably terrain -> floor -> wall -> furniture, with the first hit winning. Plan should pin this.
- **Rotation step exact value.** 15 / 22.5 / 30 degrees -- pick a default; tunable later.
- **Sticky placement default.** Build is sticky on Line / Paint, single-shot on others. Decoration is always sticky with `Esc` to exit. Confirm in the plan.
- **Move tool's eligibility filter.** Form-tag based (`DECORATION` or `FURNITURE`)? Or a `placement_style()` based check? Pick one.
- **Quickbar persistence.** In-memory only for v1 per this design; revisit later if it earns its keep.
