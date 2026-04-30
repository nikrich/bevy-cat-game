//! Edit-mode undo / redo.
//!
//! Each Place / Remove operation is recorded in `EditHistory.undo`. Ctrl+Z
//! pops from undo, applies the inverse (despawn placed cubes / respawn
//! removed cubes), and pushes to redo. Ctrl+Shift+Z does the reverse.
//! A new operation clears the redo stack -- branching history.
//!
//! Operations also surface as buttons in the build tool egui hotbar so
//! the player can drive undo/redo without keyboard chords.

use bevy::prelude::*;

use super::placed_item::PlacedItem;
use crate::building::{spawn_placed_building, BuildMode};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{InteriorCatalog, ItemId, ItemRegistry};

/// Cap on undo/redo stack depth. Each entry holds a single op (a single
/// click), so 50 covers a comfortable session of fine-grained edits.
const HISTORY_CAP: usize = 50;

pub fn register(app: &mut App) {
    app.init_resource::<EditHistory>()
        .add_systems(Update, undo_redo_hotkeys);
}

#[derive(Resource, Default)]
pub struct EditHistory {
    pub undo: Vec<BuildOp>,
    pub redo: Vec<BuildOp>,
}

impl EditHistory {
    /// Record a new op. Clears the redo stack -- performing a fresh action
    /// invalidates the redo branch.
    pub fn record(&mut self, op: BuildOp) {
        self.redo.clear();
        self.undo.push(op);
        if self.undo.len() > HISTORY_CAP {
            self.undo.remove(0);
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[derive(Clone, Debug)]
pub enum BuildOp {
    /// One or more pieces just placed. `entity` is `Some` while the piece
    /// exists in the world; undo despawns it and clears it to `None`.
    Placed(Vec<PieceRef>),
    /// One or more pieces just removed via the Remove tool. Single
    /// click-removes are a 1-element vec; shift+click line removals can
    /// despawn many at once. `entity` starts `None`; undo respawns each
    /// and records the new entity ids.
    Removed(Vec<PieceRef>),
    /// Atomic swap: `old` was despawned + refunded, `new` was spawned in
    /// its place. Used by `PlacementStyle::Replace` (door/window into wall).
    /// Undo respawns `old` and despawns `new`; redo does the reverse.
    Replaced { old: PieceRef, new: PieceRef },
}

#[derive(Clone, Debug)]
pub struct PieceRef {
    pub item: ItemId,
    pub transform: Transform,
    /// Currently-alive entity for this piece. `None` between operations
    /// where the piece is not in the world (e.g., after undoing a Place).
    pub entity: Option<Entity>,
}

/// Ctrl+Z = undo. Ctrl+Shift+Z (or Ctrl+Y) = redo. Fires while build OR
/// decoration mode is active -- both modes share the same EditHistory
/// stack, so undo / redo work across mode swaps.
#[allow(clippy::too_many_arguments)]
fn undo_redo_hotkeys(
    keyboard: Res<ButtonInput<KeyCode>>,
    build_mode: Option<Res<BuildMode>>,
    decoration_mode: Option<Res<crate::decoration::DecorationMode>>,
    mut history: ResMut<EditHistory>,
    mut commands: Commands,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    placed_q: Query<(Entity, &Transform, &PlacedItem)>,
    catalog: Res<InteriorCatalog>,
) {
    if build_mode.is_none() && decoration_mode.is_none() {
        return;
    }
    let ctrl = keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight);
    if !ctrl {
        return;
    }
    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);

    if keyboard.just_pressed(KeyCode::KeyZ) {
        if shift {
            apply_redo(
                &mut history,
                &mut commands,
                &registry,
                &asset_server,
                &mut meshes,
                &mut materials,
                &mut inventory,
                &mut inv_events,
                &placed_q,
                &catalog,
            );
        } else {
            apply_undo(
                &mut history,
                &mut commands,
                &registry,
                &asset_server,
                &mut meshes,
                &mut materials,
                &mut inventory,
                &mut inv_events,
                &catalog,
            );
        }
    }
    // Windows-style Ctrl+Y also redos.
    if keyboard.just_pressed(KeyCode::KeyY) && !shift {
        apply_redo(
            &mut history,
            &mut commands,
            &registry,
            &asset_server,
            &mut meshes,
            &mut materials,
            &mut inventory,
            &mut inv_events,
            &placed_q,
            &catalog,
        );
    }
}

/// Public -- also called from the egui Undo button.
#[allow(clippy::too_many_arguments)]
pub fn apply_undo(
    history: &mut EditHistory,
    commands: &mut Commands,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
    catalog: &InteriorCatalog,
) {
    let Some(op) = history.undo.pop() else { return };
    let inverse = match op {
        BuildOp::Placed(pieces) => {
            // Undo of placement = despawn each piece + refund inventory.
            let cleared: Vec<PieceRef> = pieces
                .into_iter()
                .map(|p| {
                    if let Some(e) = p.entity {
                        commands.entity(e).despawn();
                    }
                    inventory.add(p.item, 1);
                    inv_events.write(InventoryChanged {
                        item: p.item,
                        new_count: inventory.count(p.item),
                    });
                    PieceRef { entity: None, ..p }
                })
                .collect();
            BuildOp::Placed(cleared)
        }
        BuildOp::Removed(pieces) => {
            // Undo of removal = respawn each piece + decrement inventory.
            let respawned: Vec<PieceRef> = pieces
                .into_iter()
                .map(|p| {
                    let new_entity = spawn_placed_building(
                        commands,
                        registry,
                        asset_server,
                        meshes,
                        materials,
                        catalog,
                        p.item,
                        p.transform,
                    );
                    // Decrement inventory only if it has stock -- undo
                    // shouldn't go negative even with INFINITE_RESOURCES off.
                    if inventory.count(p.item) > 0 {
                        let entry = inventory.items.entry(p.item).or_insert(0);
                        *entry = entry.saturating_sub(1);
                        inv_events.write(InventoryChanged {
                            item: p.item,
                            new_count: inventory.count(p.item),
                        });
                    }
                    PieceRef { entity: new_entity, ..p }
                })
                .collect();
            BuildOp::Removed(respawned)
        }
        BuildOp::Replaced { old, new } => {
            // Undo of swap: despawn `new`, respawn `old`. Inventory mirrors
            // the original swap (refund `new`, consume `old`) so the player
            // ends up exactly where they were before the click.
            if let Some(e) = new.entity {
                commands.entity(e).despawn();
            }
            inventory.add(new.item, 1);
            inv_events.write(InventoryChanged {
                item: new.item,
                new_count: inventory.count(new.item),
            });
            let respawned = spawn_placed_building(
                commands, registry, asset_server, meshes, materials, catalog, old.item,
                old.transform,
            );
            if inventory.count(old.item) > 0 {
                let entry = inventory.items.entry(old.item).or_insert(0);
                *entry = entry.saturating_sub(1);
                inv_events.write(InventoryChanged {
                    item: old.item,
                    new_count: inventory.count(old.item),
                });
            }
            BuildOp::Replaced {
                old: PieceRef { entity: respawned, ..old },
                new: PieceRef { entity: None, ..new },
            }
        }
    };
    history.redo.push(inverse);
}

/// Public -- also called from the egui Redo button.
#[allow(clippy::too_many_arguments)]
pub fn apply_redo(
    history: &mut EditHistory,
    commands: &mut Commands,
    registry: &ItemRegistry,
    asset_server: &AssetServer,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
    placed_q: &Query<(Entity, &Transform, &PlacedItem)>,
    catalog: &InteriorCatalog,
) {
    let Some(op) = history.redo.pop() else { return };
    let forward = match op {
        BuildOp::Placed(pieces) => {
            // Redo of placement = respawn each piece, decrement inventory.
            let respawned: Vec<PieceRef> = pieces
                .into_iter()
                .map(|p| {
                    let new_entity = spawn_placed_building(
                        commands,
                        registry,
                        asset_server,
                        meshes,
                        materials,
                        catalog,
                        p.item,
                        p.transform,
                    );
                    if inventory.count(p.item) > 0 {
                        let entry = inventory.items.entry(p.item).or_insert(0);
                        *entry = entry.saturating_sub(1);
                        inv_events.write(InventoryChanged {
                            item: p.item,
                            new_count: inventory.count(p.item),
                        });
                    }
                    PieceRef {
                        entity: new_entity,
                        ..p
                    }
                })
                .collect();
            BuildOp::Placed(respawned)
        }
        BuildOp::Removed(pieces) => {
            // Redo of removal = despawn each piece (re-find by transform if
            // the stored entity id is stale from intervening edits).
            let cleared: Vec<PieceRef> = pieces
                .into_iter()
                .map(|mut p| {
                    let target = p.entity.or_else(|| {
                        placed_q
                            .iter()
                            .find(|(_, tf, b)| {
                                b.item == p.item
                                    && tf.translation.distance(p.transform.translation) < 0.05
                            })
                            .map(|(e, _, _)| e)
                    });
                    if let Some(e) = target {
                        commands.entity(e).despawn();
                        inventory.add(p.item, 1);
                        inv_events.write(InventoryChanged {
                            item: p.item,
                            new_count: inventory.count(p.item),
                        });
                    }
                    p.entity = None;
                    p
                })
                .collect();
            BuildOp::Removed(cleared)
        }
        BuildOp::Replaced { old, new } => {
            // Redo of swap: despawn `old`, respawn `new`. Mirror of the
            // original `place_replace`. Re-find `old` by transform if its
            // stored entity id is stale.
            let target = old.entity.or_else(|| {
                placed_q
                    .iter()
                    .find(|(_, tf, b)| {
                        b.item == old.item
                            && tf.translation.distance(old.transform.translation) < 0.05
                    })
                    .map(|(e, _, _)| e)
            });
            if let Some(e) = target {
                commands.entity(e).despawn();
            }
            inventory.add(old.item, 1);
            inv_events.write(InventoryChanged {
                item: old.item,
                new_count: inventory.count(old.item),
            });
            let respawned = spawn_placed_building(
                commands, registry, asset_server, meshes, materials, catalog, new.item,
                new.transform,
            );
            if inventory.count(new.item) > 0 {
                let entry = inventory.items.entry(new.item).or_insert(0);
                *entry = entry.saturating_sub(1);
                inv_events.write(InventoryChanged {
                    item: new.item,
                    new_count: inventory.count(new.item),
                });
            }
            BuildOp::Replaced {
                old: PieceRef { entity: None, ..old },
                new: PieceRef { entity: respawned, ..new },
            }
        }
    };
    history.undo.push(forward);
}
