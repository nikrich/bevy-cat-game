# Phase 4 — Crafting + Town Pool + Treats

> Status: Planned
> Depends on: Phase 2
> Exit criteria: Workbenches exist as physical placeable buildings. Each workbench exposes its own recipe set. Recipes are discovered, not all-given. Treats exist with tiers, spoilage, and refrigeration. The current `Inventory` resource is replaced by a `TownPool` that is bottomless and ambient. Mouth slot remains the single visible carry. BuildJob materials check the pool and queue if insufficient.

## Goal

Replace the current single inventory + flat recipe list with the spec's deeper craft system: workbenches as places, recipes as discoveries, treats as social currency. Town pool removes inventory-hunting friction. Build queue respects materials but never blocks the player.

## Why now

Phase 5's NPC cats use treats as their primary affection currency. Phase 3's blueprints expand into BuildJobs that need materials. Phase 6's roles (Cook, Forager, Builder) all hinge on the town pool. Phase 4 must land before NPC cats become interesting.

This phase can run in parallel with Phase 3 if there are two work streams, since the only Phase 3 dependency is the blueprint expansion that calls into BuildJob materials checks (W3.6).

## Deliverables

- 5 workbench types as placeable pieces: Carpentry, Stonemason, Kitchen, Sewing, Drafting
- Recipe registry with discovery sources (starting / taught / found / role-unlocked)
- Treat data: tier (Foraged/Cooked/Personalized/Legendary), favorite-of, spoilage, cozy boost
- Treat spoilage tick + refrigeration extension
- `TownPool` resource replacing `Inventory` (bottomless, ambient, location-scoped to current town)
- Mouth slot reserved for *intentional* items only
- BuildJob materials check: deduct on completion, queue waiting if pool insufficient
- Migration: existing 8 recipes redistributed across workbenches
- ~20 new recipes spanning all 5 workbenches

## Decisions to record

- DEC-029 — Workbenches are physical placeables; their *menu* is the crafting UI, the *workbench* is the place. No global crafting menu.
- DEC-030 — `TownPool` is a per-town resource. Phase 4 ships with one town (the spawn town); Phase 6 generalizes to multi-town.
- DEC-031 — Recipe discovery model: `RecipeSource::Starting | TaughtBy(Trait) | FoundIn(LoreKind) | RoleUnlock(Role, Level)`.
- DEC-032 — Mouth slot is single-item, visible, intentional carry. Materials never go to mouth slot — they go straight to TownPool. Player explicitly picks an item from a workbench output to carry.
- DEC-033 — Treat spoilage uses absolute in-game days; refrigeration multiplies shelf life by 3×.

## Tech debt closed

- DEBT-010 — already closed; Phase 4 carries forward the Spiritfarer-inspired panel chrome into the egui port.

## Work breakdown

### W4.1 — `TownPool` resource

**What:** `Resource TownPool { items: HashMap<ItemId, u32> }`. Ambient: any system inside the player's current town reads/writes this. Phase 4 ships one global pool because there's only one town; Phase 6 makes it per-town with boundary-based selection.
**Acceptance:** Replace all `Inventory` reads with `TownPool` reads. Save/load preserves pool. Adding 100 oak logs to pool works in one tick.

### W4.2 — Migrate `Inventory` consumers

**What:** Find every consumer of the old `Inventory` resource. Update gathering, crafting, building refunds, save/load, UI. Delete `inventory::Inventory`. Keep a thin compatibility `MouthSlot` resource (already shipped in Phase 2) for the visible carry.
**Acceptance:** Game compiles with no `Inventory` references. Gathering oak logs adds them to TownPool. Hotbar shows pool counts.

### W4.3 — Workbench piece category

**What:** Add `Workbench { kind: WorkbenchKind }` component. Workbench kinds: Carpentry, Stonemason, Kitchen, Sewing, Drafting. Each kind has its own piece in the kit (Phase 2's authoring extends here). Placing a workbench works like any other building piece.
**Acceptance:** All 5 workbench types placeable from the build hotbar. Each has a distinct mesh. Place one in a building.

### W4.4 — Workbench interaction

**What:** Approach a workbench, press Interact (E / A button) → opens egui crafting menu filtered to that workbench's recipes. Closing returns to gameplay. Player must remain within 2 m or the menu closes.
**Acceptance:** Approaching Carpentry shows wood-related recipes only. Approaching Kitchen shows treat recipes only. No global menu opens from anywhere.

### W4.5 — Recipe registry refactor

**What:** Convert recipe data to `Recipe { id, ingredients: Vec<(ItemFamily, u32)>, output: ItemId, workbench: WorkbenchKind, source: RecipeSource, craft_time: f32 }`. Migrate existing 8 recipes to the new structure.
**Acceptance:** Old recipes still craft. Recipes filter correctly per workbench. Source field defaults to `Starting` for migrations.

### W4.6 — Recipe discovery

**What:**
- `Starting`: known at game start.
- `TaughtBy(Trait)`: friendship milestone with a cat carrying that trait teaches the recipe (Phase 5 hook).
- `FoundIn(LoreKind)`: unlocked when the player picks up a lore artifact (LoreKind = RecipeBook | Scroll | Tablet).
- `RoleUnlock(Role, Level)`: unlocked when an NPC reaches a role level (Phase 6 hook).

Phase 4 implements `Starting` + `FoundIn`. Lore artifacts spawn in dig spots (placeholder dig spot system).
**Acceptance:** Walk to a dig spot → press Interact → roll on loot table → if recipe scroll drops, recipe added to known set, recipe appears in matching workbench. UI shows a "+1 Recipe Discovered" toast.

### W4.7 — Treat data and tiers

**What:** New `Treat { id, tier, cozy_boost, affection_value, spoilage_days, refrigerated_days }`. Treats are items, but with treat-specific data. Tiers: Foraged (+5), Cooked (+10), Personalized (+20), Legendary (+30) per spec §7.5.
**Acceptance:** Berries are Foraged. Berry pie is Cooked. The "Sage Stew" recipe (taught by Wise cat in Phase 5) is Personalized. Legendary treats reserved for milestone moments — not shipped yet.

### W4.8 — Treat spoilage tick

**What:** System ticks `spoilage_remaining_days` for treats in TownPool every in-game day boundary. At 0, treat is removed from pool with a UI notification. Refrigeration: any building with a "Refrigerator" piece extends shelf life of all treats in that town's pool by `refrigerated_days` factor.
**Acceptance:** Berries spoil after 3 in-game days if no refrigeration. With a Refrigerator piece, last 9 in-game days. Spoilage notifications fire correctly.

### W4.9 — Refrigerator piece

**What:** Add Refrigerator to the kitchen fixtures category. Visually similar to a chest icon; functionally adds the spoilage extension flag to its town.
**Acceptance:** Place a Refrigerator. Treats in the pool now show extended shelf life in the UI.

### W4.10 — `MouthSlot` for intentional carry only

**What:** Lock down the rule: gathering goes to TownPool. Crafted items default to TownPool unless the player explicitly chose "Take" from the workbench output prompt, in which case the item lands in MouthSlot. Treats given as gifts (Phase 5) carry from MouthSlot.
**Acceptance:** Gather 5 berries → all in TownPool. Craft a Berry Pie → prompt asks "Take" or "Add to pool" → "Take" puts pie in MouthSlot, visible at cat's mouth.

### W4.11 — BuildJob materials check

**What:** Before a job starts construction, verify required materials are in TownPool. If yes, reserve them (decrement). If no, mark job `BuildJobStatus::WaitingOnMaterials`. Foragers (Phase 6) will eventually fulfill, then any builder will pick the job up.
**Acceptance:** Place a wall with insufficient stone → job spawns in WaitingOnMaterials, ghost is dimmed and tagged in the UI. Add stone to pool → next tick, job moves to Pending and can be started. Player is never blocked from placing.

### W4.12 — Material refund on delete

**What:** Phase 2's piece-delete refund now feeds the TownPool (replacing the Phase 2 placeholder).
**Acceptance:** Place a stone wall (-4 stone), delete it (+4 stone). Net pool unchanged.

### W4.13 — Authoring: 20 new recipes

**What:**
- Carpentry: Plank → Wall, Plank → Floor, Plank → Door, Beam, Bookshelf, Window-frame
- Stonemason: Stone Block, Stone Wall, Slate Roof Tile, Hearth, Cobblestone path
- Kitchen: Berry Pie (Cooked), Mushroom Stew (Cooked), Fish Soup (Cooked), Herb Tea (Cooked), Sage Stew (Personalized, Phase 5 unlock)
- Sewing: Wool Rug, Curtains, Cushion, Tapestry
- Drafting: Blueprint Scroll (used for sharing — late content; ship as discoverable)

**Acceptance:** All 20 craftable from the appropriate workbench with the right ingredients. Outputs land in pool (or mouth slot). Times feel right (1–8 s per recipe).

### W4.14 — Crafting menu UX polish

**What:** egui port of the Spiritfarer-inspired panel. Categories per workbench. Show ingredient list with greyed-out items the player lacks. Hover a recipe → show tooltip with description and source ("Found in dig spot, Forest").
**Acceptance:** Visual parity with current painted-panel design. New egui implementation feels as good as the prior Bevy UI version. No information regressions.

### W4.15 — Save migration

**What:** TownPool, known recipes, treats with spoilage timers, all serialize via moonshine.
**Acceptance:** Save with 5 recipes known, 12 treats in various spoilage states, reload → all match exactly.

## Risks / open questions

- **Cooked treats need Cook NPC for sustainable production.** Phase 4 lets the player cook directly at the Kitchen workbench. Phase 6 introduces Cook role and daily auto-production. Both paths must coexist.
- **Recipe scroll loot table.** First version is hand-tuned. Plan for tuning sweep in Phase 7.
- **Multi-town pool boundary lookup.** Phase 4 ships one global pool. Phase 6 must add boundary lookup without breaking save format. Plan the schema accordingly.

## Out of scope

- Cook role daily production (Phase 6)
- Forager role gathering (Phase 6)
- Trade between towns (post-EA)
- Recipe sharing between players (post-EA)

## Estimated effort

7–10 work-days. Treat spoilage and recipe discovery are the gnarlier items.
