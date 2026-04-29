use bevy::prelude::*;
use std::collections::HashMap;

use crate::items::ItemId;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .add_event::<InventoryChanged>();
    }
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

#[derive(Event)]
pub struct InventoryChanged {
    pub item: ItemId,
    pub new_count: u32,
}
