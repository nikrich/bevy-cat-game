use bevy::prelude::*;

use crate::input::GameInput;
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{Form, ItemId, ItemRegistry, Material};

pub struct CraftingPlugin;

impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CraftingState>()
            .init_resource::<RecipeRegistry>()
            .add_message::<CraftRequest>()
            .add_systems(
                Startup,
                seed_default_recipes.after(crate::items::registry::seed_default_items),
            )
            .add_systems(
                Update,
                (toggle_crafting_menu, handle_crafting, handle_craft_requests),
            );
    }
}

/// Top-level grouping for the recipe browser. Each recipe belongs to one
/// category; the UI shows one category at a time via a tab strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RecipeCategory {
    Refining,
    Furniture,
    Building,
    Decor,
    Food,
}

impl RecipeCategory {
    pub fn label(self) -> &'static str {
        match self {
            RecipeCategory::Refining => "Refining",
            RecipeCategory::Furniture => "Furniture",
            RecipeCategory::Building => "Building",
            RecipeCategory::Decor => "Decor",
            RecipeCategory::Food => "Food",
        }
    }

    pub const ALL: &'static [RecipeCategory] = &[
        RecipeCategory::Refining,
        RecipeCategory::Furniture,
        RecipeCategory::Building,
        RecipeCategory::Decor,
        RecipeCategory::Food,
    ];
}

/// One concrete craftable. For Phase 2, recipes are concrete -- ingredients and
/// result are specific `ItemId`s. Phase 3 added category tags. A future phase
/// will add MaterialFamily templating (one "Chair" recipe accepting any wood,
/// with a material picker in the UI).
#[derive(Clone, Debug)]
pub struct Recipe {
    pub result: ItemId,
    pub result_count: u32,
    pub ingredients: Vec<(ItemId, u32)>,
    pub category: RecipeCategory,
}

#[derive(Resource, Default)]
pub struct RecipeRegistry {
    pub recipes: Vec<Recipe>,
}

#[derive(Resource)]
pub struct CraftingState {
    pub open: bool,
    pub category: RecipeCategory,
    /// Index into the filtered (per-category) list, NOT into recipes.recipes.
    pub selected_in_category: usize,
}

impl Default for CraftingState {
    fn default() -> Self {
        Self {
            open: false,
            category: RecipeCategory::Refining,
            selected_in_category: 0,
        }
    }
}

/// UI clicks emit this to ask for a specific recipe (by index) to be crafted.
#[derive(Message)]
pub struct CraftRequest {
    pub index: usize,
}

fn seed_default_recipes(
    registry: Res<ItemRegistry>,
    mut recipe_registry: ResMut<RecipeRegistry>,
) {
    let r = &registry;
    let lookup = |form: Form, mat: Material| {
        r.lookup(form, mat).unwrap_or_else(|| {
            warn!("Recipe references missing item ({:?}, {:?})", form, mat);
            ItemId(0)
        })
    };

    let oak_log = lookup(Form::Log, Material::Oak);
    let pine_log = lookup(Form::Log, Material::Pine);
    let stone_chunk = lookup(Form::StoneChunk, Material::Stone);
    let flower = lookup(Form::Flower, Material::FlowerMix);
    let mushroom = lookup(Form::Mushroom, Material::Mushroom);
    let bush_sprig = lookup(Form::BushSprig, Material::Bush);

    let oak_plank = lookup(Form::Plank, Material::Oak);
    let pine_plank = lookup(Form::Plank, Material::Pine);
    let stone_brick = lookup(Form::Brick, Material::Stone);

    let mut recipes = Vec::new();

    let mut add = |category, result, result_count, ingredients: Vec<(ItemId, u32)>| {
        recipes.push(Recipe {
            category,
            result,
            result_count,
            ingredients,
        });
    };
    use RecipeCategory::*;

    // Refining
    add(Refining, oak_plank, 2, vec![(oak_log, 1)]);
    add(Refining, pine_plank, 2, vec![(pine_log, 1)]);
    add(Refining, stone_brick, 2, vec![(stone_chunk, 2)]);

    // Furniture
    add(Furniture, lookup(Form::Fence, Material::Oak), 1, vec![(oak_plank, 2), (oak_log, 1)]);
    add(Furniture, lookup(Form::Fence, Material::Pine), 1, vec![(pine_plank, 2), (pine_log, 1)]);
    add(Furniture, lookup(Form::Bench, Material::Oak), 1, vec![(oak_plank, 3), (stone_chunk, 1)]);
    add(Furniture, lookup(Form::Chair, Material::Oak), 1, vec![(oak_plank, 3)]);
    add(Furniture, lookup(Form::Chair, Material::Pine), 1, vec![(pine_plank, 3)]);
    add(Furniture, lookup(Form::Table, Material::Oak), 1, vec![(oak_plank, 4)]);

    // Building (modular structures, foundation of Phase 4)
    add(Building, lookup(Form::Floor, Material::Oak), 1, vec![(oak_plank, 4)]);
    add(Building, lookup(Form::Floor, Material::Pine), 1, vec![(pine_plank, 4)]);
    add(Building, lookup(Form::Floor, Material::Stone), 1, vec![(stone_brick, 4)]);
    add(Building, lookup(Form::Wall, Material::Oak), 1, vec![(oak_plank, 6)]);
    add(Building, lookup(Form::Wall, Material::Pine), 1, vec![(pine_plank, 6)]);
    add(Building, lookup(Form::Wall, Material::Stone), 1, vec![(stone_brick, 6)]);
    add(Building, lookup(Form::Wall, Material::Brick), 1, vec![(stone_brick, 6)]);
    add(Building, lookup(Form::Door, Material::Oak), 1, vec![(oak_plank, 3), (oak_log, 1)]);

    // Decor
    add(Decor, lookup(Form::Lantern, Material::Stone), 1, vec![(stone_chunk, 2), (oak_log, 1), (flower, 1)]);
    add(Decor, lookup(Form::FlowerPot, Material::Stone), 1, vec![(stone_chunk, 1), (flower, 2)]);
    add(Decor, lookup(Form::Wreath, Material::None), 1, vec![(flower, 3), (bush_sprig, 1)]);

    // Food
    add(Food, lookup(Form::Stew, Material::None), 1, vec![(mushroom, 2), (bush_sprig, 1)]);

    info!("RecipeRegistry seeded with {} recipes", recipes.len());
    recipe_registry.recipes = recipes;
}

impl RecipeRegistry {
    /// Returns (global_index, &Recipe) for every recipe in `category`,
    /// preserving registration order. Used by the UI to render one tab.
    pub fn iter_category(&self, category: RecipeCategory) -> impl Iterator<Item = (usize, &Recipe)> {
        self.recipes
            .iter()
            .enumerate()
            .filter(move |(_, r)| r.category == category)
    }
}

fn toggle_crafting_menu(
    input: Res<GameInput>,
    mut state: ResMut<CraftingState>,
    recipes: Res<RecipeRegistry>,
) {
    if input.toggle_craft {
        state.open = !state.open;
        state.selected_in_category = 0;
    }

    if !state.open {
        return;
    }

    let visible_count = recipes.iter_category(state.category).count();
    if visible_count == 0 {
        return;
    }

    if input.menu_down {
        state.selected_in_category = (state.selected_in_category + 1).min(visible_count - 1);
    }
    if input.menu_up {
        state.selected_in_category = state.selected_in_category.saturating_sub(1);
    }
}

fn can_craft(recipe: &Recipe, inventory: &Inventory) -> bool {
    recipe
        .ingredients
        .iter()
        .all(|(item, count)| inventory.count(*item) >= *count)
}

fn try_craft(
    index: usize,
    recipes: &RecipeRegistry,
    inventory: &mut Inventory,
    inv_events: &mut MessageWriter<InventoryChanged>,
) {
    let Some(recipe) = recipes.recipes.get(index) else { return };
    if !can_craft(recipe, inventory) {
        return;
    }

    for (item, count) in &recipe.ingredients {
        let entry = inventory.items.entry(*item).or_insert(0);
        *entry = entry.saturating_sub(*count);
    }

    inventory.add(recipe.result, recipe.result_count);

    inv_events.write(InventoryChanged {
        item: recipe.result,
        new_count: inventory.count(recipe.result),
    });
}

fn handle_crafting(
    input: Res<GameInput>,
    state: Res<CraftingState>,
    recipes: Res<RecipeRegistry>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
) {
    if !state.open {
        return;
    }
    if input.menu_confirm {
        if let Some((global_idx, _)) =
            recipes.iter_category(state.category).nth(state.selected_in_category)
        {
            try_craft(global_idx, &recipes, &mut inventory, &mut inv_events);
        }
    }
}

fn handle_craft_requests(
    mut requests: MessageReader<CraftRequest>,
    recipes: Res<RecipeRegistry>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
) {
    for req in requests.read() {
        try_craft(req.index, &recipes, &mut inventory, &mut inv_events);
    }
}

/// Helper: does the player have everything needed for a recipe?
pub fn recipe_satisfied(recipe: &Recipe, inventory: &Inventory) -> bool {
    can_craft(recipe, inventory)
}
