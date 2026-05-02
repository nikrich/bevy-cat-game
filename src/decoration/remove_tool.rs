//! Decoration `Remove` tool -- consume a left click on a placed
//! decoration item and despawn it. Refunds inventory and records a
//! BuildOp::Removed history entry so Ctrl+Z / Ctrl+Shift+Z restore it.

use bevy::prelude::*;

use crate::edit::{BuildOp, EditHistory, PieceRef, PlacedItem};
use crate::input::CursorState;
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{ItemRegistry, ItemTags};

use crate::edit::INFINITE_RESOURCES;
use super::{DecorationMode, DecorationTool};

#[allow(clippy::too_many_arguments)]
pub fn remove_decoration(
    mut commands: Commands,
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    placed_q: Query<(Entity, &Transform, &PlacedItem)>,
    registry: Res<ItemRegistry>,
    mut inventory: ResMut<Inventory>,
    #[allow(unused_variables)] mut inv_events: MessageWriter<InventoryChanged>,
    mut history: ResMut<EditHistory>,
) {
    let Some(mode) = decoration_mode else { return };
    if !matches!(mode.tool, DecorationTool::Remove) {
        return;
    }
    if !cursor.world_click {
        return;
    }
    let Some(hit) = cursor.cursor_hit else { return };
    let Ok((entity, tf, placed)) = placed_q.get(hit.entity) else { return };
    let Some(def) = registry.get(placed.item) else { return };

    // Only decoration items are removable through this tool. Walls /
    // floors / doors / windows belong to build mode's Remove tool.
    let is_decor = def.tags.contains(ItemTags::DECORATION)
        || def.tags.contains(ItemTags::FURNITURE);
    if !is_decor {
        return;
    }

    commands.entity(entity).despawn();

    if !INFINITE_RESOURCES {
        inventory.add(placed.item, 1);
        inv_events.write(InventoryChanged {
            item: placed.item,
            new_count: inventory.count(placed.item),
        });
    }

    history.record(BuildOp::Removed(vec![PieceRef {
        item: placed.item,
        transform: *tf,
        entity: None,
    }]));

    if let Some(def) = registry.get(placed.item) {
        info!("[decoration] removed {}", def.display_name);
    }
}
