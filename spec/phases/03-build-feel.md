# Phase 2 — Build Feel: Modular Kit + Snap + BuildJob

> Status: Planned
> Depends on: Phase 1
> Exit criteria: Player can place modular building pieces from a hotbar, snapping to neighbors. Pieces are constructed over time as `BuildJob` entities (player builds them in Phase 2; Builder cats consume queue in Phase 6). Walls, floors, roofs, stairs, doors, windows, and ~5 furniture/decoration pieces ship. Interior visibility (translucent walls, hidden roof) works. Cozy Score MVP visible as heart particles.

## Goal

Make building feel good. Snap-based, in-world, no mode-switch beyond a single toggle. Construction is *visible*: pieces don't pop in, they assemble. Editing existing pieces is one-touch. The cozier a room, the more it feels alive.

This phase is the spec's #1 pillar. It is the longest phase and the one most worth slow-cooking until it feels right.

## Why now

Terrain is editable (Phase 1), and auto-flatten is wired. Without the building kit, all that infrastructure has no payoff. Build feel is also the foundation everything else (blueprints, NPCs, towns, festivals) hangs off.

## Deliverables

- `BuildingPiece` entity model with snap points, materials, cozy_value
- Snap algorithm with green/red ghost preview
- Hotbar build menu (egui) with categories
- `BuildJob` queue + construction-over-time system, animated
- Mouth-slot inventory: cat carries the piece visually while constructing
- Edit interactions: drag-reposition, R rotate, scroll material, delete refunds
- Multi-select via shift-drag-box
- Interior visibility (translucent occluding walls + hidden roof)
- Cozy Score MVP: per-piece value aggregated per building, visible as heart particles
- ~15 starter kit pieces authored, plus migration of existing buildables (fence, bench, lantern, flowerpot, wreath) into the kit catalogue
- Construction animations: framing → walls rising → roof capping (per piece type)

## Decisions to record

- DEC-022 — Building piece data lives in `Form` registry (extending DEC-010 combinatorial system): Form holds mesh + snap points + cozy_value + category, Material remains a separate axis
- DEC-023 — Snap radius: 2 m around cursor; piece-local snap point matching by type (wall-end-to-wall-end, etc.)
- DEC-024 — Construction time per piece scales with category (decoration 2–5 s, furniture 5–10 s, wall 10–20 s, floor 5–10 s, roof 15–30 s)
- DEC-025 — Existing fence/bench/lantern/flowerpot/wreath migrate to Decoration category; their meshes feed the new Form registry, no asset churn

## Tech debt closed

- DEBT-005 — let-else patterns in player/camera (touch them when building system reads movement state)

## Work breakdown

### W2.1 — `Form` registry extension for building pieces

**What:** Extend `items::Form` (or a sibling enum) with building-piece variants. Each Form has: mesh handle, collider shape, list of `SnapPoint { local_pos, local_normal, kind }`, cozy_value, build_time_secs, category (Wall, Floor, Roof, Stairs, Foundation, Fixture, Furniture, Decoration, Outdoor).
**Acceptance:** Forms registered at startup. Querying a Form's snap points returns world-space points after applying transform. Existing items in registry unaffected.

### W2.2 — Snap point kinds and compatibility table

**What:** Define `SnapKind`: `WallEnd`, `WallBottom`, `FloorEdge`, `FloorCorner`, `RoofRidge`, `RoofEave`, `StairTop`, `StairBottom`, `Surface`, `WallSurface`. Compatibility matrix declares which kinds can snap to which (e.g. `WallEnd ↔ WallEnd`, `WallBottom ↔ FloorEdge`, `Surface ↔ FloorCorner` for furniture on floors).
**Acceptance:** Compatibility check is a single matrix lookup. New kinds added in one place propagate to all snap logic.

### W2.3 — Snap algorithm

**What:** When a ghost piece is positioned, query all building pieces within 2 m of cursor via spatial index (rapier broadphase or a simple grid). For each candidate, find compatible snap point pairs. Snap to the nearest valid pair. Default to grid (0.5 m primary, 0.25 m with Shift) if no neighbor.
**Acceptance:** Walls snap end-to-end perfectly. Floors snap edge-to-edge. Furniture snaps to floor surfaces. Snap responsiveness < 16 ms.

### W2.4 — Ghost piece preview with green/red validity

**What:** While in `BuildState::Building` with a piece selected, spawn a ghost entity that follows the cursor. Ghost uses a translucent shader. Tint green if placement is valid, red if invalid (overlap, no snap target where required, insufficient space). Snap point markers on neighbors highlight when ghost is in their range.
**Acceptance:** Ghost is always visible and never blocks gameplay. Valid/invalid state changes as cursor moves over different positions. Snap markers feel like a tactile guide.

### W2.5 — `BuildJob` queue

**What:** `Resource BuildJobs(VecDeque<Entity>)` plus per-job entity carrying `BuildJob { form, material, transform, progress: 0.0..1.0, assignee: Option<Entity>, materials_reserved: bool }`. Click-to-place enqueues a job at cursor position with current selected piece.
**Acceptance:** Click 10 times along a path → 10 jobs queued, all visible as "scaffolding" ghosts at their target positions.

### W2.6 — Construction system: tick + animate

**What:** System advances `progress` for jobs whose `assignee` is currently the player (Phase 2) or a Builder cat (Phase 6 hook). Visual representation: scaffolding mesh swap as progress crosses thresholds (0–25 % framing, 25–75 % piece building, 75–100 % details). At 1.0, despawn job, spawn the final `BuildingPiece` entity with collider, snap points, and material.
**Acceptance:** Placed wall takes its declared time to construct. Visual feedback at quartile thresholds is clear. Final piece behaves as a permanent building piece (collisions, snap target).

### W2.7 — Player as builder in Phase 2

**What:** Player walks to the next queued job in range, plays a build animation (or capsule placeholder gesture), drives `progress`. Multiple nearby jobs queue in distance order.
**Acceptance:** Click-to-place 3 walls in a row → player walks job to job, building each in turn. Walking away pauses progress; returning resumes. No double-claim of a single job.

### W2.8 — Mouth-slot inventory: visible carry

**What:** New component `MouthSlot(Option<Entity>)` on player. When carrying, the piece's mesh attaches at the cat's head bone (or an offset above the capsule placeholder). Carrying disables grooming, meowing (no-op stubs reserved for later). Drop on the same input that picked up.
**Acceptance:** Selecting a piece + walking shows the piece visibly attached. Switching pieces swaps the visible attachment. Dropping clears the slot and spawns the piece on the ground.

### W2.9 — Edit interactions on placed pieces

**What:**
- Hover a piece → outline it.
- Click+drag → reposition (re-snap at destination, refund materials only on cancel via Esc).
- R → rotate 90° (Shift+R = 15°).
- Scroll while hovering → cycle material (live, no rebuild).
- Delete key → remove, refund materials to town pool (Phase 4 hook).
- Right-click → context menu (delete, duplicate, copy material).

**Acceptance:** All interactions feel snappy. Drag preview shows the piece moving with valid/invalid tint. Material cycle is instant (handle swap, no respawn).

### W2.10 — Multi-select via shift-drag-box

**What:** Hold Shift, drag a screen-space box. All pieces whose center is inside the projected world box become selected (indicated by outline). Batch operations: delete-all, rotate-all, change-material-all.
**Acceptance:** Box-selecting 10 walls and pressing Delete refunds all 10 and clears them in one frame. Batch material change is instant.

### W2.11 — Interior visibility: translucent occluders + roof hide

**What:** When camera is inside a `Building` AABB, all walls of that building between the camera and the cat fade to ~30 % alpha. Roof becomes invisible. Implementation: per-building shader uniform `interior_visible: bool`, plus a per-frame check that flips it based on camera transform.
**Acceptance:** Walking the cat into a single-room cottage: roof disappears, the wall behind the camera fades, the cat is visible inside. Walking out reverses cleanly.

### W2.12 — Cozy Score aggregator

**What:** `Building` entity holds child `BuildingPiece` entities. System recomputes Cozy Score on `Changed<Children>` for the building or `Changed<Material>` on a piece: sum of `cozy_value` per piece, plus bonuses (lighting active at night, kitchen completeness, fireplace + rug + plant combo). Penalties for empty rooms (heuristic: large floor area with few non-floor pieces).
**Acceptance:** Adding a chair to an empty room increases score. Adding a lamp at night adds more than during day. Score readout visible in build mode header.

### W2.13 — Cozy heart particle effect

**What:** Particle emitter attached to each Building entity. Emission rate scales with normalized cozy_score. Particles drift up and dissipate. Subtle, not overwhelming.
**Acceptance:** Empty cottage: zero hearts. Furnished cottage: gentle stream of hearts visible from outside. Maxed-out cottage: dense column.

### W2.14 — Hotbar build menu (egui)

**What:** Persistent hotbar with categories (Walls, Floors, Roofs, Doors/Windows, Stairs, Furniture, Decoration, Outdoor, Brushes). Number keys 1–9 select within current category. Tab cycles category. LT (gamepad) opens a radial menu mirroring categories.
**Acceptance:** Every piece in the kit is reachable in ≤ 2 inputs. Radial menu works on gamepad with no KB+M dependency.

### W2.15 — Authoring: 15 starter kit pieces

**What:** Mesh authoring (Blender → glTF). Pieces:
1. Wall (full, 4 m wide)
2. Wall with window
3. Wall with door
4. Wall (half-height)
5. Floor tile (2 m × 2 m)
6. Foundation (stone)
7. Ceiling (flat)
8. Roof gable end
9. Roof slope
10. Roof ridge cap
11. Stairs (straight)
12. Door (placeable into wall-with-door slot)
13. Window pane (placeable into wall-with-window slot)
14. Bed (basic)
15. Table (dining)

Plus migration: existing fence, bench, lantern, flowerpot, wreath registered as Decoration.
**Acceptance:** All 15 pieces selectable from hotbar, place via snap, render correctly with all current materials (wood/stone). Migrated pieces continue to work in saves from Phase 1.

### W2.16 — Save migration: pieces and Cozy Score

**What:** `BuildingPiece`, `Building`, `MouthSlot`, `BuildJobs` queue serialized via moonshine reflection.
**Acceptance:** Place a small cottage, save, reload. Cottage and its cozy score and visible particles return identical.

## Risks / open questions

- **Snap correctness with rotated pieces.** Mathematical bug surface is high. Lock down with unit tests for snap point transforms early.
- **Translucent wall shader for interior visibility.** Order-independent transparency is hard. Acceptable starting point: simple alpha blend with depth pre-pass; iterate if visual quality is poor.
- **Construction-over-time on capsule player.** Without cat animations, construction "feels" thin. Mitigation: framing scaffolding visualization carries the feel of construction independent of player animation.

## Out of scope

- Floor plan tool (Phase 3)
- Auto-roof tool (Phase 3)
- Builder cat consuming jobs (Phase 6)
- Town pool for material refund (Phase 4 — until then, refund into existing inventory)

## Estimated effort

12–18 work-days. Snap correctness, construction visualization, and interior visibility are the long poles.
