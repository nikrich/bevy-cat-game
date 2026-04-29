# Phase 5 — First NPC Cat: Personality + Friendship + Utility AI

> Status: Planned
> Depends on: Phase 1 (navmesh), Phase 2 (interior buildings), Phase 4 (treats + town pool)
> Exit criteria: A wandering cat appears at the town edge after the player builds enough to give the town reputation. Player can approach, gift treats, build friendship through three bars (trust/affection/respect) bounded by a milestone-driven cap. The cat has a personality (1–2 traits), discovers treat preferences over interactions, runs a daily routine via utility AI, lives in a player-built house, and reacts emergently to gathering spots (sunbeams, fountains).

## Goal

Make the world feel populated. One real NPC cat with the full friendship + AI stack proves the social pillar; it scales to 8–10 archetypes in Phase 6 with no architecture rewrite.

## Why now

Phase 4 gives treats their meaning. Phase 2 gives houses. Phase 1's navmesh gives walking. Without Phase 5 the world is beautiful but lonely; this is the move from "lonely cat" to "first friends" per spec §2.1.

## Deliverables

- Cat NPC entity with full component set (Cat, CatName, CatVisual, Personality, Friendship, Needs, Mood, Home, Workplace [empty for now], OriginTown, DailyRoutine, CurrentTask)
- 12 personality traits with mechanical effects
- `RelationshipBars { trust, affection, respect, current_cap }` with milestone cap raises
- Affection daily decay (-2 %, capped at 50 % floor) and easy restoration
- Treat gift system (right-click cat → gift menu drawn from MouthSlot or TownPool)
- Treat preference discovery: random reactions for first 5 gifts, then revealed
- Hand-rolled utility AI core: scorer registry, action registry (~150 LOC, replaces archived `big-brain`)
- 6 core scorers: Nap, VisitFriend, Work, Eat, Wander, SitInSunbeam
- DailyRoutine schedule per spec §7.8 (Morning/Mid-day/Afternoon/Evening/Night)
- Cat conversation/interaction prompt UI (egui dialog)
- Wandering stray spawn logic (rep > threshold)
- Invitation panel: portrait + personality preview + "welcome / let pass"
- Sunbeam entities that drift across the world with the day cycle
- 5 starting cat archetypes authored: 1 each of Sociable, Lazy, Curious, Industrious, Wise (each with the trait combo, name pool, color palette)
- Cat sleeps in player-placed beds; sleeping triggers day cycle advance per spec §3.2

## Decisions to record

- DEC-034 — Utility AI is hand-rolled in-tree, not `big-brain` (archived 2025-10-07 per the 2026-04-29 crate audit). Scorers and actions are simple traits; the Thinker tick is ~150 LOC. Same architecture as spec §7.9 — composable per-cat scorer set, highest score wins each tick. Trade-off accepted: minor maintenance burden for zero abandonment risk on a system that will see continuous extension across Phases 5–6.
- DEC-035 — Personality has 1–2 primary + 1–2 secondary traits. Combinations create distinct personalities — never hard-code archetype-specific behavior; archetypes are trait combos.
- DEC-036 — Friendship cap milestones list is data-driven (`Resource FriendshipMilestones`), not hardcoded in match arms.
- DEC-037 — No negative friendship state. Mistakes result in zero or small affection hits, never long-term damage. Spec §7.4 hard rule.
- DEC-038 — Cat visuals use color-palette swap on a shared mesh (placeholder geometry — same capsule as player for Phase 5; replaced later).

## Tech debt closed

- Tracks toward but does not close DEBT-001 (cat model). Phase 5 ships with a tinted capsule + name label.

## Work breakdown

### W5.1 — Core components

**What:** Define every component in spec §7.2. Add `#[derive(Component, Reflect)] + #[reflect(Save)]` so they round-trip through moonshine.
**Acceptance:** Spawning a cat via `commands.spawn(...)` with all components compiles. Save/load preserves every field exactly.

### W5.2 — Personality traits enum + effect tables

**What:** `Trait` enum with all 12 from spec §7.3. Effect tables map trait → modifiers (work efficiency, decay rate, treat preferences, scorer biases). Effects compose additively across traits.
**Acceptance:** A cat with `[Industrious, Shy]` has +15 % work efficiency *and* slow friendship growth. Effect query returns expected modifiers.

### W5.3 — `RelationshipBars` + milestones

**What:** Implement trust / affection / respect as floats with their own update functions. `current_cap` starts at 60. `Resource FriendshipMilestones(Vec<Milestone>)` lists cap-raising events: first gift accepted (+20), completed quest (+30), birthday attended (+20), etc. Triggering a milestone fires a `MilestoneReached` event and raises cap. Cap never decreases.
**Acceptance:** Give a cat their first treat → cap rises 60 → 80, "First gift accepted" toast appears. Subsequent gifts no longer raise cap until next milestone.

### W5.4 — Affection daily decay + floor

**What:** On each in-game day boundary, `affection -= 2.0 * decay_modifier(traits)`, floored to `current_cap * 0.5`. Trust and respect never decay.
**Acceptance:** Skip a cat for 5 in-game days → affection drops by ~10 %, floors at 50 %. Visit + treat → snaps back near max.

### W5.5 — Treat gift interaction

**What:** Right-click on a cat (or hold A on gamepad with focus) → opens gift menu showing treats from MouthSlot first, then TownPool. Selecting one consumes the treat, plays delivery animation, applies affection delta based on tier × preference. Treat preference unknown for the first 5 gifts: reactions are random within a band.
**Acceptance:** Gift a Berry Pie to a Sociable cat → +10 affection (Cooked tier baseline). Gift their declared favorite (after 5 gifts revealed) → +20 with Personalized tier bonus. Gift a treat they dislike → 0 affection, never negative.

### W5.6 — Treat preference discovery

**What:** Each cat has hidden `favorite_treats: Vec<TreatId>` and `disliked_treats`. Per gift, reaction text is randomized within a band tied to actual preference (loud-enthusiastic for Gluttonous, subtle for Shy). After 5 gifts, the cat's profile panel reveals known favorites/dislikes.
**Acceptance:** Player can deduce preferences before reveal by reaction patterns. Gluttonous cat *announces* loves loudly, Shy cat is subtle. Reveal threshold = 5 gifts.

### W5.7 — Utility AI core (hand-rolled)

**What:** Implement a small utility-AI core in-tree (replaces archived `big-brain`):
- `Scorer { kind: ScorerKind, last_score: f32 }` — multiple per cat as child entities or as a `Vec` on the cat.
- `Action { kind: ActionKind, state: ActionState }` — current action.
- `Thinker` — system that, on each tick, evaluates every scorer for the cat, picks the highest-scoring one, and transitions the cat's `Action` if it differs from the current one. Hysteresis margin (e.g. require new score > current + 0.05) to prevent thrashing.
- Scorer evaluation: a small dispatcher mapping `ScorerKind` → `fn(&World, Entity) -> f32`. Action execution: one Bevy system per `ActionKind`, run-conditioned on cats with that action.

Approximately 150 LOC for the core, excluding per-scorer logic. Lives in `src/ai/` as a new module.
**Acceptance:** Spawned cat with placeholder scorers (always-0 except Wander = 0.2) wanders aimlessly. No panics, no thrashing between actions. Adding a new scorer is a one-file change.

### W5.8 — Core scorers

**What:**
- `NapInBedScorer` — 0.3 base, 0.9 if Energy < 30 and a usable bed is nearby.
- `SitInSunbeamScorer` — 0.2 base, 0.7 if Energy < 60 and a sunbeam is within 10 m.
- `VisitFriendScorer` — `friendship_with(target) * time_since_last_visit_normalized * sociability_modifier`, max one target per tick.
- `WorkScorer` — 0.0 if no role, else 0.8 if assigned role + work in queue + time-of-day matches role's work hours.
- `EatScorer` — scales with Hunger; 0.95 if Hunger < 20.
- `WanderScorer` — 0.2 base; lower if any other scorer above 0.4.

**Acceptance:** A tired cat near a bed naps. A bored cat wanders. A hungry cat seeks food (eats from nearest pool source for now). Players can predict cat behavior from observed needs.

### W5.9 — DailyRoutine schedule

**What:** `DailyRoutine` is a list of `RoutineEntry { time_of_day: Range<f32>, preferred_action: ActionKind }` that biases scorers during the entry's window. Default routine per spec §7.8: morning (wake, eat, leave), mid-day (primary activity), afternoon (secondary), evening (social), night (return home, sleep).
**Acceptance:** Cat wakes at 6–7 in-game, leaves home at 8, returns by 22, sleeps. Personality biases routine: Adventurous wanders further afield mid-day; Lazy naps longer.

### W5.10 — Sunbeam entities

**What:** Procedural entities that spawn at sunrise, drift across the world tracking the inverse of sun direction (light through windows or open ground), despawn at sunset. Cats with `SitInSunbeamScorer` engaging them gain a "Sunbathed" comfort buff (small Affection regen toward the player if friendship > 30).
**Acceptance:** Visible sunbeam patches drift across floor and grass during day. Cats sometimes choose to sit in them at midday. The "Sunbathed" buff appears on the cat's mood for 5 in-game minutes after.

### W5.11 — Cat home assignment

**What:** When friendship reaches 50, the cat asks to "move into a house." Player gets a prompt to assign a `Building` entity as their `Home`. Cat sleeps there nightly. If unassigned, cat sleeps near a campfire (procedural fallback bedding spot).
**Acceptance:** Build a small cottage with a bed → at friendship 50, prompt to assign → assign the cottage → cat sleeps there at night. Beds outside any building still work as fallback.

### W5.12 — Wandering stray spawn

**What:** When the spawn town's reputation > 30 (Phase 6 reputation; for Phase 5 use a placeholder counter incremented by Cozy Score and player exploration milestones), spawn a stray cat at the town's outer boundary every 24 in-game hours. The stray's personality is randomly rolled. Player approaches → invitation panel.
**Acceptance:** Build 3 small structures + decorate → reputation crosses 30 → next morning a stray appears at town edge. Player can ignore (cat wanders for an in-game day, then leaves) or approach.

### W5.13 — Invitation panel

**What:** egui modal showing: cat portrait (color-tinted capsule render), name (from procedural name pool), 1–2 primary traits + 1 secondary, an empathic flavor sentence ("They look like they could use a warm meal."), buttons "Welcome" / "Let pass."
**Acceptance:** Welcoming a stray spawns them as a permanent NPC, sets `OriginTown`, gives them a default home assignment (or fallback), starts their daily routine. "Let pass" despawns the stray after a polite goodbye.

### W5.14 — Cat profile panel

**What:** Approach cat + Hold Interact → opens profile panel: portrait, name, traits, friendship bars (filled to current/cap), revealed favorites/dislikes (after 5 gifts), current task ("Headed home for tea"), home/workplace assignments.
**Acceptance:** Profile updates live: friendship bars rise as gifts are accepted, current task changes as routine progresses.

### W5.15 — Authoring: 5 starting cat archetypes

**What:** Procedural rolls produce a stray with one of 5 named archetypes:
- **Mochi** — Sociable + Curious. Loves baked goods. Hates fish.
- **Olive** — Lazy + Wise. Loves herb tea. Hates anything spicy.
- **Pip** — Curious + Mischievous. Loves berries. Hates mushroom stew.
- **Tofu** — Industrious + Shy. Loves fish. Hates flowery treats.
- **Sage** — Wise + Sociable. Loves rare herbs (Personalized treat unlock at high friendship). Hates dried fish.

Each gets a unique color palette and a unique-favorite Personalized treat recipe taught at friendship cap stage 3.
**Acceptance:** Across 10 spawn rolls, all 5 archetypes appear. Each plays distinctly. Sage taught a recipe that no other cat teaches.

### W5.16 — Conversation / chat prompt

**What:** Lightweight "conversation" — interact with a friend cat → random flavor line drawn from a per-trait pool. No dialogue tree. Affection +1 per conversation, capped at 5/day.
**Acceptance:** Talking to Sociable Mochi yields chatty lines; Shy Tofu yields shorter, cuter lines. Conversation cap prevents spamming.

### W5.17 — Save migration

**What:** All cat data round-trips through moonshine. Sunbeams are transient (regenerated from sun position on load).
**Acceptance:** Save mid-day with 2 cats in distinct routine states, reload → cats are at their last known position with correct task and friendship state.

### W5.18 — Performance: distance-based scorer tick

**What:** Cats further than 50 m from the player tick scorers at 1 Hz instead of every frame; further than 200 m, 0.2 Hz with waypoint teleport on action change. Per spec §13.4.
**Acceptance:** With 8 cats spawned across the map, frame time impact < 1 ms. Distant cats still progress through their routines, just discretely.

## Risks / open questions

- **Hand-rolled utility AI vs. `big-brain`.** `big-brain` was archived 2025-10-07; the closest alternative `bevior_tree` is behavior trees, not utility AI, and is not a drop-in for the spec §7.9 architecture. Hand-rolling preserves the architecture exactly and eliminates abandonment risk. Budget +1 day vs. adopting a crate, paid back the first time the system is extended.
- **Capsule placeholder for cats.** Visual identity is thin until cat models land. Mitigation: distinct color palettes + name labels + idle animations placeholder ("breathing" Y-bob).
- **Affection floor + cap interaction.** Edge case: cap raised mid-decay window. Lock convention early: floor is `current_cap * 0.5` *at decay time*, not the historical cap.

## Out of scope

- Roles and workplaces (Phase 6 — `Workplace` field stays empty)
- Multiple cats interacting with each other (Phase 6 — friendship_with_others stays empty)
- Festivals (Phase 6)
- Cat life cycle: kittens, elders (post-EA per spec §7.13)
- Scholar lore research (post-EA)
- Cat departures (post-EA)

## Estimated effort

12–18 work-days. Utility AI tuning and treat preference reveal pacing are slow-cook items; budget time for play-feel iteration.
