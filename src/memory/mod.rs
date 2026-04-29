//! Phase A substrate: shared data the rest of the cohesive system reads from
//! and writes to.
//!
//! - [`WorldMemory`] tracks per-cell state (visit count, slept_here, warmth,
//!   marked, etc.) so the world can subtly respond to where the player has
//!   been -- foundation for cat-verbs, warmth-aware ambience, and animals
//!   that learn routines around the player.
//! - [`Journal`] stores observational entries (encounters, sleeps, builds,
//!   echoes) -- the autobiographical "field journal" surfaced later as a
//!   readable book in the UI.
//!
//! Phase A is pure plumbing: resources, save/load, and a tracking system
//! that keeps `WorldMemory` up to date. No visible behaviour change yet.

pub mod journal;
pub mod tile_tint;
pub mod verbs;
pub mod world;

use bevy::prelude::*;

pub use journal::{Journal, JournalEntry, JournalKind};
pub use world::{world_to_cell, CellMemory, WorldMemory};

use crate::player::Player;

pub struct MemoryPlugin;

impl Plugin for MemoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldMemory>()
            .init_resource::<Journal>()
            .add_systems(Update, (track_player_cell, decay_warmth));
        verbs::register(app);
        tile_tint::register(app);
    }
}

/// Per-frame: increment `visit_count` and update `last_visited_secs` on the
/// cell the player is currently standing in. Visit count only ticks once per
/// "fresh entry" (player crosses a tile boundary), not every frame.
fn track_player_cell(
    time: Res<Time>,
    mut memory: ResMut<WorldMemory>,
    mut last_cell: Local<Option<IVec2>>,
    player_q: Query<&Transform, With<Player>>,
) {
    let Ok(tf) = player_q.single() else { return };
    let cell = world_to_cell(tf.translation);
    let elapsed = time.elapsed_secs_f64();

    let entered_new_cell = *last_cell != Some(cell);
    *last_cell = Some(cell);

    let entry = memory.cells.entry(cell).or_default();
    entry.last_visited_secs = elapsed;
    if entered_new_cell {
        entry.visit_count = entry.visit_count.saturating_add(1);
        // Tiny warmth bump on first entry; the casual trail stays mostly
        // dark so the visible "this place is mine" glow is reserved for
        // deliberate verbs (nap +0.25, mark +0.5, examine +0.05).
        entry.warmth = (entry.warmth + 0.005).min(1.0);
    }
}

/// Slow warmth decay so abandoned cells fade. Tuned so a cell last visited a
/// long time ago drifts back toward 0 at roughly 0.005/sec, i.e. a fully-warm
/// cell takes ~3 minutes of game-time absence to fully cool.
fn decay_warmth(mut memory: ResMut<WorldMemory>, time: Res<Time>) {
    let dt = time.delta_secs();
    for cell in memory.cells.values_mut() {
        cell.warmth = (cell.warmth - 0.005 * dt).max(0.0);
    }
}
