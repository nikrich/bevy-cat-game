use bevy::prelude::*;

use crate::input::GameInput;
use crate::inventory::{Inventory, InventoryChanged, ItemKind};

pub struct CraftingPlugin;

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CraftingState>()
            .add_systems(Update, (toggle_crafting_menu, handle_crafting));
    }
}

pub struct Recipe {
    pub result: ItemKind,
    pub result_count: u32,
    pub ingredients: &'static [(ItemKind, u32)],
}

pub const RECIPES: &[Recipe] = &[
    Recipe {
        result: ItemKind::Plank,
        result_count: 2,
        ingredients: &[(ItemKind::Wood, 1)],
    },
    Recipe {
        result: ItemKind::StoneBrick,
        result_count: 2,
        ingredients: &[(ItemKind::Stone, 2)],
    },
    Recipe {
        result: ItemKind::Fence,
        result_count: 1,
        ingredients: &[(ItemKind::Plank, 2), (ItemKind::Wood, 1)],
    },
    Recipe {
        result: ItemKind::Bench,
        result_count: 1,
        ingredients: &[(ItemKind::Plank, 3), (ItemKind::Stone, 1)],
    },
    Recipe {
        result: ItemKind::Lantern,
        result_count: 1,
        ingredients: &[(ItemKind::Stone, 2), (ItemKind::Wood, 1), (ItemKind::Flower, 1)],
    },
    Recipe {
        result: ItemKind::FlowerPot,
        result_count: 1,
        ingredients: &[(ItemKind::Stone, 1), (ItemKind::Flower, 2)],
    },
    Recipe {
        result: ItemKind::Stew,
        result_count: 1,
        ingredients: &[(ItemKind::Mushroom, 2), (ItemKind::Bush, 1)],
    },
    Recipe {
        result: ItemKind::Wreath,
        result_count: 1,
        ingredients: &[(ItemKind::Flower, 3), (ItemKind::Bush, 1)],
    },
];

#[derive(Resource, Default)]
pub struct CraftingState {
    pub open: bool,
    pub selected: usize,
}

fn toggle_crafting_menu(input: Res<GameInput>, mut state: ResMut<CraftingState>) {
    if input.toggle_craft {
        state.open = !state.open;
        state.selected = 0;
    }

    if !state.open {
        return;
    }

    if input.menu_down {
        state.selected = (state.selected + 1).min(RECIPES.len() - 1);
    }
    if input.menu_up {
        state.selected = state.selected.saturating_sub(1);
    }
}

fn can_craft(recipe: &Recipe, inventory: &Inventory) -> bool {
    recipe
        .ingredients
        .iter()
        .all(|(item, count)| inventory.count(*item) >= *count)
}

fn handle_crafting(
    input: Res<GameInput>,
    state: Res<CraftingState>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: EventWriter<InventoryChanged>,
) {
    if !state.open {
        return;
    }

    if input.menu_confirm {
        let recipe = &RECIPES[state.selected];

        if !can_craft(recipe, &inventory) {
            return;
        }

        for (item, count) in recipe.ingredients {
            let entry = inventory.items.entry(*item).or_insert(0);
            *entry = entry.saturating_sub(*count);
        }

        inventory.add(recipe.result, recipe.result_count);

        inv_events.write(InventoryChanged {
            item: recipe.result,
            new_count: inventory.count(recipe.result),
        });
    }
}
