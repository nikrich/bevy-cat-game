# Phase 7 — Weather, Audio, Polish, EA Launch

> Status: Planned
> Depends on: Phase 6
> Exit criteria: Game ships at Early Access scope. Weather (rain, snow, wind) animates and affects ambience. Per-biome audio loops + SFX library. Tutorial / onboarding flow. Main menu, settings, accessibility. Save corruption recovery + Steam Cloud sync. Performance optimized: instancing, distance LOD, interior culling. Controller polish pass. Steam page assets ready. EA build passes a first-time-user playtest.

## Goal

Take the playable single-town slice from Phase 6 and turn it into something that can launch on Steam. This phase is breadth, not depth: weather, sound, menus, accessibility, performance, and the dozens of small polish items that turn a vertical slice into a product.

## Why now

This is the last phase before EA. Skipping any item here turns into post-launch tech debt or a worse first impression. Every item is small individually; the total effort is real because there are many.

## Deliverables

- Weather system: Clear, Rain, Snow, Wind (with intensity)
- Rain particles + ground wetness shader
- Snow particles + ground accumulation
- Wind affecting prop sway intensity (hooks into existing PropSway)
- `bevy_kira_audio` (or `bevy_seedling`) integration
- Per-biome ambient soundscapes (Forest, Meadow, Coastal, Mountain, Wetland, day vs. night = 12 loops)
- SFX library: build, gather, craft, cat meows, footsteps, water splash, treat eat, festival
- 4 music tracks: calm exploration, festival, night, town theme
- Tutorial / onboarding flow (first 15 minutes guided)
- Main menu (egui): New Game / Load Game / Settings / Quit
- Settings: audio volumes, control rebinding, accessibility toggles (text size, color-blind, motion reduction)
- Pause menu with Save, Settings, Quit to Menu
- Save corruption recovery (versioned saves, last-good fallback, rolling 3 auto-saves)
- Steam Cloud sync verified via Steamworks SDK (or path-based file sync if Steamworks integration deferred)
- GPU instancing for repeated pieces
- Distance LOD on system tick rates (per spec §13.4)
- Interior culling pass
- Controller polish: radial menu, hotbar cycle, ghost piece rotation feel
- Steam store page: capsule images, screenshots, trailer cuts
- First-time-user playtest pass with 3+ external testers

## Decisions to record

- DEC-044 — Weather is a `Resource WeatherState { kind, intensity, transition_progress }`. Transitions are linear interpolations. Spec §16 weather effects: cosmetic + sway only, not gathering or mood (kept simple for EA).
- DEC-045 — Audio uses `bevy_kira_audio`. Spatial audio for SFX, 2D for music and ambient loops.
- DEC-046 — Save versioning: header field `save_version: u32`. On load, run migrations top-down. Rolling 3 auto-saves rotate.
- DEC-047 — Tutorial is opt-in: first run defaults to Yes, with a "Skip Tutorial" toggle on the New Game dialog.

## Tech debt to close

- DEBT-006 — no test coverage. Phase 7 ships at least 20 integration tests covering: save round-trip, building snap, friendship gift, festival execution, terrain edit + persist, weather transition, audio loop swap.
- DEBT-007/008 already closed in Phase 1. Phase 7 *verifies* the closure with a perf pass.

## Work breakdown

### W7.1 — `WeatherState` resource + transitions

**What:** `WeatherState { kind: Clear | Rain | Snow | Wind, intensity: f32, transition_progress: f32 }`. Transitions over 30 in-game minutes. Weather schedule: random walk biased by biome (rain more likely in Forest/Wetland, snow only in Tundra/Snow biomes).
**Acceptance:** Watching the sky over an hour shows visible weather changes. Forest sees rain, Tundra sees snow, Desert stays largely clear.

### W7.2 — Rain particles + ground wetness

**What:** Particle effect across the camera frustum during Rain. Ground material gains a "wetness" parameter that darkens albedo, raises specular. Intensity scales rain density and wetness.
**Acceptance:** Rain feels like rain (visible streaks, sound, ground darkens). Sheltered areas (under roof) get less wet visually.

### W7.3 — Snow particles + accumulation

**What:** Snow particles drifting down. Ground gains a "snow accumulation" parameter that whitens albedo. Accumulation rate scales with intensity, melts when temperature rises.
**Acceptance:** A long snowfall in Tundra leaves the ground white. Walking through accumulated snow is visible (cosmetic only, no slow-down).

### W7.4 — Wind affecting prop sway

**What:** Existing PropSway gains a wind multiplier read from `WeatherState`. Higher wind = larger sway amplitude + faster cycle.
**Acceptance:** Trees and grass visibly thrash in high wind, gentle breeze otherwise. Wind sound layer added to ambient mix.

### W7.5 — `bevy_kira_audio` integration

**What:** Add `bevy_kira_audio`. Replace any current Bevy-native audio code (if any) with kira channels: Music, Ambient, SFX, UI.
**Acceptance:** Audio mixes cleanly across channels. Volumes adjustable per channel via settings.

### W7.6 — Per-biome ambient loops

**What:** 12 ambient loops (5 biomes featured at EA × day/night, plus 2 wildcard tracks for festival/storm). Loop crossfades when player crosses biome boundary or day/night transitions.
**Acceptance:** Walking from Forest to Meadow crossfades audio over ~3 s. Day/night crossfade similarly smooth.

### W7.7 — SFX library

**What:** SFX with spatial audio: build piece complete, gather (per material category), craft complete, cat meows (varied per personality), footsteps (per surface), water splash, treat eat, festival cheer, weather thunder.
**Acceptance:** Walking on grass vs. stone vs. wood plays distinct footstep sounds. Building a wall plays construction SFX through the build animation, finishing with a hammer-strike.

### W7.8 — Music tracks

**What:** 4 tracks. Crossfade triggered by context (festival → festival track; nighttime in town → town night theme; otherwise calm exploration). Player can disable music in settings.
**Acceptance:** Track changes feel motivated, not jarring. No track loops obnoxiously within a 10-minute session.

### W7.9 — Tutorial / onboarding

**What:** Scripted first 15 minutes: introduce movement → first gathering → first crafting (carpentry bench) → first build (a wall, a floor) → first decoration → first stray cat arrival. Hints triggered on context, not modal popups blocking play. Skippable.
**Acceptance:** A new player completes the tutorial in 15–20 minutes, ends with a small shelter built and one stray cat befriended. Tutorial can be skipped from New Game dialog.

### W7.10 — Main menu

**What:** egui main menu with logo, four buttons (New Game / Continue / Settings / Quit), background scene of an idyllic town view.
**Acceptance:** Menu is the first thing the player sees after Loading. Continue is greyed if no save.

### W7.11 — Settings menu

**What:** Tabs: Display (resolution, vsync, render distance), Audio (master, music, SFX, ambient), Controls (rebinding, mouse sensitivity, gamepad sensitivity), Accessibility (text size 100/125/150 %, color-blind palette swap, motion reduction).
**Acceptance:** All settings persist via a `settings.json` next to saves. Re-launching the game preserves choices.

### W7.12 — Pause menu

**What:** Esc opens Pause menu (egui): Resume, Save, Settings, Quit to Menu, Quit to Desktop. Pausing freezes simulation.
**Acceptance:** Pause stops cat AI and weather progression. Resume continues exactly. Save during pause works.

### W7.13 — Save corruption recovery

**What:** Versioned saves with header. Rolling 3 auto-saves: `auto-1.json`, `auto-2.json`, `auto-3.json` rotate every 30s of play. On load, try requested file, fall back to next in rotation if corrupt. Manual saves are independent files.
**Acceptance:** Manually corrupting `auto-1.json` and loading "Continue" recovers from `auto-2.json` with no user-facing error beyond a "Recovered earlier save" toast.

### W7.14 — Steam Cloud sync

**What:** Saves live in the OS data dir. Steam Cloud is configured in the Steamworks app config to sync that directory. If Steamworks SDK integration is in-scope for EA, add `steamworks-rs` and detect Steam launch; otherwise rely on path-based sync with documentation.
**Acceptance:** Saving on Mac and signing in on Steam Deck retrieves the same save. Verify with at least one cross-platform sync test.

### W7.15 — GPU instancing

**What:** Pieces sharing mesh + material batched via Bevy's `MaterialMeshBundle` instancing primitives. Most aggressive on terrain mesh (already single-mesh per chunk after Phase 1) and on repeated decorations (fences, lanterns).
**Acceptance:** Frame time on a town with 50+ pieces drops measurably (record before/after). No visual regressions.

### W7.16 — Distance LOD on systems

**What:** Implement spec §13.4 distance bands: < 50 m full sim, 50–200 m reduced rate, > 200 m minimal updates. Systems opt in via run conditions.
**Acceptance:** With ~100 entities across the world, frame time stays steady. Distant cats still complete routines (visible when player approaches).

### W7.17 — Interior culling

**What:** Buildings the camera is not inside have their interior children set to `Visibility::Hidden`. When inside, sibling buildings' interiors hide. Per spec §13.4.
**Acceptance:** Walking into a fully-furnished house: framerate steady. Walking through a town with 8 furnished buildings: framerate steady.

### W7.18 — Controller polish pass

**What:** Tighten radial menu open/close timings. Ensure ghost piece rotation feels good on shoulder buttons. Hotbar cycle on D-pad respects current category. Test on Steam Deck.
**Acceptance:** A controller-only playthrough of the tutorial completes without keyboard. Steam Deck handheld test passes.

### W7.19 — Test suite expansion

**What:** Add at least 20 integration tests covering critical loops: save round-trip, building snap, friendship gift, festival execution, terrain edit + persist, weather transition, audio loop swap, role assignment, BuildJob queue, treat spoilage.
**Acceptance:** `cargo test` runs all in < 60s. CI runs them on push.
**Closes:** DEBT-006.

### W7.20 — Steam page assets + EA build

**What:** Capsule images (header, library, small), 8–10 screenshots, a 30s trailer cut. Steam product page draft. EA description, FAQ.
**Acceptance:** Page passes Steam review. Trailer reads as cozy + cat-protagonist + building-driven within the first 5 seconds.

### W7.21 — First-time-user playtest pass

**What:** Recruit 3+ external testers. Watch playthroughs (recorded or live). Identify the top 5 friction points. Fix the 3 worst.
**Acceptance:** Three testers each complete the tutorial and reach "first stray welcomed" within an hour without external help.

## Risks / open questions

- **Audio asset acquisition.** Music and SFX are bigger asset asks than meshes. Schedule audio sourcing early in Phase 7 (or earlier if budget allows). Royalty-free libraries acceptable for EA.
- **Tutorial feel vs. cozy identity.** A tutorial that nags violates the cozy promise. Default to in-context hints, never modal blocks.
- **Steamworks SDK integration scope.** If Steamworks integration takes more than 1 day, ship EA with path-based sync only and add API integration in a post-EA patch. Steam Cloud at the file-sync level still works.
- **Performance on Steam Deck.** Profile early. If GPU-bound, consider fallback shaders for the slope-blend material.

## Out of scope

- Multi-town gameplay, carts, trade (post-EA roadmap, spec §15.5)
- Console ports (post-EA)
- Modding API (post-EA)
- New biomes beyond the EA-featured set (post-EA)
- Kittens, life cycle (post-EA)

## Estimated effort

15–25 work-days. Audio sourcing, tutorial, and the playtest fix-it-list dominate. Performance work is unbounded — set a hard time-box.
