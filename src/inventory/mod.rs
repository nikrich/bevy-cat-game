use bevy::prelude::*;
use std::collections::HashMap;

use crate::items::{ItemId, ItemRegistry, ItemTags};

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .add_message::<InventoryChanged>()
            // Fresh-world starter inventory: fills 50 of every stackable item
            // if no save loaded any. Runs in PostStartup so it sees the result
            // of save's load_game pass.
            .add_systems(PostStartup, dev_starter_inventory)
            // F2: top up every stackable item to 999 for build-tool testing.
            .add_systems(Update, debug_top_up_inventory);
    }
}

fn debug_top_up_inventory(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut inventory: ResMut<Inventory>,
    registry: Res<ItemRegistry>,
    mut inv_events: MessageWriter<InventoryChanged>,
) {
    if !keyboard.just_pressed(KeyCode::F2) {
        return;
    }
    const TARGET: u32 = 999;
    for def in registry.all() {
        if !def.tags.contains(ItemTags::STACKABLE) {
            continue;
        }
        let current = inventory.count(def.id);
        if current < TARGET {
            inventory.add(def.id, TARGET - current);
            inv_events.write(InventoryChanged {
                item: def.id,
                new_count: TARGET,
            });
        }
    }
    info!("F2: topped up every stackable item to {}", TARGET);
}

fn dev_starter_inventory(
    mut inventory: ResMut<Inventory>,
    registry: Res<ItemRegistry>,
    mut inv_events: MessageWriter<InventoryChanged>,
) {
    if !inventory.items.is_empty() {
        return;
    }
    for def in registry.all() {
        if def.tags.contains(ItemTags::STACKABLE) {
            inventory.add(def.id, 50);
            inv_events.write(InventoryChanged { item: def.id, new_count: 50 });
        }
    }
    info!("Dev starter inventory: 50 of every stackable item");
}

#[derive(Resource, Default)]
pub struct Inventory {
    pub items: HashMap<ItemId, u32>,
}

impl Inventory {
    pub fn add(&mut self, item: ItemId, count: u32) {
        *self.items.entry(item).or_insert(0) += count;
    }

    pub fn count(&self, item: ItemId) -> u32 {
        self.items.get(&item).copied().unwrap_or(0)
    }
}

#[derive(Message)]
pub struct InventoryChanged {
    pub item: ItemId,
    pub new_count: u32,
}
