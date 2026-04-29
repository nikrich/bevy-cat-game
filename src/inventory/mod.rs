use bevy::prelude::*;
use std::collections::HashMap;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .add_event::<InventoryChanged>();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    // Raw materials
    Wood,
    Stone,
    Flower,
    Mushroom,
    Bush,
    Cactus,
    PineWood,
    // Crafted items
    Plank,
    StoneBrick,
    Fence,
    Bench,
    Lantern,
    FlowerPot,
    Stew,
    Wreath,
}

impl ItemKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            ItemKind::Wood => "Wood",
            ItemKind::Stone => "Stone",
            ItemKind::Flower => "Flower",
            ItemKind::Mushroom => "Mushroom",
            ItemKind::Bush => "Bush",
            ItemKind::Cactus => "Cactus",
            ItemKind::PineWood => "Pine",
            ItemKind::Plank => "Plank",
            ItemKind::StoneBrick => "Brick",
            ItemKind::Fence => "Fence",
            ItemKind::Bench => "Bench",
            ItemKind::Lantern => "Lantern",
            ItemKind::FlowerPot => "Pot",
            ItemKind::Stew => "Stew",
            ItemKind::Wreath => "Wreath",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            ItemKind::Wood => Color::srgb(0.55, 0.38, 0.22),
            ItemKind::Stone => Color::srgb(0.55, 0.52, 0.48),
            ItemKind::Flower => Color::srgb(0.85, 0.45, 0.50),
            ItemKind::Mushroom => Color::srgb(0.75, 0.35, 0.30),
            ItemKind::Bush => Color::srgb(0.32, 0.52, 0.28),
            ItemKind::Cactus => Color::srgb(0.35, 0.55, 0.30),
            ItemKind::PineWood => Color::srgb(0.35, 0.28, 0.18),
            ItemKind::Plank => Color::srgb(0.70, 0.55, 0.35),
            ItemKind::StoneBrick => Color::srgb(0.62, 0.60, 0.58),
            ItemKind::Fence => Color::srgb(0.60, 0.45, 0.28),
            ItemKind::Bench => Color::srgb(0.50, 0.35, 0.20),
            ItemKind::Lantern => Color::srgb(0.90, 0.80, 0.40),
            ItemKind::FlowerPot => Color::srgb(0.72, 0.45, 0.35),
            ItemKind::Stew => Color::srgb(0.65, 0.40, 0.25),
            ItemKind::Wreath => Color::srgb(0.40, 0.65, 0.35),
        }
    }

    pub fn is_placeable(&self) -> bool {
        matches!(
            self,
            ItemKind::Fence
                | ItemKind::Bench
                | ItemKind::Lantern
                | ItemKind::FlowerPot
                | ItemKind::Wreath
        )
    }
}

#[derive(Resource, Default)]
pub struct Inventory {
    pub items: HashMap<ItemKind, u32>,
}

impl Inventory {
    pub fn add(&mut self, item: ItemKind, count: u32) {
        *self.items.entry(item).or_insert(0) += count;
    }

    pub fn count(&self, item: ItemKind) -> u32 {
        self.items.get(&item).copied().unwrap_or(0)
    }

    pub fn total_items(&self) -> u32 {
        self.items.values().sum()
    }
}

#[derive(Event)]
pub struct InventoryChanged {
    pub item: ItemKind,
    pub new_count: u32,
}
