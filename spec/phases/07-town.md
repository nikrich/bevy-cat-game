# Phase 6 — Town + Roles + Festivals

> Status: Planned
> Depends on: Phase 5
> Exit criteria: A `Town` first-class entity exists with boundary, population, town pool, reputation, and (optional) specialty. The player can assign cats to roles (Builder, Forager, Cook, Fisher, Farmer, Innkeeper, Bard, Mayor, Merchant, Scholar). Builder cats consume `BuildJob`s autonomously. Foragers gather daily. Cooks produce treats. Festivals plannable from a Town Hall building, with at least three festival types running. The single-town gameplay loop is complete.

## Goal

Take the lone-cat-with-friends slice and grow it into a self-running town. Roles are how labor scales without the player having to babysit every job. Festivals are the social crescendo that ties the social pillar to the architecture pillar (architecture determines where cats gather).

## Why now

Phase 5 has one cat. Phase 6 makes a town of 5–10 cats meaningful and self-perpetuating. This phase closes the EA scope minus polish/audio/weather (Phase 7).

## Deliverables

- `Town` entity (id, name, population, buildings, blueprint_lib, storage, cozy_score, reputation, specialty, boundary, biome) per spec §9.1
- Town boundary: convex hull from buildings + buffer; auto-recomputed on building add/remove
- `Town Founding Stone` placeable (Phase 1 spawn town auto-created at game start)
- Reputation aggregator (cozy_score, happy residents, festivals hosted, decoration coverage)
- Multi-town `TownPool` selection by player position (collapses to spawn town for now since multi-town is post-EA)
- 10 roles per spec §7.6: Builder, Forager, Cook, Fisher, Farmer, Innkeeper, Bard, Mayor, Merchant, Scholar
- Workplace requirements (each role ↔ one workplace building type)
- Role assignment UI in cat profile panel
- Work efficiency formula per spec §7.7 with 40 % floor
- Builder cats consume BuildJobs (closes the Phase 2 placeholder)
- Forager daily gathering (biome-appropriate)
- Cook daily treat production (biome-appropriate recipes)
- Festival Planner UI (Town Hall)
- 3 festival types: Spring Bloom, Summer Sunfest, Music Night
- Festival execution: cat gathering, decorations spawn, results screen with friendship boosts
- 5 additional NPC archetypes (10 total counting Phase 5)
- Inn arrivals: traveler cats periodically stop, can be invited

## Decisions to record

- DEC-039 — Town boundary uses convex hull of `Building` entity AABBs + 4 m buffer; recomputed on add/remove (debounced)
- DEC-040 — Roles are 1-per-cat, switchable freely with a 1 in-game day "settling" delay
- DEC-041 — Work efficiency floor 40 % (no cat is ever useless)
- DEC-042 — Festival types are data-driven (`Resource FestivalCatalog`), so adding a new festival requires data, not code
- DEC-043 — Mayor role unlocks the Town Hall UI; without a Mayor the player still has access (player is implicit mayor in spawn town until one is assigned)

## Tech debt closed

- Closes the explicit Phase 2 placeholder where the player was the only builder

## Work breakdown

### W6.1 — `Town` entity

**What:** Define `Town` per spec §9.1. Spawn one at game start centered on player spawn. Mark all existing world objects as part of this town based on a generous initial boundary.
**Acceptance:** `Query<&Town>` returns one entity. Save/load preserves it. Population list updates as cats are welcomed.

### W6.2 — Town boundary

**What:** Convex hull of `Building` AABBs in the town + 4 m buffer. Recompute on building add/remove (debounced 1 s). Store as `Polygon`. Visualize as a soft outline overlay when the player toggles the Map zoom.
**Acceptance:** Boundary updates within 1 s of placing a new outlying cottage. Outline visible at Map zoom and hidden at lower zooms.

### W6.3 — Town Founding Stone

**What:** Placeable structural piece. In spawn town, spawned automatically at game start (`MainMenu` → `New Game` flow). Used post-EA for founding new towns.
**Acceptance:** Founding Stone exists at spawn. Placing one in Phase 6 outside an existing town's boundary creates a second town entity (post-EA usage; data path exists from Phase 6, UI hidden).

### W6.4 — Reputation aggregator

**What:** System computes town reputation from: aggregate cozy_score across buildings, count of happy residents (mood ≥ Content), festivals hosted in last 7 in-game days, decoration coverage (% of buildings with cozy_score > threshold). Updates every in-game day. Reputation 0–100.
**Acceptance:** A town with one cottage and one resident shows reputation ~10. Adding 4 cozy buildings + 3 happy cats raises rep to ~50. Hosting a festival adds +10 for a week.

### W6.5 — Multi-town `TownPool` selection

**What:** Replace Phase 4's global pool with `HashMap<TownId, TownPool>`. Player accesses the pool of the town whose boundary contains them. Query `current_town(player_pos: Vec3) -> Option<TownId>` runs on player movement.
**Acceptance:** Phase 4 behavior preserved when only one town exists. Walking to a hypothetical second town's boundary swaps which pool the UI shows.

### W6.6 — Role enum + workplace requirements

**What:** `Role` enum with all 10 from spec §7.6. `WorkplaceKind` enum (Workshop, Storehouse, Kitchen, Dock, Farmhouse, Inn, MusicHall, TownHall, Shop, Library, DraftingOffice, CartStation). Each role maps to one workplace kind. Workplace buildings detected by tagging buildings with `WorkplaceKind` when the player marks them in the build menu.
**Acceptance:** Build a cottage → mark it as Inn → it becomes a valid Inn workplace. Cats assigned Innkeeper require an Inn building somewhere in the town.

### W6.7 — Role assignment UI

**What:** In the cat profile panel (Phase 5 W5.14), add a Roles section. Listed roles greyed out unless: friendship trust ≥ 30 (per spec §7.6), and a workplace exists in the town. Clicking a role assigns it. Switching costs 1 in-game day "settling."
**Acceptance:** Assign Mochi as Forager. Mochi spends the next morning settling, then begins a forager routine.

### W6.8 — Work efficiency formula

**What:** Implement `efficiency = base_role_efficiency * (current_friendship / cap_friendship) * trait_modifier * mood_modifier`. Floor at 0.4. Used by all role action systems.
**Acceptance:** A perfectly suited Industrious cat at full friendship + Cheerful mood reaches ~1.5× efficiency. A trait-mismatched neglected cat floors at 0.4.

### W6.9 — Builder cats consume BuildJobs

**What:** New `BuilderAction` for big-brain. Scorer: 0.7 if Builder role + on-duty hours + jobs in queue + materials available. Action: walk to job, claim it, drive `progress` (efficiency-modulated). Multiple builders work in parallel on distinct jobs.
**Acceptance:** Queue 5 jobs, assign 2 Builders → both walk to jobs, work in parallel, complete the queue at ~2× single-builder rate.

### W6.10 — Forager daily gathering

**What:** Forager `Action` walks to gatherable nodes within town boundary (or beyond if assigned an outpost), gathers via the existing gathering interaction, deposits to TownPool. Yields biome-appropriate materials. Daily output scales with efficiency.
**Acceptance:** A Forager in a Forest town brings 8–12 oak/pine logs + assorted berries/herbs per in-game day. Pool tally rises overnight.

### W6.11 — Cook daily treat production

**What:** Cook `Action` walks to Kitchen workbench, picks an unfulfilled treat type (priority: town's lowest-stock treat), crafts it via the recipe system (consuming pool materials), deposits to pool. Produces 3–6 treats per day at 1.0 efficiency.
**Acceptance:** Pool's berry pie stock rises overnight while a Cook is on duty. Stops producing if all ingredients are missing (queues a status note in the town panel).

### W6.12 — Mood system

**What:** `Mood` enum: Content, Cheerful, Lonely, Restless, Excited. Computed from Needs + recent events (festival attended → Excited for 24h, low Social → Lonely, etc.). Used in efficiency formula.
**Acceptance:** Cat at low Social shows Lonely mood; visiting a friend changes mood to Cheerful within minutes.

### W6.13 — Inn arrivals

**What:** If town has Inn building + assigned Innkeeper, traveler cats stop every 2–3 in-game days. Travelers stay for 1 day; player can invite them to settle.
**Acceptance:** Build Inn + assign Innkeeper → next day a traveler cat arrives, lingers near the Inn, can be approached for an invitation.

### W6.14 — Festival Planner UI

**What:** Approach Town Hall + Interact (player or Mayor cat) → Festival Planner egui panel: choose festival type, set date (default: tomorrow), allocate budget (treats, decorations, music — drawn from pool), confirm. Festival becomes a scheduled event.
**Acceptance:** Schedule a Spring Bloom festival → planner shows costs vs. pool, prevents confirm if pool insufficient. Confirms successfully when sufficient.

### W6.15 — Festival execution

**What:** On festival day, cats prioritize the Festival action over their routine (scorer 0.95 in their on-duty window). Cats gather at the festival location (Town Hall plaza). Decorations spawn (festival-specific: Spring Bloom = flower garlands; Summer Sunfest = sun banners; Music Night = stage with instruments). After festival ends (3 in-game hours), results screen: friendship boosts, reputation gain, special outcomes (cross-cat friendships forming, etc.).
**Acceptance:** Festival day: 5 cats gather at Town Hall, decorations visible, festival music plays. Results screen shows +10–15 affection across attending cats and +10 reputation.

### W6.16 — Festival types: 3 implementations

**What:**
- **Spring Bloom** — flower garlands, growth boosts (cosmetic), all-trait attendance.
- **Summer Sunfest** — sun banners, sunbathing hotspot, Sociable cats engage strongly.
- **Music Night** — stage with placeholder instrument props, requires Bard role to fully execute. Town-wide mood boost lasts 24h.

**Acceptance:** Three festival types each feel distinct in visual + mechanical effect. Music Night requires a Bard or it falls back to a "quiet gathering" with smaller boosts.

### W6.17 — Authoring: 5 additional NPC archetypes

**What:** Mix of trait combos to fill role specializations:
- **Bun** — Industrious + Sociable. Excellent Builder.
- **Coco** — Adventurous + Wise. Forager + Scholar candidate.
- **Reed** — Artistic + Sociable. Bard candidate.
- **Maple** — Fussy + Wise. Mayor candidate.
- **Cinder** — Grumpy + Industrious. Loyal Cook eventually.

Each archetype has a unique color palette, name, trait combo, and (where applicable) a Personalized recipe taught at friendship cap stage 3.
**Acceptance:** Stray spawn pool now contains all 10 archetypes (5 from Phase 5 + 5 here). Roles can be staffed without role-archetype mismatches feeling forced.

### W6.18 — Cat-to-cat relationships

**What:** Populate `Friendship::with_others`. Daily routine has cats chat when collocated; each chat raises affection between them. Used for festival results ("Mochi and Bun became closer at the festival").
**Acceptance:** Two cats whose routines overlap at the bench at noon develop friendship over in-game weeks. Profile panel optionally shows their top friend.

### W6.19 — Save migration

**What:** Town entity, roles, workplaces, festivals (scheduled and historical), all serialize via moonshine.
**Acceptance:** Save mid-festival → reload → festival continues from same point with same attendees.

### W6.20 — Performance pass for ~10 active cats

**What:** Confirm the distance-based scorer tick (Phase 5 W5.18) holds with 10 cats. Add `SleepWhenFar` opt-in component for buildings + props that don't need ticking. Profile and optimize hotspots.
**Acceptance:** 10 cats + 30 buildings + populated town: frame time on M-series Mac stable at ≥ 60 FPS at default render distance.

## Risks / open questions

- **Mayor role's relationship with player.** Spec implies Mayor unlocks town projects/diplomacy. Phase 6 ships Mayor as primarily a *flavor* role — they're a cat who hangs out at Town Hall and gives advice. Real diplomacy is post-EA (multi-town).
- **Builder cat work pacing vs. player satisfaction.** Builders too fast = player feels redundant; too slow = player resents waiting. Tunable via efficiency floor and base build times. Plan a tuning sweep in Phase 7.
- **Festival decoration spawning.** Decorations should clean up after the festival ends. Make sure festival decorations are tagged so they despawn cleanly.

## Out of scope

- Multi-town gameplay (post-EA per spec §10)
- Cart routes (post-EA)
- Trade between towns (post-EA)
- Joint festivals across towns (post-EA)
- Town specialty (post-EA — data path exists, UI hidden)
- Discovered NPC settlements (post-EA)

## Estimated effort

12–18 work-days. Festival execution polish and role tuning take the most time.
