use bevy::prelude::*;

use crate::items::ItemId;

#[derive(Component)]
pub struct PlacedItem {
    pub item: ItemId,
}
