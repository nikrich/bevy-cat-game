# Phase 3 — Floor Plan + Blueprint Library

> Status: Planned
> Depends on: Phase 2
> Exit criteria: Player can sketch a building outline on terrain, define rooms, mark doors/windows, choose material preset, and have the system queue construction jobs that build the structure piece-by-piece. Player can save any constructed building as a blueprint and re-place it. Auto-roof MVP works for rectangular footprints.

## Goal

Lift building from "place each wall by hand" to "sketch a cottage, watch it build itself." Floor plans abstract the per-piece grind for common cases. Blueprints make a beautiful building reusable, including by NPC builder cats in later phases.

## Why now

The modular kit (Phase 2) is the substrate. Floor plans + blueprints are how the player actually expresses architectural intent at town scale, and how Phase 6 Builder cats will autonomously construct without player babysitting.

## Deliverables

- Floor plan tool: click corners on terrain, define outline, snap to grid
- Interior wall placement on the outline
- Door and window markers on outline edges
- Building type tags (Cottage, Shop, Workshop, Café, Library, etc.)
- Material preset selector (oak/pine/birch/stone variants)
- `BuildingBlueprint` entity with rough/specific/fully-detailed levels
- Floor plan execution: enqueues `BuildJob`s in correct order (foundation → walls → roof → doors/windows → interior basics)
- Auto-roof MVP for rectangular footprints (gable + hip)
- Save selected pieces as a Blueprint → `.bp.ron` file
- Blueprint browser UI; placeable like any other build tool
- Blueprint hot-reload (edit `.bp.ron` while game runs)

## Decisions to record

- DEC-026 — Blueprint format: RON (`.bp.ron`), human-readable, hot-reloadable as Bevy `Asset`. Rationale: spec §6.10 + spec §13.2 mandate
- DEC-027 — Floor plan execution order: foundation → exterior walls → interior walls → doors → windows → roof → ceiling → starter furniture (per building type defaults)
- DEC-028 — Auto-roof v1 covers axis-aligned rectangular footprints only; non-rectangular falls back to manual modular roof pieces

## Tech debt closed

- (None — debt accumulates in this phase, see Risks)

## Work breakdown

### W3.1 — Floor plan tool: outline sketcher

**What:** Build hotbar entry "Floor Plan." When active, clicks add corner points on terrain, snapped to 0.5 m grid. Visual: dotted line follows last point to cursor. Right-click closes the polygon.
**Acceptance:** Sketch a 6×8 m rectangle in 4 clicks. Polygon closes correctly. Outline persists as a ghost until confirmed or canceled.

### W3.2 — Interior wall divisions

**What:** After outline is closed, player can click two points along edges to add interior wall segments. Each interior wall is a separate segment in the plan data.
**Acceptance:** Add two interior walls to a rectangle to make 3 rooms. Wall endpoints snap to outline edges or to other interior walls.

### W3.3 — Door and window markers

**What:** Click an edge segment, choose Door or Window, drag to position along the segment, set width.
**Acceptance:** Place 1 door + 2 windows on a 6 m wall. Markers persist on the plan and remain editable.

### W3.4 — Building type tag + material preset

**What:** Modal selector when finalizing the plan: building type (Cottage / Shop / Workshop / Library / Café / Generic), material preset (Oak / Pine / Birch / Stone / mixed). Preset determines default piece materials and starter furniture per type.
**Acceptance:** Choosing "Cottage + Pine" results in a finished build with pine walls, pine furniture defaults (bed, table, chair). Choosing "Workshop + Stone" yields stone walls and a workbench inside.

### W3.5 — `BuildingBlueprint` entity

**What:** `BuildingBlueprint { footprint: Polygon, interior_walls: Vec<Segment>, openings: Vec<Opening>, building_type: BuildingType, material_preset: MaterialPreset, fidelity: Rough | Specific | FullyDetailed, piece_overrides: Vec<PieceOverride> }`. Rough = outline + type only. Specific = + materials + opening placement. FullyDetailed = every piece declared.
**Acceptance:** Blueprint round-trips through serialization. Three example blueprints (rough, specific, detailed) load correctly.

### W3.6 — Plan-to-jobs expansion

**What:** System expands a `BuildingBlueprint` into ordered `BuildJob`s. Order per W3.4 spec. For Rough plans, fill unspecified details from `BuildingType` defaults. For FullyDetailed, expand piece list verbatim.
**Acceptance:** Confirming a "Pine Cottage" rough plan enqueues ~25 jobs. Player walks them and the cottage assembles in correct order. Final result is a fully-roofed, doored, windowed, furnished cottage.

### W3.7 — Auto-flatten on plan confirmation

**What:** When a plan is confirmed, call Phase 1's auto-flatten with the plan's footprint AABB before enqueueing the foundation job.
**Acceptance:** Plan placed on a slope produces a level foundation, with the skirt visible around the building.

### W3.8 — Auto-roof MVP for rectangular footprints

**What:** When the footprint is axis-aligned and rectangular, generate a roof procedurally: gable along the long axis, ridge at center, slopes to eaves, gable end pieces at the short edges. Output is a set of `BuildJob`s for the appropriate roof pieces from the kit.
**Acceptance:** A 6×8 m rectangular cottage gets a gabled roof. A 4×4 m square gets a hipped roof (square heuristic). Non-rectangular footprints fall back to placing modular roof pieces by hand.

### W3.9 — Save building as Blueprint

**What:** Multi-select pieces (Phase 2's box-select) → context menu → "Save as Blueprint." Modal asks for name, tags, style. Writes `.bp.ron` to `~/Library/Application Support/Cat World/blueprints/<name>.bp.ron`.
**Acceptance:** Multi-select all pieces of a built cottage → save as "Pine Hilltop Cottage." The file appears on disk, hand-readable. Blueprint becomes available in the placer.

### W3.10 — Blueprint placer

**What:** New build hotbar category "Blueprints." Lists all `.bp.ron` files. Selecting one shows a translucent footprint ghost. Click to place; system runs auto-flatten and expands into jobs. Rotation supported.
**Acceptance:** Saved blueprint re-places identically (modulo terrain auto-flatten). Two copies side-by-side look the same.

### W3.11 — Blueprint hot-reload

**What:** Register `.bp.ron` as an `Asset`. Watch the blueprints directory. On file change, reload the asset; placed copies are unaffected, but new placements use the updated blueprint.
**Acceptance:** Edit a `.bp.ron` to change the wall material. Place a new copy. The new copy has the new material; existing copies untouched.

### W3.12 — Blueprint browser UI

**What:** egui panel showing blueprint thumbnails (rendered offline once via `Camera::RenderTarget`) with name, tags, style. Search bar filters by tag.
**Acceptance:** 5 saved blueprints render thumbnails on first browser open. Filter by "Cottage" hides non-cottages.

### W3.13 — Building type starter furniture defaults

**What:** Each `BuildingType` has a defaults table mapping (BuildingType, RoomCount) → list of furniture/fixture pieces with relative positions. Used when blueprint fidelity = Rough.
**Acceptance:** Rough Cottage defaults: bed + table + 2 chairs + bookshelf. Rough Workshop defaults: workbench + stool + storage chest. Defaults are placed sensibly relative to the room outline.

### W3.14 — Save migration: blueprints and plans

**What:** Active `BuildingBlueprint` entities serialize with the world. `.bp.ron` files live separately and are loaded on demand.
**Acceptance:** Mid-construction blueprint saves correctly: reload resumes the in-progress construction with the same job queue and progress.

## Risks / open questions

- **Auto-roof on non-rectangular shapes.** Spec mentions advanced auto-roof as later content. Phase 3 ships the rectangular case only; document the L-shaped case as post-EA.
- **Blueprint rotation with non-square footprints.** Need to be careful about origin and bounding box conventions. Lock down convention early (e.g., footprint origin at SW corner, rotation about that point).
- **NPC builders not yet present.** Phase 3 jobs are still consumed by the player. Phase 6 wires Builder cats. Make sure the queue API is generic over `assignee: Option<Entity>`.

## Out of scope

- Builder cat AI (Phase 6)
- Architect cat blueprint modifications (post-EA per spec §16)
- Auto-roof for non-rectangular footprints (post-EA)
- Inter-building blueprints (entire town blocks) — post-EA

## Estimated effort

8–12 work-days. Auto-roof and rough-plan defaults are the trickiest items.
