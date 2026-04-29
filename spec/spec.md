# Cat World — Game Design & Technical Specification

## 1. Vision

Cat World is a peaceful, open-world crafting and civilization sim where the player is a stray cat in a post-human world. Through gathering, building, and friendship, the player evolves from a lone survivor into the architect of an interconnected cat civilization spread across procedurally generated biomes.

The game's emotional arc is the journey from solitude to community to legacy. Every mechanic serves the fantasy of cats inheriting a gentle, humanless world and slowly rediscovering what civilization can mean for them.

### Pillars

1. **Beautiful intricate building** — modular, fluid, no mode-switch construction at human scale with explorable interiors.
2. **Genuine friendships** — Sims-depth relationships with no negative vibes; treats, gifts, and shared experiences as social currency.
3. **Emergent town life** — autonomous cats with personalities and routines making the world feel alive without player input.
4. **Player-shaped civilization** — terrain editing, blueprint design, cart routes, and trade networks are all expressions of player taste.
5. **Cozy at all costs** — no fail states, no punishment, no urgency. The game waits for the player.

### Reference points

- Tiny Glade (building feel, auto-flatten terrain, fluid editing)
- Stardew Valley (friendship system, NPC schedules, festival cadence)
- Animal Crossing: New Horizons (interior decoration depth, cozy tone)
- Stray (cat-as-protagonist physicality, low-poly aesthetic)
- A Short Hike (gathering joy, world-as-reward)
- The Sims 4 (relationship depth, build mode polish)
- Townscaper / Dorfromantik (low-poly stylization, snap-based building)

---

## 2. Game Loop & Progression

### 2.1 Phase progression

The game has four loose phases. Players move between them organically based on town development, not gates.

**Phase 1 — The Lonely Cat (hours 0–5).** Player is alone. Forages and builds a small shelter. World is beautiful but quiet. Distant smoke hints at other cats existing somewhere. Gathering and basic crafting are the only mechanics. Goal: another cat notices the player and approaches.

**Phase 2 — First Friends (hours 5–20).** A few cats appear. Player befriends through gifts, conversation, shared meals. First role assignments — Forager, Builder. Town is small and intimate; every cat is a known character. Goal: a stable village of 5–8 cats with identity.

**Phase 3 — The Architect (hours 20–60).** Town has grown. Friendship management is lighter; close-circle friendships stay high naturally. Focus shifts to expansion: laying out neighborhoods, designing buildings, planning festivals, deciding town character. New cats arrive regularly. Goal: a thriving town that runs itself with the player as visionary.

**Phase 4 — The Civilization (hours 60+).** Player founds second (and third, etc.) towns in different biomes. Cart routes connect them. Trade networks emerge. Joint festivals across towns. Player manages a small civilization. Other procedurally-discovered cat settlements can become trade partners. Goal: define what cat civilization *is* — utopia, mercantile network, artistic commune, isolated peace.

### 2.2 Core loops

**Moment-to-moment loop (seconds):** walk → notice something → interact → small reward → continue walking.

**Session loop (minutes):** decide a goal → gather/build/socialize toward it → small completion → choose next goal.

**Daily loop (in-game day):** morning routine (check on friends, attend deliveries) → mid-day project work → afternoon socializing → evening gathering or festival → sleep.

**Weekly loop:** new arrivals consideration, festival cadence, trade route balance review, larger building projects.

**Macro loop (multi-hour):** town expansion, second town founding, trade network development, civilization milestones.

### 2.3 No fail states

The game has no death, no eviction, no loss. Mechanics that *feel* like challenges are framed positively:

- "Hunger" is replaced with comfort buffs from eating, not penalties from not eating.
- Cats never leave the town in anger; they may leave for happy reasons (joining another town's love story, founding their own settlement).
- Storms and bad weather are atmospheric, never destructive to buildings.
- Friendship decay is gentle and easily restored; it nudges socializing rather than punishing absence.

---

## 3. The Player Character (Cat)

### 3.1 Movement

- **WASD / left stick** — walk
- **Shift / left stick click** — sprint (the zoomies); has a soft cooldown where the cat pants briefly after extended use
- **Space / A** — jump
- **Auto-climb** — walking into low climbable surfaces (chairs, fences, low rooflines) triggers automatic climb. No button required.
- **Crouch / B** — slow stalking walk; can squeeze under low gaps. Mostly for charm and cat fantasy.

### 3.2 Cat-specific actions

Actions that reinforce the cat fantasy and are mechanically meaningful:

- **Knocking things off surfaces** — small physics objects (cups, books, decorations) can be batted off shelves and tables. Mostly for fun; some objects break and require crafting replacements.
- **Sitting in sunbeams** — sunbeams drift across the world with the day cycle. Sitting in one applies a small "Sunbathed" comfort buff.
- **Kneading** — interaction available on soft surfaces (beds, moss, dough). Builds Comfort meter.
- **Grooming** — passive idle behavior; can be triggered manually for a small Focus restore.
- **Sleeping** — only in beds the player has placed (theirs or in friend houses with permission). Restores Energy fully and triggers the day cycle to advance.

### 3.3 Carrying items in mouth

The cat physically carries small items in its mouth. This is a *core charm mechanic* and applies broadly:

- Picking up small items shows them in the cat's mouth as it walks.
- Building pieces are carried to placement spots in the mouth.
- Gifts and treats are delivered in the mouth to recipients.
- Special quest items (keys, letters, found objects) carry in the mouth as a visual indicator of "carrying a thing."

Carrying a single item disables some other actions (can't groom, can't meow). Drop with the same button used to pick up.

### 3.4 Inventory model

The player has two inventories:

- **Mouth slot** — single item the cat is physically carrying. Visible on the model.
- **Town pool** (shared with town) — bottomless storage for materials and resources. Available everywhere within a town's boundaries.

Materials gathered (logs, stone, berries, etc.) go directly to the town pool. The mouth slot is reserved for *intentional* items the player wants to deliver, place, or display.

There is no weight system, no encumbrance, no inventory management chore. Stack sizes are effectively infinite.

---

## 4. Camera & Controls

### 4.1 Camera system

Single camera, smooth zoom range. Default behavior:

- **Right-click drag / right stick** — orbit around the cat
- **Scroll wheel / triggers** — zoom in/out
- **Zoom levels:**
  - **Intimate (1–3m)** — close third-person, behind cat. For exploring interiors and emotional moments.
  - **Standard (5–10m)** — default play position. Slightly elevated, slight tilt.
  - **Strategic (15–30m)** — pulled back overhead-ish view. For surveying town, planning builds.
  - **Map (30m+)** — fully overhead, near top-down. For planning cart routes and viewing the world.

The camera should auto-suggest zoom level by context (e.g., entering a building zooms in, opening the build mode pulls back slightly), but the player can always override.

### 4.2 Interior visibility

When the camera is inside a building, walls between camera and cat become translucent. Roof is hidden. This is implemented as a per-room shader that fades based on camera position and direction.

### 4.3 Input system

Use **leafwing-input-manager** for action abstraction from day one. All inputs route through named actions (`Move`, `Jump`, `Interact`, `PlacePiece`, etc.) bound to either keyboard/mouse or gamepad.

### 4.4 Primary input scheme: PC mouse + keyboard

- **WASD** — movement
- **Mouse** — camera rotation, hover, cursor for placement
- **Left click** — interact / place
- **Right click** — modify / contextual menu / camera orbit (held)
- **E** — interact prompt confirm
- **Tab** — open inventory / build menu
- **B** — toggle build cursor
- **1–9** — hotbar selection
- **Scroll** — zoom (default), or cycle materials/rotation while placing
- **Shift** — sprint
- **Space** — jump
- **Ctrl+Z / Ctrl+Y** — undo / redo build actions

### 4.5 Controller support (Steam Deck / Switch port readiness)

Day-one controller support, designed natively rather than auto-mapped:

- **Left stick** — movement
- **Right stick** — camera
- **Triggers** — zoom in/out
- **A** — interact
- **B** — jump
- **X** — sprint
- **Y** — context action (varies)
- **D-pad** — hotbar cycling
- **Shoulders** — rotate piece while placing
- **Hold LT** — radial menu (categories, materials, blueprints)
- **Touchpad / select** — open menus

---

## 5. World & Terrain

### 5.1 World structure

- **Bounded procedurally generated world.** Approximate size: 2km × 2km. Large enough for 4–6 player towns plus wilderness; small enough to walk across in 10–15 minutes.
- **Single persistent world**, no streaming. All chunks loaded simultaneously.
- **Deterministic generation** from a seed. Same seed = identical world. World does not expand; players fill what's there.
- **PCG already implemented and seedable** in the existing project.

### 5.2 Biomes

The world includes multiple biomes, each with distinct visual identity, native resources, and town specializations:

| Biome | Native Resources | Specialty Crafts |
|---|---|---|
| Forest | Oak, pine, birch logs; mushrooms; berries; herbs; feathers | Wooden furniture, woodland teas |
| Mountain | Stone, granite, gemstones, mountain herbs | Stone architecture, gem-crafted decor |
| Coastal | Fish, shells, seaweed, salt, driftwood, pearls | Seafood treats, shell-inlaid items |
| Meadow | Wildflowers, honey, wool, grain | Flower arrangements, baked goods |
| Wetland | Reeds, peat, lily pads, special herbs | Reed weaving, marsh teas |
| Sandstone/Desert | Clay, sandstone, succulents, sun-dried foods | Clay pottery, preserved foods |

Biome distribution is determined by PCG. Mountains naturally form ridges, forests cluster, coasts follow water boundaries.

### 5.3 Terrain system

**Vertex-height grid.** Terrain is a 2D grid of vertices, each with a float height. Tiles are quads (rendered as two triangles) with corners at vertex heights. This produces gently sloped, low-poly terrain matching the existing aesthetic.

**Chunked mesh generation.** World is divided into chunks (e.g., 32×32 vertices). Each chunk is a separate mesh entity. Edits flag chunks dirty; a system regenerates dirty chunks each frame (capped at N per frame to avoid hitches).

**Per-tile material.** Each tile has a material/biome ID determining its texture (grass, sand, stone, etc.). Material can be edited like height. Slopes auto-blend materials at their edges via shader.

**Slope-based material override.** Faces with normal angle exceeding a threshold (e.g., 45°) automatically render as rock material regardless of the tile's base material. Cheap, looks great for mountains.

### 5.4 Terrain editing tools

Available in the player's hotbar/build menu:

- **Raise brush** — hold to raise vertices under cursor. Falloff curve determines softness.
- **Lower brush** — same, inverted.
- **Flatten brush** — pick a target height (default: cursor's current ground height). Drags vertices toward target.
- **Smooth brush** — averages neighboring vertex heights. Cleans up jagged edges.
- **Material paint** — change tile material/biome under cursor. For decorative paths, sand patches, etc.
- **Stamp tools (later content)** — pre-shaped terrain modifications: small hill, valley, cliff edge, riverbed.

Brush radius and intensity are adjustable via scroll wheel while the brush is active.

### 5.5 Auto-flatten on building placement

When the player draws a floor plan or places a structural piece:

1. Compute building footprint AABB.
2. Sample median ground height under footprint.
3. Smoothly raise/lower all vertices inside footprint to that height.
4. Create a soft skirt of vertices in a 1–2 tile ring around the footprint that blend back to natural terrain via interpolation.

Result: placing a building on uneven terrain produces a clean, level foundation with natural-looking transitions outside the walls. Reduces friction in the most common build use case.

### 5.6 Pathfinding considerations

- Use **oxidized_navigation** for navmesh generation.
- Navmesh regenerates in chunks affected by terrain edits.
- Cats can traverse slopes up to a configurable angle (e.g., 30°). Steeper terrain requires built stairs/paths.
- Stairs and cart paths are explicit navmesh overrides allowing traversal of steep height changes.

---

## 6. Building System

### 6.1 Core principles

- **No build mode.** Building happens in-world, in real-time, while playing.
- **Pieces remain hot-editable forever.** No mesh combining, no finalization step.
- **Construction takes time.** Pieces don't pop into existence; cats physically build them.
- **Snap-based, low-poly modular kit.** Architectural quality comes from well-designed pieces, not pixel-perfect placement.
- **Beautiful by default.** Every piece is hand-modeled with proper proportions, trim, and detail.

### 6.2 Piece categories

**Structural pieces** (define building shape):

- Walls (full, with window, with door, half-height, corner)
- Floors (per-material, with decorative variants)
- Ceilings (flat, vaulted, beamed)
- Roofs (gable, hip, mansard, dormer, ridge cap, gable-end, chimney)
- Stairs (straight, L-shape, spiral)
- Foundations (stone, wood post)
- Columns and beams
- Bay windows, arches, awnings (snap-on modifiers)

**Interior fixtures**:

- Fireplaces (small hearth, large stone, kitchen stove)
- Sinks, counters, kitchen islands
- Built-in shelves and cabinets
- Window seats
- Mantels and trim
- Wallpaper / paint / wood paneling (wall finishes — applied as overlay layer)

**Furniture**:

- Beds (basic, plush, four-poster, kitten cradle)
- Sofas, armchairs, ottomans
- Tables (dining, side, coffee, work)
- Chairs, stools, benches
- Storage (chests, dressers, wardrobes)
- Bookshelves (with books, empty)
- Desks
- Cushions and pillows (small placeable items)

**Decoration**:

- Rugs and carpets
- Wall art (paintings, photographs, tapestries)
- Plants (potted, hanging, trailing)
- Lighting (lamps, sconces, chandeliers, candles, lanterns) — *actual light sources*
- Trinkets (small placeable items on shelves and tables)
- Curtains
- Cat-specific: scratching posts, cat trees, cardboard boxes, window perches

**Outdoor**:

- Fences (post, picket, stone wall, hedge)
- Gates and arches
- Garden paths (different materials)
- Planter boxes, flower beds
- Trellises, pergolas
- Garden lights and lanterns
- Mailboxes, signs, weather vanes
- Outdoor seating, fountains, fire pits, bird baths

### 6.3 Piece scale

- Wall segments are **2m or 4m wide** (not 1m). Players build at human scale, not pixel-art scale.
- Standard interior height: **3m**.
- Grid resolution: **0.5m** primary snap, **0.25m** fine-snap (held modifier key).

### 6.4 Snapping system

Each piece has snap points — positions and orientations where it connects to neighbors. The snapping algorithm:

1. When placing, query all pieces within ~2m of cursor.
2. Find compatible snap points (wall-end-to-wall-end, wall-bottom-to-floor-edge, etc.).
3. Snap to the nearest compatible point.
4. Default to grid snap if no neighbors.

Visual feedback:

- Ghost piece shows preview at snap location.
- Green ghost = valid placement.
- Red ghost = invalid (overlapping, no snap target, insufficient space).
- Snap connection points highlight when within range.

### 6.5 Build-while-playing flow

The player builds without entering a mode:

1. Open hotbar (always available, persistent UI).
2. Select a piece from hotbar.
3. Cursor / ghost preview shows where it would go.
4. Click to place — this creates a `BuildJob` entity at that position.
5. The player's cat (or an assigned Builder) walks to the spot, plays construction animation, the piece progressively builds over time.
6. Player can immediately select another piece and queue more jobs nearby, or walk away.
7. Construction continues in the background; player can return to see progress.

Construction time per piece scales with complexity:

- Small decoration: 2–5 seconds
- Furniture: 5–10 seconds
- Wall segment: 10–20 seconds
- Floor: 5–10 seconds per tile
- Roof piece: 15–30 seconds

Multiple cats can work on multiple jobs in parallel. A queue of 50 pieces with 5 builders completes ~10× faster than with one builder.

### 6.6 Edit interactions on existing pieces

Hover any placed piece for inline interactions:

- **Click+drag** — reposition the piece (re-snaps at destination).
- **Right-click** — context menu (delete, duplicate, copy material).
- **Scroll while hovering** — cycle material on that piece (live, no rebuild).
- **R while hovering** — rotate (90° increments by default, hold shift for fine).
- **Delete key** — remove piece. Materials refunded to town pool.

Multi-select via shift+drag-box selects multiple pieces for batch operations.

### 6.7 Material system

Each piece can be rendered in any compatible material. A "wall" mesh is one geometry; the material (oak, pine, birch, stone, sandstone, painted, etc.) is a separate property. Switching materials on an existing piece is instant — just swaps the material handle on the entity.

Materials available depend on the player's progression and biome access:

- **Starting materials**: oak, basic stone, dirt path
- **Forest unlocks**: pine, birch, more wood variants
- **Mountain unlocks**: granite, slate, marble (rare)
- **Coastal unlocks**: driftwood, shell-inlay, weathered wood
- **Through trade**: any biome's specialty materials become accessible if traded for

### 6.8 Roof system

Roofs are the most architecturally challenging element. Two-tier approach:

**Modular roof kit** (default):

- Gable end pieces, hip pieces, valley pieces, ridge caps, dormers
- Manual placement via snap, like other pieces
- Roof material applied as an overlay layer (slate, terracotta, thatch, wood shake, copper)

**Auto-roof tool** (advanced, late content):

- Player draws a building footprint
- System generates appropriate roof geometry (hipped, gabled based on shape)
- Roof material applied as overlay
- Player can edit individual generated pieces afterward if desired

Auto-roof is procedural mesh generation; not required for v1 but worth building once core systems are stable.

### 6.9 Floor plans

The "floor plan tool" lets the player sketch a building's outline directly on the ground, abstracting away individual piece placement:

1. Select Floor Plan tool from hotbar.
2. Click corners on terrain to define an outline (auto-snaps to grid).
3. Specify room divisions (interior walls).
4. Mark doors and windows on the outline.
5. Choose building type tag (cottage, shop, workshop, etc.) and material preset.
6. Confirm — system creates a `BuildingBlueprint` entity with the floor plan data.

Builder cats consume floor plans from the queue:

1. Find an available BuildingBlueprint without an assigned builder.
2. Walk to site.
3. Construct foundation, then walls, then roof, then doors/windows, then interior basics — piece by piece, over time.
4. For unspecified details (interior walls' material, exact window placement), Builder applies sensible defaults from the blueprint library.

Floor plans can be:

- **Rough** — outline + type only. Builder fills in everything from defaults.
- **Specific** — outline + material choices + door/window placement. Builder constructs to spec.
- **Fully detailed** — every piece specified, including interior. Builder just constructs.

### 6.10 Blueprint library

Players can save any constructed building as a Blueprint:

1. Multi-select all pieces of the building (or use "Save Building" prompt on a tagged structure).
2. Name the blueprint.
3. Choose tags (Cottage, Shop, etc.) and style (Cottagecore, Stone, Rustic).
4. Blueprint saved as `.bp.ron` file in the project's blueprints folder.

Blueprints can be:

- Re-placed by the player (point and place, builders construct).
- Used by NPC cats for their own homes (when expanding the town).
- Shared with the community (file is human-readable RON).

### 6.11 Cozy score

Each building has a Cozy Score derived from its contents:

- Each decoration contributes a `cozy_value` (defined per-piece in asset data).
- Lighting contributes if the building has light sources active at night.
- Functional completeness boosts (kitchen has stove + sink + counter? +bonus).
- Interior temperature/atmosphere bonuses (fireplace, rug, plants).
- Penalties for empty rooms or unfinished walls.

Cozy Score affects:

- **Resident happiness** — cats living in cozy homes are happier.
- **Resident attraction** — higher-tier cats only move into cozy homes.
- **Visitor reactions** — guest cats comment positively on high-cozy spaces.

The score is *visible* in build mode as a soft heart particle effect intensity around the building. The cozier the home, the more particles drift up. Subtle but rewarding.

---

## 7. Cats — NPC System

### 7.1 Cat archetypes

Cats are not a separate class from the player; they share components and many behaviors. NPCs are entities with the same `Cat` marker plus AI components.

### 7.2 Cat components

```rust
#[derive(Component)] struct Cat;
#[derive(Component)] struct CatName(String);
#[derive(Component)] struct CatVisual { breed: BreedKind, color_palette: Palette }

#[derive(Component)] struct Personality {
    primary_traits: Vec<Trait>,    // 1-2 dominant traits
    secondary_traits: Vec<Trait>,  // 1-2 minor traits
    favorite_treats: Vec<TreatId>,
    disliked_treats: Vec<TreatId>,
    favorite_decorations: Vec<StyleTag>,
}

#[derive(Component)] struct Friendship {
    with_player: RelationshipBars,
    with_others: HashMap<Entity, RelationshipBars>,
}

#[derive(Component)] struct RelationshipBars {
    trust: f32,           // 0-100, slow to build
    affection: f32,       // 0-100, daily fluctuation
    respect: f32,         // 0-100, milestone-driven
    current_cap: f32,     // ceiling on combined score, raised by milestones
    last_interaction: GameTime,
}

#[derive(Component)] struct Needs {
    hunger: f32,         // never reaches 0; influences mood
    social: f32,
    comfort: f32,
    energy: f32,
}

#[derive(Component)] struct Mood(MoodKind); // Content, Cheerful, Lonely, Restless, Excited

#[derive(Component)] struct Role(Option<RoleKind>);
#[derive(Component)] struct CurrentTask(Option<TaskKind>);

#[derive(Component)] struct Home(Option<Entity>);       // their house entity
#[derive(Component)] struct Workplace(Option<Entity>);  // their job building entity
#[derive(Component)] struct OriginTown(Entity);         // which town they belong to

#[derive(Component)] struct DailyRoutine {
    schedule: Vec<RoutineEntry>,
    current_index: usize,
}
```

### 7.3 Personality traits

Traits are tags that affect AI scoring, conversation responses, gift preferences, and role suitability:

- **Adventurous** — explores edges of map, brings back rare items, prefers travel
- **Artistic** — decorates extensively, prefers buildings with art objects, gives art gifts
- **Curious** — investigates new buildings, asks questions, learns roles fast
- **Fussy** — only happy in high-Cozy-Score homes, dislikes some treats strongly
- **Gluttonous** — strong treat motivation, eats often, larger build
- **Grumpy** — slow to befriend but loyal once close, prefers solitude
- **Industrious** — high work efficiency, prefers active roles, dislikes idleness
- **Lazy** — low base efficiency but slow decay, naps frequently, charming
- **Shy** — slow friendship growth but stable, dislikes festivals
- **Sociable** — needs group events, friendship grows fast, hosts gatherings
- **Mischievous** — knocks things off shelves, occasional pranks, playful
- **Wise** — gives advice, unlocks lore, attracts other cats

A cat has 1–2 primary traits and 1–2 secondary. Combinations create distinct personalities.

### 7.4 Friendship system

**Three relationship bars, one cap.**

- **Trust** (slow): built by consistent gifts, fulfilling requests, attendance at their important moments. Max 100.
- **Affection** (fluctuating): built by favorite treats, grooming, sleeping nearby. Max 100. Daily fluctuation up to ±10%.
- **Respect** (milestone-driven): built by skilled construction, completing big projects, leadership choices. Max 100.

The **cap** (sum ceiling) starts at, e.g., 60 and rises through milestones:

- First gift accepted: cap +20
- Completed a quest for them: cap +30
- Attended their birthday: cap +20
- Weathered a storm together: cap +20
- Hosted a festival they attended: cap +30
- Helped them through a personal moment (lore beats): cap +50

The cap can never go down.

Combined friendship score is `trust + affection + respect`, displayed visually as a single heart-fill bar. The bar tops out at the current cap. Mechanically, cap-relative percentage is what matters.

**Decay:**

- Decay only affects Affection (the daily-fluctuating bar).
- Decay rate: ~2% per in-game day if not interacted with.
- Decay caps at 50% (Affection won't drop below half if cap allows).
- Trust and Respect never decay.

**Restoration:**

- Single visit + treat fully restores Affection.
- Group event boosts multiple cats' Affection simultaneously.

**No negative friendship.** A cat at 0 friendship is "Acquaintance." There is no "enemy" state. Mistakes (giving a hated treat, asking a Lazy cat to do hard labor) result in *no friendship gain* or a small Affection hit, never long-term damage.

### 7.5 Treats and gifts

Treats are crafted at kitchens and given to cats. They affect Affection most strongly:

- **Tier 1 — Foraged**: berries, dried fish, mushrooms, simple herbs. Universal +5 Affection.
- **Tier 2 — Cooked**: stews, baked goods, fish pies, herb teas. +10 Affection.
- **Tier 3 — Personalized**: a specific cat's favorite recipe (discovered through gifts and observation). +20 Affection, plus mood boost.
- **Tier 4 — Legendary**: rare ingredients from deep exploration, used for milestone moments. +30 Affection plus cap raise.

Treats spoil over in-game days, preventing stockpile abuse. Refrigeration (built kitchen feature) extends shelf life.

**Discovering favorites:**

- First few treats given: random reactions hint at preference.
- After ~5 gifts, the cat's preference becomes visible in their relationship panel.
- Some traits make discovery easier (Gluttonous cats announce loves loudly, Shy cats hint subtly).

### 7.6 Roles and assignments

A cat must reach **Trust ≥ 30** before role assignment is unlocked.

| Role | Workplace | Effects |
|---|---|---|
| Builder | Workshop | Constructs queued building pieces |
| Forager | Storehouse | Daily material gathering, biome-appropriate |
| Cook | Kitchen / Café | Produces treats, attracts visitors |
| Fisher | Dock | Daily fish, unlocks seafood recipes |
| Farmer | Farmhouse + plots | Grows ingredients (long-term) |
| Innkeeper | Inn | Brings traveler cats to town |
| Bard | Music Hall | Town-wide mood boost, attracts artistic cats |
| Mayor | Town Hall | Unlocks town projects, festivals, diplomacy |
| Merchant | Shop | Buys/sells, late-game trade unlocks |
| Scholar | Library | Research blueprints, lore unlocks |
| Coachman | Cart Station | Drives carts on routes, runs trade |
| Architect | Drafting Office (advanced) | Modifies blueprints with variations |

Assignments are made via the cat's relationship panel. A cat can hold one role at a time. Switching roles is free but takes a day to settle.

### 7.7 Work efficiency

Efficiency formula:

```
efficiency = base_role_efficiency
           * (current_friendship / cap_friendship)
           * trait_modifier
           * mood_modifier
```

Where:

- `base_role_efficiency`: 1.0 (fits the role naturally), 0.7 (mismatch), 1.3 (perfect fit)
- `trait_modifier`: e.g., Industrious +15%, Lazy -20%
- `mood_modifier`: 1.0 baseline, ±20% based on Mood

Efficiency affects:

- Job completion speed (Builder)
- Materials gathered per day (Forager)
- Treats produced per day (Cook)
- Cart route speed (Coachman)
- Etc.

Minimum efficiency floor: **40%**. A cat is never useless. The mechanic rewards good relationship management without punishing neglect.

### 7.8 Daily routines

Every cat (employed or not) has a daily routine. Default structure:

- **Morning (6-10 in-game)**: wake, eat at home, leave for work or first activity
- **Mid-day (10-14)**: primary activity (work, foraging, or personality-driven activity)
- **Afternoon (14-18)**: secondary activity, social drift toward gathering spots
- **Evening (18-22)**: socializing, eating with friends, festival time
- **Night (22-6)**: return home, idle, sleep

Routines are scheduled task lists. The AI selects activities based on:

- Role obligations (employed cats prioritize work mid-day)
- Personality (Adventurous cats wander further, Lazy cats nap longer)
- Current Needs (hungry cats eat, lonely cats seek others)
- Special events (festivals override normal routines)

Routines visible to player in the cat's profile panel — you can see what your friends are doing right now.

### 7.9 Autonomy via utility AI

Use **big-brain** crate for utility AI. Each potential activity has a scorer that returns a 0-1 priority. The cat picks the highest-scoring action each tick.

Example scorers:

- `NapInSunbeamScorer`: returns 0.3 base, scales to 0.9 if Energy is low and a sunbeam is nearby.
- `VisitFriendScorer`: returns `friendship_with(target) * time_since_last_visit_normalized`.
- `WorkScorer`: returns 0.8 if assigned role and work is queued and time-of-day matches, else 0.0.
- `EatScorer`: scales with Hunger.
- `WanderScorer`: returns 0.2 baseline (always low priority filler).

Cats choose actions emergently from these scores. No scripted behavior required for default activities.

### 7.10 Emergent gathering spots

Cats prefer benches, fountains, fire pits, sunbeams, gardens, and other "gathering objects." Place a fountain in the town square — cats naturally drift there at midday. Place a bench by the lake — a Lazy cat may claim it as their daily nap spot.

**Architecture shapes behavior.** This is a hallmark feature. Players should notice that *where they place things matters socially*.

### 7.11 New cat arrivals

Cats arrive in town through several channels:

- **Wandering strays** — periodically appear at the town edge if the town has reputation > threshold. Player can approach and befriend.
- **Inn arrivals** — if the town has an Inn with an Innkeeper, traveler cats periodically stop. Some can be invited to stay.
- **Family arrivals** — high-friendship residents may invite siblings, parents, or kittens to visit; visits can become moves with player approval.
- **Festival visitors** — joint festivals draw cats from other towns; some may relocate.
- **Discovered settlements** — exploring NPC cat villages can lead to migration.

Each arrival is a player choice. The arrival panel shows the cat's portrait, personality preview, desired role (if any), and a "welcome / let pass" option.

### 7.12 Cat departures

Cats leave only for *positive* reasons:

- Joining a romantic partner in another town (lore moment, never sad)
- Founding their own settlement (new town opportunity for player)
- Returning to a homeland on a celebratory journey

Mechanically, departures are rare and always come with a player-meaningful event (a goodbye party, a letter, a celebration).

### 7.13 Cat life cycle

Cats live long, peaceful in-game lives. Over the course of many in-game years (well into Phase 4):

- Kittens are born to high-friendship pairs (player can encourage by building nursery).
- Kittens grow into young cats over in-game seasons.
- Elder cats slowly retire from active roles, becoming wise gentle presences.
- Eventually, off-screen and peacefully, elders pass on. The town remembers them — their houses become heirloom buildings, their portraits hang in the town hall, festivals are named after them.

Death is never traumatic, never on-screen, never a fail state. It's a gentle thread of mortality that gives the world *historical depth*.

---

## 8. Resources & Crafting

### 8.1 Resource categories

- **Wood**: oak logs, pine logs, birch logs, driftwood. Crafted into planks, beams, furniture pieces.
- **Stone**: rough stone, granite, sandstone, slate. Crafted into bricks, tiles, decorative stone.
- **Metals (rare, late)**: copper, iron, brass. For nails, hinges, decorative fittings.
- **Fibers**: wool, reeds, plant fiber. For rugs, baskets, textiles.
- **Food ingredients**: berries, mushrooms, fish, herbs, grains, honey, salt.
- **Special**: gemstones, shells, feathers, pearls, crystals. For decorative crafts and gifts.

### 8.2 Gathering

All gathering is direct-control, in-world action. Materials go to town pool automatically.

- **Trees**: walk up, press interact to swat. Cat animation, leaves/twigs/acorns drop. Multi-swat for branches, full chop for logs. Trees regrow over in-game days.
- **Bushes/flowers**: walk through or paw at. Berries, leaves, petals.
- **Mushrooms/rocks/sticks**: scattered in world, walk over to collect.
- **Stone deposits**: small rock formations, scratch interaction yields pebbles. Larger deposits unlock with tools/Forager role.
- **Fish**: water tiles with ripples. Edge interaction, paw swipe, sometimes catch.
- **Dig spots**: random patches, daily refresh. Yields treasures, rare items, lore artifacts.
- **Gifts from cats**: high-friendship cats leave gifts at the player's doorstep.

### 8.3 Crafting workbenches

Crafting happens at physical workbenches in the world:

- **Carpentry bench** — wood pieces, basic furniture
- **Stonemason** — stone pieces, masonry
- **Kitchen** — treats, cooked food
- **Sewing table** — textiles, soft furnishings, rugs
- **Forge (late)** — metals, fittings, decorative iron
- **Drafting office** — blueprints, building plans, modifications

Approach a workbench, press interact, the crafting menu opens. The menu *is* the interface; the workbench is the *place*. Different workbenches expose different recipes.

### 8.4 Crafting flow

1. Open workbench menu.
2. Select recipe from available list (filtered by player's discovered recipes).
3. Confirm — cat plays crafting animation for a few seconds.
4. Item appears in town pool (or mouth slot if intentionally crafted).

Recipes are *discovered*, not given:

- Some recipes are starting knowledge (planks, basic stone block).
- Some are taught by friend cats at sufficient friendship.
- Some are found in lore artifacts (recipe books, scrolls in dig spots, old buildings).
- Some unlock via role advancement (Cook learns new dishes over time).
- Some unlock via festivals or trade with NPC towns.

### 8.5 Construction materials

Building pieces are crafted from raw materials. Example chain:

- Oak Log (gathered) → Oak Plank (carpentry bench, 2 planks per log) → Oak Wall (workshop, 4 planks per wall) → placed via build system.

The player typically doesn't manually craft each piece. With Forager and Builder cats assigned, the chain runs automatically — Foragers gather, building piece queues consume materials and trigger crafting, Builders construct.

### 8.6 The "no inventory hunting" rule

The player never has to walk to storage to find materials. The town pool is bottomless and ambient — materials are *available* wherever the player is within a town's boundaries. UI surfaces what's available when relevant (placing a wall checks pool; if insufficient, shows missing materials needed).

If pool is empty, placement queues anyway as a `BuildJob` waiting on materials. Foragers will eventually fulfill and the job will proceed. Player isn't blocked.

---

## 9. Towns

### 9.1 Town as entity

A Town is a first-class entity:

```rust
#[derive(Component)]
struct Town {
    id: TownId,
    name: String,
    population: Vec<Entity>,           // cats living here
    buildings: Vec<Entity>,            // buildings in this town
    blueprint_library: Vec<Handle<Blueprint>>,
    storage: HashMap<MaterialId, u32>, // town material pool
    cozy_score: f32,                   // aggregate from buildings
    reputation: f32,                   // attracts new cats
    specialty: Option<TownSpecialty>,  // unique production
    boundary: TownBoundary,            // spatial extent
    biome: BiomeKind,                  // primary biome
}
```

### 9.2 Founding a town

Phase 1: player's starting location is the foundation of Town 1, automatic.

Phase 4: founding new towns:

- Travel to a sufficiently distant location (in a different biome ideally).
- Place a "Town Founding Stone" buildable (requires materials, unlocks at certain milestones).
- Confirm — new Town entity created, boundary defined, player can now build there.
- The new town starts empty; player must attract cats (via Inn or wandering) to populate.

A player can found 4–6 towns over a long playthrough. Each in a distinct biome amplifies the trade and cultural systems.

### 9.3 Town boundaries

Each town has a spatial extent — the area considered "this town's territory." Determined automatically by placed buildings (convex hull plus buffer) or manually editable.

Boundaries affect:

- Which cats live in which town (residents are bound to their town entity).
- Material pool access (player accesses Town A's pool while inside Town A's boundary).
- Reputation effects (cozy score and decoration in Town A boost Town A's reputation).

### 9.4 Town reputation

Reputation is a 0-100 score derived from:

- Aggregate Cozy Score across buildings
- Number of happy residents
- Active cultural events (festivals)
- Trade activity
- Decoration coverage

Reputation effects:

- Higher reputation → more wandering strays appear at town edge.
- High reputation → higher-tier cats (rare personalities, special skills) consider moving in.
- Low reputation → no negative consequences, just slower growth.

### 9.5 Town specialty

After a town has stable population (say, 8+ cats) and a Mayor, the player can declare a specialty:

- Determined by biome (some specialties only available in specific biomes)
- Influenced by population (artistic-heavy population enables artisan specialty)
- Unlocks unique craftable goods
- Affects trade (specialty goods are highly valued by other towns)

Examples:

- **Forest Artisan Town**: produces unique wooden furniture not craftable elsewhere
- **Mountain Stoneworking Town**: produces unique stone decor and architectural pieces
- **Coastal Fishing Town**: produces unique seafood treats and pearl-inlaid items
- **Meadow Bakery Town**: produces unique baked goods and honey products
- **Wetland Tea House**: produces unique herbal teas with strong friendship effects

Specialties are *cultural identity* as much as gameplay. Each town becomes its own thing.

---

## 10. Cart System & Inter-Town Travel

### 10.1 Cart paths

Cart paths are buildable infrastructure connecting towns:

- Selected from build hotbar as "Cart Path" tool.
- Player clicks waypoints on terrain; system auto-routes a smooth curve between them.
- Each path segment auto-flattens a narrow strip of terrain.
- Costs materials per segment (gravel/cobble/etc.).
- Builder cats construct segments over time.

### 10.2 Path tiers

| Tier | Material | Cart Speed | Cost | Aesthetic |
|---|---|---|---|---|
| Worn track | None (auto-formed) | 0.7× | Free | Dirt path that emerges from cat traffic |
| Dirt path | Gravel | 1.0× | Low | Rustic country path |
| Cobblestone | Stone bricks | 1.5× | High | Civilized, durable |
| Boardwalk | Planks | 1.2× | Medium | For wet terrain, lakeside |
| Bridge | Stone + wood | 1.0× | Very high | Crosses water/ravines |

Worn tracks emerge automatically where cats walk frequently — a nice touch implementing your "path memory" idea.

### 10.3 Path validation during placement

Visual feedback during sketching:

- **Green segment** — valid placement.
- **Yellow segment** — possible but suboptimal (steep slope, slow segment).
- **Red segment** — invalid (too steep, water without bridge, blocked).

The path-sketcher proposes auto-corrections (route around obstacles) when red segments appear.

### 10.4 Carts as vehicles

A Cart is an entity:

```rust
#[derive(Component)]
struct Cart {
    style: CartStyle,        // basic / covered / luxury / cargo
    base_speed: f32,
    capacity: u32,
    home_station: Entity,    // which cart station owns it
}

#[derive(Component)]
struct FollowingPath {
    path: Entity,            // CartPath entity
    progress: f32,           // 0.0 to 1.0 along path
    direction: PathDirection,
    cargo: Vec<(MaterialId, u32)>,
    on_arrival: ArrivalAction,
}
```

Cart speed at runtime:

```
speed = cart.base_speed
      * path_quality_multiplier(at current progress)
      * coachman_efficiency
      * slope_penalty(at current progress)
```

### 10.5 Cart Stations

Each town has a Cart Station building (player-built). Acts as:

- Cart spawn / parking location
- Player-facing travel UI (list connected destinations)
- Trade route configuration UI
- Coachman cat workplace

### 10.6 Player travel

When the player approaches a Cart Station and wants to travel:

1. Cart Station UI shows connected destinations.
2. For each, options: **Ride along** (real-time scenic) or **Fast travel** (instant fade).
3. First-time journeys must be Ride along (discovers the route).
4. Subsequent journeys allow Fast travel.

During Ride along:

- Camera attaches to cart (passenger seat).
- Player can rotate camera to look around but not control cart.
- Travel takes 30s–3min depending on distance and path quality.
- Ambient events occasionally happen (butterflies, distant cats waving, weather, dig spots visible from path).
- Player can press a button to stop the cart at any point and disembark.

### 10.7 Trade routes

Trade is the late-game economic system:

1. At a Cart Station, open Trade Routes panel.
2. Configure: From Town → To Town, what to send, what to receive, frequency.
3. Confirm — Coachman cats run carts on schedule.

Trade carts:

- Run autonomously between towns.
- Player sees them on paths, can wave, can pass them on the road.
- Carry actual material entities; goods deplete from sender pool, accumulate in receiver pool.
- Coachman efficiency affects trip time and reliability.

Trade balance:

- Fair trades boost both towns' morale.
- Unfair trades create resentment in the disadvantaged town (slowly).
- Player can adjust trades anytime; not locked in.

### 10.8 NPC town trade

Discovered procedural cat settlements offer trade:

- Approach the village, speak to their Mayor or representative.
- Negotiate trade routes (NPC towns drive harder bargains than your own towns).
- Unique goods may be available only through NPC trade (rare crystals, ancient recipes, mystical herbs).
- Long-term trade with NPC towns can lead to diplomatic events (cultural exchanges, joint festivals, offered immigrants).

---

## 11. Festivals & Events

### 11.1 Festival types

- **Seasonal festivals** (auto-triggered):
  - Spring Bloom — flower arrangements, growth boosts
  - Summer Sunfest — sunbathing competitions, beach gathering
  - Autumn Harvest — food displays, communal cooking
  - Winter Hearth — fireplace gatherings, gift exchanges

- **Town-hosted festivals** (player-organized):
  - Music Night — Bards perform, mood boost
  - Tea Party — small intimate, friendship boost
  - Market Day — trading event, materials surplus boost
  - Building Showcase — players' architectural achievements celebrated, reputation boost
  - Joint Festival — invite cats from other towns, civilization-tier event

### 11.2 Hosting a festival

1. At Town Hall, open Festival Planner.
2. Choose festival type.
3. Set date (in-game day).
4. Allocate budget (treats, decorations, music).
5. Send invitations (other towns, specific cats, public).
6. Day arrives — cats gather at festival location, eat, dance, mingle.
7. After festival, results screen: friendship boosts, reputation gain, special outcomes (love stories, immigrations).

### 11.3 Festival as civilization moment

Joint festivals are the *crescendo* of late-game play:

- Carts ferry cats from multiple towns.
- The host town fills with visitors.
- Cross-town friendships form.
- Cultural exchange happens (recipes shared, stories told).
- Sometimes results in immigrations or new trade routes.

A successful joint festival is the "trailer moment" of the game — multiple towns' worth of cats crowding a square, music, decorations, friendship hearts everywhere. Should be designed for *spectacle*.

---

## 12. Worldbuilding & Lore

### 12.1 The post-human framing

Cat World takes place after humanity is gone — peacefully, gently. Not extinction trauma; just absence. The world is healing. Nature has reclaimed roads. Buildings stand empty. And cats — once humanity's companions — have inherited a beautiful, gentle world.

Cats are slowly evolving as a species. They walk on hind legs (selectively — still cat-shaped, just bipedal-capable). They use tools, build, cook, develop language and culture. The game is their civilization story.

### 12.2 Human ruins

Procedurally placed throughout the world:

- Cracked highway sections poking through grass
- Rusted-out vehicle hulks
- Derelict houses (wood rotted, ivy-grown)
- Abandoned shops with weathered signs
- Strange artifacts (a child's toy, a coffee mug, a book)

Cats don't fully understand these. They're objects of curiosity, mystery, and sometimes utility (ruined houses can be rebuilt by industrious cats).

Scholar cats can research artifacts over time, slowly piecing together "what came before." Lore is gentle and bittersweet, never tragic — humans are remembered as the kind giants who fed cats.

### 12.3 Cultural emergence

As towns develop, they accrue cultural identity:

- **Dominant building style** based on player's blueprint use
- **Cultural festivals** based on which seasonal events the town has hosted
- **Local legends** auto-generated from notable resident events (the cat who founded the town, the kitten born during the storm, the artist who painted the town hall)
- **Local cuisine** based on what Cooks have specialized in
- **Town anthem** unlocked at high reputation, composed based on town history

A "Town Heritage" panel shows accumulated cultural artifacts. Late game, every town feels distinct *as a culture*.

### 12.4 Scientific & cultural milestones

Civilization-tier unlocks gated by population, role specialization, and time:

- **First Library** → writing systems, recipe books, lore preservation
- **First Music Hall** → instrumental music, dance traditions
- **First Festival Hosted** → cultural calendar
- **First Joint Festival** → diplomatic relations
- **First Trade Route** → economic networks
- **First Inter-Town Marriage** → cultural exchange, blended families

Each unlocks new mechanics and content. Civilization as content gating, not gates.

---

## 13. Technical Architecture

### 13.1 Engine

- **Bevy 0.18+** (latest stable as of 2026).
- **Rust stable** (MSRV per Bevy requirements).
- Targets: Windows, Linux (incl. Steam Deck), macOS. Console ports later.

### 13.2 Core ecosystem dependencies

- `bevy_rapier3d` — physics
- `bevy_tnua` — character controller
- `oxidized_navigation` — navmesh pathfinding
- `big-brain` — utility AI for NPC cats
- `leafwing-input-manager` — input action abstraction
- `bevy_egui` — UI panels and menus
- `bevy-inspector-egui` — dev-time entity inspection
- `bevy_kira_audio` or `bevy_seedling` — audio (spatial sounds)
- `bevy_atmosphere` — sky and day/night cycle
- `bevy_asset_loader` — asset loading states
- `moonshine-save` (or custom reflection-based) — save/load
- `noise` — procedural noise (already integrated in PCG)
- `serde` + `ron` — serialization for blueprints, save files

### 13.3 ECS architecture

**Single persistent World.** All entities — terrain chunks, cats, buildings, pieces, towns, paths, carts — live in one Bevy World. No streaming, no multi-world.

**Marker components for fast querying:**

```rust
// Object types
#[derive(Component)] struct BuildingPiece;
#[derive(Component)] struct InteriorPiece;
#[derive(Component)] struct ExteriorPiece;
#[derive(Component)] struct StructuralPiece;
#[derive(Component)] struct DecorationPiece;
#[derive(Component)] struct InteractionPoint;
#[derive(Component)] struct TerrainChunk;

// Behavior modifiers
#[derive(Component)] struct SleepWhenFar;       // performance opt-in
#[derive(Component)] struct InstancedRender(BatchKey);
#[derive(Component)] struct DistanceLOD(LODLevel);
```

**Hierarchical entities** for buildings: a Building parent contains BuildingPiece children. Despawn cascades.

### 13.4 Performance strategy

**GPU instancing.** Pieces sharing mesh + material are batched into instance groups. Render in single draw calls. Critical for handling thousands of pieces.

**Distance LOD on systems, not assets.** Low-poly meshes need no geometric LOD. Instead, system work is distance-gated:

- Within 50m of player: full simulation, animations, particles.
- 50–200m: reduced tick rate, simpler animations.
- Beyond 200m: minimal updates (waypoint teleport, AI tick every N seconds).

**Interior culling.** Buildings outside player's view have interior children set to `Visibility::Hidden`. When camera is inside a building, *other* buildings' interiors hide. Massive perf win.

**Marker-driven systems.** Most building piece entities have *zero* per-frame logic. They're just transforms + meshes. Systems use `Changed<T>` filters and run conditions to avoid iterating dormant entities.

**Chunked terrain mesh regen.** Terrain edits flag chunks dirty; a system regenerates dirty chunks (max N per frame). Editing is responsive without hitches.

### 13.5 Save system

- Single save file per world (RON or compressed bincode).
- Reflection-based serialization of all relevant components.
- Saves on game quit, periodic auto-save (every in-game hour), manual save anytime.
- Blueprints saved as separate `.bp.ron` files in a blueprints folder.
- World seed stored in save; PCG regenerates terrain identically on load.

Save file size estimate: 10–50MB for mature world. Acceptable.

### 13.6 Asset pipeline

- All meshes authored in Blender, exported as glTF (`.glb`).
- Low-poly art style: ~50–500 tris per piece.
- Material variants via shader (single mesh, swappable albedo + properties).
- Hot-reload enabled in dev for fast iteration.
- Texture atlases for material variants to reduce draw call count.

### 13.7 Build system architecture

- **Blueprint as Asset**: each blueprint is a `.bp.ron` file loaded as a Bevy Asset. Hot-reloadable.
- **BuildJob queue**: resource holding pending construction jobs.
- **Builder AI**: utility scorer that pulls jobs from queue when cat has Builder role and is on duty.
- **Construction system**: ticks active jobs, plays animations, instantiates piece entities at completion.

### 13.8 Determinism

- World generation deterministic from seed.
- Critical: lock the noise function and PCG algorithm before launch. Version it in saves.
- Most other systems do not need to be deterministic (player input + AI driven, save/load handles state).

### 13.9 Modding considerations

- RON-format blueprints are human-readable. Players can hand-edit and share.
- Asset folders structured for mod replacement (textures, meshes can be swapped).
- Future: official mod API for adding new pieces, traits, biomes, recipes.

---

## 14. Platform & Launch

### 14.1 Primary target

PC via Steam, with first-class Steam Deck support.

### 14.2 Secondary targets (post-launch)

- Switch 2 / Nintendo eShop
- PlayStation 5
- Xbox

Prioritize Switch 2 for cozy market reach.

### 14.3 Pricing strategy

- Early Access launch: $19.99–$24.99 USD
- Full release: same or +$5
- No microtransactions, no DLC chasing. Possible expansions for new biomes / civilizations later.

### 14.4 Launch scope

Early Access Phase 1 launch contains:

- Single-town gameplay loop fully functional
- 1–2 biomes (Forest + Meadow recommended)
- All core systems: building, friendship, roles, gathering, crafting
- Festival system with 2–3 festival types
- ~30–50 building pieces, ~15–20 furniture/decoration items
- 8–10 NPC cat archetypes (personality + visual variants)
- Save/load, UI, controller support, polish

Phase 2 / 3 / 4 features (multi-town, carts, trade, joint festivals) ship as free post-launch updates over 12–18 months.

---

## 15. Development Roadmap

### 15.1 Vertical slice (8 weeks)

Goal: prove all five core pillars work together in one playable scene.

- **Weeks 1–2**: Build feel — modular kit MVP, snapping, place-and-construct flow.
- **Weeks 3–4**: Interior depth — wall finishes, floor materials, basic furniture, lighting, decoration. Build one cottage that feels good.
- **Weeks 5–6**: One cat NPC — friendship, treats, daily routine in the cottage. Visit, gift, watch them live.
- **Weeks 7–8**: Floor plan tool + Builder cat — sketch a plan, assigned cat constructs over time. Proves the architecture-at-distance loop.

Decision gate: if all four feel right, continue. If any feels hollow, fix before scope expansion.

### 15.2 Pre-alpha (months 3–6)

- Terrain editing system (vertex-height grid, brushes, auto-flatten)
- Multiple NPC cats with utility AI and personality system
- Role assignments and work efficiency
- Crafting workbenches and recipe discovery
- Town entity, town pool, reputation
- Save/load fully functional
- Festivals MVP (one type, hostable)

### 15.3 Alpha (months 6–10)

- Full personality trait system (10–12 traits)
- All starting roles implemented
- Multiple biomes (at least 2 fully featured)
- Full building piece kit (~40 pieces)
- Furniture and decoration depth (~20 items)
- New cat arrival system
- Polish on building feel, animations, particle effects
- Sound design pass

### 15.4 Beta / Early Access (months 10–14)

- Performance optimization (instancing, LOD systems)
- UI polish, controller support polish
- Tutorial and onboarding
- Save corruption recovery
- Settings menu, accessibility options
- Bug fixing, playtesting iteration
- Marketing, demo, Steam Next Fest participation

### 15.5 Post-launch (months 14–30)

- **Update 1**: Second town founding, basic cart paths
- **Update 2**: Trade routes, biome specialties
- **Update 3**: Joint festivals, civilization milestones
- **Update 4**: NPC settlements, diplomacy
- **Update 5**: New biomes, kittens, life cycle
- **Update 6**: Console ports

---

## 16. Open Questions

Areas requiring further design exploration:

1. **Combat or none?** The game intentionally has no combat. But what about *peaceful confrontation* — territorial wild cats, mischievous raccoons stealing from gardens, weather events as challenges? Currently leaning fully peaceful, but worth considering soft-conflict flavor.

2. **Multiplayer scope.** Visiting a friend's world (read-only? co-op build?). Worth designing for early but probably ships post-launch.

3. **Day-night cycle pacing.** How long is an in-game day? 20 real minutes? An hour? Affects routine pacing and player expectations.

4. **Kitten gameplay.** Are kittens NPCs or could the player adopt one and influence their growth? Could be a beautiful late-game thread.

5. **Economy rebalancing.** Will need extensive playtesting to ensure the gather → craft → build → friendship loop is satisfyingly paced without being grindy.

6. **Architect role specifics.** How exactly do Architect cats modify blueprints? Limit to material swaps, or allow real geometry changes?

7. **Save file portability.** Can players export their towns to share? Worth supporting from day one.

8. **Weather effects on gameplay.** Rain, snow, fog — purely cosmetic or do they affect gathering, mood, festivals?

---

## Appendix A: Glossary

- **Blueprint** — saved building design, reusable as a placeable.
- **Cap (relationship)** — ceiling on combined friendship score, raised by milestones.
- **Cozy Score** — building-level metric of decoration, lighting, and functional completeness.
- **Floor plan** — building outline sketched on terrain, can be rough or detailed.
- **Mouth slot** — single visible item the cat physically carries.
- **Town pool** — shared bottomless storage accessible everywhere within a town.
- **Utility AI** — score-based decision system where each potential action returns priority and cat picks highest.
- **Worn track** — auto-formed dirt path from cat foot traffic.