# Cat World — Implementation Plan

This is the phased build plan that takes the current codebase to the spec's Early Access scope (`spec/spec.md` §14.4): a fully playable single-town loop with building, friendship, roles, gathering, crafting, festivals, weather, audio, save sync, and polish.

Each phase doc is tactical: ordered work items with acceptance criteria, intended to be fed to a Claude agent orchestrator.

## Terminal milestone — Early Access launch

- Single-town gameplay loop fully functional
- 1–2 biomes featured (Forest + Meadow recommended) with all 10 ambient
- Modular building kit: ~30–50 pieces, ~15–20 furniture/decoration items
- 8–10 NPC cat archetypes with personality, friendship, roles
- 2–3 festival types
- Weather (rain, snow, wind), per-biome audio
- Save/load with Steam Cloud sync
- Controller-first parity with mouse+keyboard
- No combat, no multiplayer

## Phases

| #  | Phase                                               | Status   | Depends on |
| -- | --------------------------------------------------- | -------- | ---------- |
| 0  | [Foundations & Engine Upgrade](./01-foundations.md) | Planned  | —          |
| 1  | [Terrain Rewrite & Editing](./02-terrain.md)        | Planned  | 0          |
| 2  | [Build Feel: Modular Kit + Snap](./03-build-feel.md)| Planned  | 1          |
| 3  | [Floor Plan + Blueprint Library](./04-blueprints.md)| Planned  | 2          |
| 4  | [Crafting + Town Pool + Treats](./05-crafting.md)   | Planned  | 2          |
| 5  | [First NPC Cat: Friendship + AI](./06-npc-cat.md)   | Planned  | 1, 2, 4    |
| 6  | [Town + Roles + Festivals](./07-town.md)            | Planned  | 5          |
| 7  | [Weather, Audio, Polish, EA Launch](./08-launch.md) | Planned  | 6          |

## Parallel workstreams

These workstreams run alongside the numbered phases above. They are not on the EA critical path; they can land whenever their owning agent finishes a slice. Each has its own spec under `docs/superpowers/specs/`.

| Workstream            | Spec                                                                                                | Owns                                                                                                | Status              |
| --------------------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------- |
| Worldcraft Expansion  | [voxel-mountain-caves](../../docs/superpowers/specs/2026-05-02-voxel-mountain-caves-design.md)      | Voxel mountains, PCG caves, climate classifier, sinkhole carving, cave-occupancy `DarknessFactor` term | Designed (DEC-024) |
| Night Torch           | [night-torch](../../docs/superpowers/specs/2026-05-02-night-torch-design.md)                        | Cat-held torch GLB, shared `DarknessFactor` resource, point light + ember particles, dawn/dusk fade | Designed (DEC-025) |

Workstream design rules:
- Must not regress the numbered-phase exit criteria.
- Must not block any numbered phase. If a numbered phase needs work that overlaps a workstream, lift the overlap into the phase doc and shrink the workstream accordingly.
- Coupling with numbered phases is documented in the workstream spec, not in the phase docs (keeps phase docs stable).

## Cross-cutting principles

- **No fail states.** Every phase preserves spec §2.3 (no death, no eviction, gentle decay).
- **No combat, no multiplayer.** Settled per spec §16.
- **Day length: 24 in-game hours = 24 real minutes.** 1 minute real time = 1 in-game hour.
- **Steam Cloud is the save target.** Save paths must be Steam-friendly from day one.
- **Tech debt closes inside its closest phase**, listed per phase. New debt is recorded in `.claude/memory/tech-debt.md`, never deferred silently.
- **Each phase opens with a decision-log update** for any superseded prior decisions and closes by recording new ones.

## Decisions superseded by this plan

- **DEC-003** (per-tile cuboid stepped terrain) → superseded by Phase 1 vertex-height grid. Stepped *aesthetic* preserved via terrain shader.
- **DEC-004** (per-tile entities in chunks) → superseded by Phase 1 single-mesh-per-chunk. DEBT-007 closes.
- **DEC-006** (12-min day cycle) → updated to 24-min in Phase 0.
- **DEC-007** (custom GameInput resource) → superseded by Phase 0 leafwing-input-manager.
- **DEC-011** (serde JSON save) → superseded by Phase 0 moonshine-save with reflection-based serialization. JSON format retained on disk for human-readability.

## Conventions inside phase docs

- `W<phase>.<n>` — work item ID, stable across edits. Reference these in commits and PRs.
- **What** — concrete change.
- **Acceptance** — observable test or runtime behavior that proves it shipped.
- **Closes** — tech debt or decision item this work resolves.

## Reading order

If this is your first time, read in order: `01-foundations.md` → `02-terrain.md` → `03-build-feel.md`. Phases 4 and 5 can run in parallel after Phase 3. Phases 6–7 are sequential.
