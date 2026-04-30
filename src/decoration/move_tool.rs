//! Decoration `Move` tool -- click a placed decoration piece to pick it
//! up; cursor drags it via the same magnetic-continuous physics as Place;
//! click again to drop. No inventory delta -- this is just relocation.

use bevy::prelude::*;

use crate::edit::PlacedItem;
use crate::input::CursorState;
use crate::items::{InteriorCatalog, ItemRegistry, ItemTags};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;

use super::placement::{compute_decoration_placement, DecorationPreview};
use super::{DecorationMode, DecorationTool};

/// Carry state for the Move tool. `entity` holds the entity being dragged
/// while `just_picked_up` blocks drop from firing in the same frame as pickup.
#[derive(Resource, Default)]
pub struct MoveCarry {
    pub entity: Option<Entity>,
    /// Set to `true` by `pickup_decoration` so `drop_decoration` skips the
    /// drop logic in the same frame. Cleared at the top of `drop_decoration`.
    pub just_picked_up: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn pickup_decoration(
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    mut carry: ResMut<MoveCarry>,
    placed_q: Query<&PlacedItem>,
    registry: Res<ItemRegistry>,
) {
    let Some(mode) = decoration_mode else { return };
    if !matches!(mode.tool, DecorationTool::Move) {
        return;
    }
    if carry.entity.is_some() {
        return; // already carrying -- drop happens on next click
    }
    if !cursor.world_click {
        return;
    }
    let Some(hit) = cursor.cursor_hit else { return };
    let Ok(placed) = placed_q.get(hit.entity) else { return };
    let Some(def) = registry.get(placed.item) else { return };

    let is_decor = def.tags.contains(ItemTags::DECORATION)
        || def.tags.contains(ItemTags::FURNITURE);
    if !is_decor {
        return;
    }
    carry.entity = Some(hit.entity);
    carry.just_picked_up = true;
}

#[allow(clippy::too_many_arguments)]
pub fn carry_follow_cursor(
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    carry: Res<MoveCarry>,
    // Read-only for placement computation; Without<DecorationPreview> keeps the ghost out.
    placed_read_q: Query<(&Transform, &PlacedItem), Without<DecorationPreview>>,
    registry: Res<ItemRegistry>,
    terrain: Res<Terrain>,
    noise: Res<WorldNoise>,
    catalog: Res<InteriorCatalog>,
    // Mutable transform for the carried entity; same filter set to satisfy Bevy.
    mut carried_tf_q: Query<&mut Transform, (With<PlacedItem>, Without<DecorationPreview>)>,
) {
    let Some(mode) = decoration_mode else { return };
    if !matches!(mode.tool, DecorationTool::Move) {
        return;
    }
    let Some(entity) = carry.entity else { return };
    let Ok((_, placed)) = placed_read_q.get(entity) else { return };
    let Some(def) = registry.get(placed.item) else { return };

    let pos = compute_decoration_placement(
        cursor.cursor_world.unwrap_or(Vec3::ZERO),
        cursor.cursor_hit,
        def,
        &placed_read_q,
        &registry,
        &terrain,
        &noise,
        &catalog,
    );
    if let Ok(mut tf) = carried_tf_q.get_mut(entity) {
        tf.translation = pos;
        tf.rotation = Quat::from_rotation_y(mode.rotation_radians);
    }
}

pub fn drop_decoration(
    keyboard: Res<ButtonInput<KeyCode>>,
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    mut carry: ResMut<MoveCarry>,
) {
    // Consume the just_picked_up flag so a pickup and drop in the same frame
    // don't cancel each other out.
    if carry.just_picked_up {
        carry.just_picked_up = false;
        return;
    }

    let Some(mode) = decoration_mode else {
        // Mode exited -- drop carry unconditionally.
        carry.entity = None;
        return;
    };
    if !matches!(mode.tool, DecorationTool::Move) {
        // Tool changed -- drop carry.
        carry.entity = None;
        return;
    }
    if carry.entity.is_none() {
        return;
    }
    // Drop on left click OR Escape (cancel = drop in place).
    let drop = cursor.world_click || keyboard.just_pressed(KeyCode::Escape);
    if !drop {
        return;
    }
    carry.entity = None;
    // No inventory delta. Move-undo is a future enhancement (record
    // (entity, before_tf, after_tf) and emit a BuildOp variant).
}
