# Memory Palace

## Art direction
- [2026-04-29] Bipedal cat character: low-poly, warm tan, rounded forms, smooth shading
- [2026-04-29] World aesthetic: earthy tones, stepped terrain, chunky low-poly
- [2026-04-29] Camera: isometric orthographic, ~45 degree angle

## Game vision
- [2026-04-29] Peaceful, lifelong game -- no win/lose state
- [2026-04-29] PCG world crafting -- always new things to discover
- [2026-04-29] Player is a bipedal cat
- [2026-04-29] Core loop: explore, gather, craft, build, discover
- [2026-04-29] Target: desktop and console

## Technical notes
- [2026-04-29] Using Bevy 0.16 (latest) -- fallible systems, new spawn API, entity relationships
- [2026-04-29] Perlin noise terrain with layered octaves for natural look
- [2026-04-29] Height quantized to 0.25 steps for low-poly stepped aesthetic
- [2026-04-29] Isometric camera uses orthographic projection (FixedVertical 20.0)
- [2026-04-29] Movement rotated by PI/4 to align WASD with iso camera

## Preferences
- [2026-04-29] No em dashes in any written content
- [2026-04-29] User provides all assets -- AI builds everything else end-to-end
- [2026-04-29] Conventional Commits for version control

## Architecture
- [2026-04-29] Chunk system: 16x16 tiles, render distance 3, max 4 chunks loaded per frame
- [2026-04-29] Props: noise-based placement (seeds 137, 251), biome-aware, children of chunk entities
- [2026-04-29] Day/night: 12-min full cycle, 6 phases, starts at 8am, moonlight at night

## External resources

## Ideas parking lot
