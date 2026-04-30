# Phase 2 -- Build Feel: Cube Placement + Decoration Split

> Status: Stage 1 (cube placement) closed; Stage 2 (procedural Replace + interior catalog) closed; Stage 3 (decoration mode split) in implementation
> Depends on: Phase 1
> Exit criteria: Two mutually exclusive placement modes (`Build` and `Decoration`) with their own physics, verbs, and UI. Build mode places structural cubes; Decoration mode glides items along surfaces with magnetic snap. Single shared undo / redo. Players can construct a small house (build) and decorate it inside (decoration) without ever feeling like one mode is doing the other's job

## Goal

Make building feel good. The original spec assumed a snap-point algorithm; that direction was scrapped (DEC-020) and replaced with cube placement, then split (DEC-021) into a structural mode and a decoration mode that uses entirely different placement physics. This phase ships both halves.

This phase is the spec's #1 pillar. Build feel includes both *building* the structural skeleton and *decorating* the inside. They are the same pillar, but they want different tools.

## Stage 1 -- Cube placement (CLOSED, DEC-020)

Shipped 2026-04-30. See `project_phase2_cube_pivot.md` memory entry and DEC-020 in `.claude/memory/decisions.md` for the full account. Summary:

- Walls / floors / doors / windows are 1×1×1 cubes; rapier raycast + surface normal drive `compute_placement`
- `Form::placement_style()` routes between `Single` / `Line` / `Paint` / `Replace`
- Line tool (walls) with continuous-mode anchor advance
- Paint tool (floors) with whole-drag-one-undo
- Replace tool (doors / windows) with `BuildOp::Replaced` history variant for atomic undo
- Camera Q/E snap-rotate folded into iso movement so WASD stays screen-relative

Original work items W2.1 (snap data) and W2.2 (compatibility table) are dead code. W2.3 (snap algorithm) and W2.4 (snap-validity tint) are superseded by `compute_placement` + green / red ghost wash.

## Stage 2 -- Procedural Replace + Interior catalog (CLOSED)

Shipped alongside Stage 1. Highlights:

- Door / Window `Replace` placement style with composite spawn (header + jambs)
- Interior asset placement for the LowPoly Interior pack (~1000 nodes)
- Per-asset AABB pre-parsed at startup; `compute_interior_placement` snaps via footprint cells
- `resolve_interior_spawns` recentres each asset by `-aabb.centre`
- Door / window asset stretch (force x to 2m world width, force z to integer snap)
- Carpet rule (`BlockingRule::WallsOnly`)
- Pre-baked thumbnails (`bake_thumbnails` bin)
- Decoration catalog UI (right-side panel, thumbnail grid, search, categories)
- Indoor reveal (X-ray) with structural-only fade so furniture stays solid
- X hotkey for X-ray reveal in build mode
- `GatheredCells` persistence so reload doesn't regrow chopped trees

## Stage 3 -- Decoration mode split (IN IMPLEMENTATION, DEC-021)

The single `BuildMode` covering both structural and decoration is being split into two mutually exclusive modes. Design spec: `docs/superpowers/specs/2026-04-30-decoration-mode-split-design.md`. ADR: DEC-021.

### Goals

- `BuildMode` (`B` key) keeps cube-grid placement for Wall, Floor, Door, Window, Roof, Fence. Tools: Place / Remove. UI: bottom hotbar with 6-swatch piece selector
- `DecorationMode` (`N` key) places everything else (Bed, Chest, Lantern, Chair, Table, Bench, Campfire, Barrel, Bucket, FlowerPot, Wreath, all `Form::Interior` items). Tools: Place / Move / Remove. UI: bottom hotbar + right-side catalog
- Magnetic-continuous placement on attach surfaces (terrain / floor top / wall face / furniture top)
- 15-degree rotation steps (`R` / `Shift+R`); `Alt` for free continuous
- Move tool picks up + carries + drops without inventory churn
- Subtle highlights for hover and held pieces
- Single shared undo / redo across both modes

### Work breakdown

#### W2.S3.1 -- Extract `src/edit/` shared infra

Move `building/history.rs` -> `edit/history.rs`. Rename `BuildHistory` -> `EditHistory`. Rename `PlacedBuilding` -> `PlacedItem` and move to `edit/placed_item.rs`. Add new `edit/highlight.rs` with `HighlightPlugin` (hover + held tints, no logic yet, just the resource and component scaffolding). All `building` imports update; saves load unchanged because moonshine reflection keys on save key not component name.

**Acceptance:** Compile clean. Save / load existing world. All existing build hotkeys still work. No behavioural change.

#### W2.S3.2 -- Slim `src/building/`

Move `compute_interior_placement`, `resolve_interior_spawns`, `footprint_clear`, `BlockingRule`, the interior-asset spawn helpers out of `building/mod.rs` -- staged in a temporary `building/interior.rs` for now. Move `draw_decoration_catalog` out of `building/ui.rs` -- staged in a temporary `building/catalog.rs`. Building still owns these temporarily; the `decoration/` plugin will adopt them next.

**Acceptance:** `building/mod.rs` is back under ~1200 lines. No behavioural change.

#### W2.S3.3 -- New `src/decoration/` plugin scaffold

Create `decoration/mod.rs` with `DecorationPlugin`, `DecorationMode` resource, `DecorationTool` enum (`Place`, `Move`, `Remove`), `N` key toggle (mutually exclusive with `BuildMode` -- pressing `N` in build mode swaps). Empty `placement.rs`, `interior.rs`, `move_tool.rs`, `catalog_ui.rs`, `hotbar_ui.rs` modules. Move the staged interior + catalog files from `building/` into `decoration/` and rewire imports.

**Acceptance:** `N` toggles a no-op decoration mode (no piece selected, no UI yet). `B` still works. Catalog UI now shows in decoration mode instead of build mode.

#### W2.S3.4 -- Magnetic-continuous v1 (fine 0.1m grid)

Implement `decoration::placement::compute_decoration_placement`. Use the existing rapier raycast hit + normal to pick the attach surface (terrain / floor / wall / furniture). Snap surface XZ to 0.1m. Rotation: 15-degree steps (`R` / `Shift+R`); `Alt+R` for free continuous.

**Acceptance:** Place a chair anywhere on a floor at 0.1m precision. Place a candle on a table top. Place a wall lamp on a wall face. Two lamps placed near each other line up at the 0.1m grid.

#### W2.S3.5 -- Decoration UI

Bottom hotbar in decoration mode: `[Place] [Move] [Remove] | thumbnail + name | recent x8 | undo / redo | X-ray`. Right-side catalog stays as is. Recent-picks quickbar in-memory only; click any to re-select. `[` / `]` cycles within the currently filtered catalog list.

**Acceptance:** Decoration hotbar renders with all elements. Catalog selection updates the hotbar thumbnail. Recent quickbar populates as you place. `[` / `]` cycles inside the filtered list.

#### W2.S3.6 -- Move tool

Pick up a placed decoration piece on click; ghost follows cursor with magnetic placement; click to drop. No inventory delta. Build-mode pieces (wall / floor / door / window / roof / fence) are not pickable. Held piece shows the gentle pulsing emissive highlight.

**Acceptance:** Click a placed chair -> chair becomes the held ghost. Click a new spot -> chair lands there. Click a wall -> nothing happens (build piece, not pickable in decoration mode).

#### W2.S3.7 -- Hover / held highlights

Implement `HighlightPlugin`: hover (cyan for Move, red for Remove) on the piece under the cursor; held (gentle pulse) on the carried piece. Both clean up on tool / mode exit and entity despawn.

**Acceptance:** Move tool hovered over a chair -> chair tints cyan. Switch to Remove -> tint flips to red. Pick up the chair -> tint becomes a soft pulse.

#### W2.S3.8 -- Build mode 6-swatch hotbar

Replace the current piece-name label in the build hotbar with six clickable swatches (Wall / Floor / Door / Window / Roof / Fence). `[` / `]` cycles them. Click a swatch to pick.

**Acceptance:** Build hotbar shows six swatches. Click a swatch -> ghost updates. `[` / `]` cycles.

### Stage 3 deferrals

- Magnetic anchors (v2). Plan to land in a polish pass after v1 is in players' hands.
- Quickbar persistence across save / load.
- Wall-mounted item Z-fighting / shader fixes.
- Multi-select / batch operations (was W2.10 in the original spec).
- Cube-cell save format.

## Risks / open questions

- **Existing save compatibility through the rename.** `PlacedBuilding` -> `PlacedItem` should be transparent (moonshine reflection keys on save key). Verify with a saved world from before the rename.
- **Surface priority order for the magnet.** Plan to lock terrain -> floor -> wall -> furniture (first hit wins). Polish pass tunes.
- **`B` and `N` muscle memory clash.** `N` is currently unused; if a future system claims it (notebook?), revisit.
- **Held-piece highlight on asset-backed entities.** Pulsing emissive on a `SceneRoot` child requires walking the children -- may need a different mechanism for those.

## Out of scope

- AI / town crafting recipes for the interior pack (separate spec, future)
- Floor plan tool (Phase 3)
- Auto-roof tool (Phase 3)
- Builder cat consuming jobs (Phase 6)
- Construction-over-time (`BuildJob`) -- the original W2.5-W2.7 cluster. Out of scope; the cube-pivot model places instantly and we are not bringing construction time back unless playtest demands it. If revived, treat as a Phase 6 deliverable when builder cats land
- Cozy Score and heart particles -- the original W2.12-W2.13. Out of scope for Phase 2; lift into Phase 4 when town life makes scoring meaningful

## Estimated effort

3-5 work-days for Stage 3 (the decoration split). The rename pass + extraction is the longest pole; magnetic v1 and Move tool are mostly straightforward against the renamed types.
