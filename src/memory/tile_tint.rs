//! Phase C tile-warmth tint -- parked under DEBT-018 by the Phase 1 terrain
//! rewrite (DEC-017 / DEC-018).
//!
//! The original implementation cloned each `Tile` entity's
//! `StandardMaterial` so it could mutate the cell's emissive amber tint
//! while the tile was warm. The new vertex-grid mesh has one shared
//! material per chunk and per-vertex colors, so the per-cell tint needs to
//! be reimplemented either by writing into the chunk mesh's per-vertex
//! emissive attribute, or by spawning a small overlay decal/light per warm
//! cell. Neither is in scope for the Phase 1 foundation slice, so the
//! visual is suppressed for now. `WorldMemory.warmth` itself still ticks
//! (see `memory/mod.rs::track_player_cell` + `decay_warmth`) so verbs and
//! journal entries are unaffected.

use bevy::prelude::*;

pub fn register(_app: &mut App) {
    // No-op: see module docs / DEBT-018.
}
