//! Decoration `Place` tool -- consume left click, spawn the selected
//! piece via magnetic v1 placement, record the op for undo.

use bevy::prelude::*;

use crate::edit::{BuildOp, EditHistory, PieceRef, PlacedItem};
use crate::input::CursorState;
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{InteriorCatalog, ItemRegistry};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;

use super::placement::{compute_decoration_placement, DecorationPreview};
use super::{DecorationMode, DecorationTool};

/// Mirrors `building::INFINITE_RESOURCES`: dev cheat that bypasses
/// inventory consumption while we iterate on placement physics. Flip to
/// false (or wire to a `Cheats` resource) when shipping.
const INFINITE_RESOURCES: bool = true;

#[allow(clippy::too_many_arguments)]
pub fn place_decoration(
    mut commands: Commands,
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    placed_q: Query<(&Transform, &PlacedItem), Without<DecorationPreview>>,
    registry: Res<ItemRegistry>,
    terrain: Res<Terrain>,
    noise: Res<WorldNoise>,
    placeables: Res<crate::building::PlaceableItems>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    catalog: Res<InteriorCatalog>,
    mut inventory: ResMut<Inventory>,
    #[allow(unused_variables)] mut inv_events: MessageWriter<InventoryChanged>,
    mut history: ResMut<EditHistory>,
) {
    let Some(mode) = decoration_mode else { return };
    if !matches!(mode.tool, DecorationTool::Place) {
        return;
    }
    if !cursor.world_click {
        return;
    }
    let Some(item_id) = placeables.0.get(mode.selected).copied() else { return };
    let Some(def) = registry.get(item_id) else { return };

    if !INFINITE_RESOURCES && inventory.count(item_id) == 0 {
        return;
    }

    let pos = compute_decoration_placement(
        cursor.cursor_world.unwrap_or(Vec3::ZERO),
        cursor.cursor_hit,
        def,
        &placed_q,
        &registry,
        &terrain,
        &noise,
    );
    let tf = Transform::from_translation(pos)
        .with_rotation(Quat::from_rotation_y(mode.rotation_radians));

    let Some(entity) = crate::building::spawn_placed_building(
        &mut commands,
        &registry,
        &asset_server,
        &mut meshes,
        &mut materials,
        &catalog,
        item_id,
        tf,
    ) else {
        return;
    };

    if !INFINITE_RESOURCES {
        let entry = inventory.items.entry(item_id).or_insert(0);
        *entry = entry.saturating_sub(1);
        inv_events.write(InventoryChanged {
            item: item_id,
            new_count: inventory.count(item_id),
        });
    }

    history.record(BuildOp::Placed(vec![PieceRef {
        item: item_id,
        transform: tf,
        entity: Some(entity),
    }]));
}
