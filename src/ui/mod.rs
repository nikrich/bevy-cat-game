use bevy::prelude::*;

use crate::building::BuildMode;
use crate::crafting::{CraftingState, RECIPES};
use crate::gathering::NearbyGatherable;
use crate::inventory::{Inventory, InventoryChanged, ItemKind};

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_ui)
            .add_systems(
                Update,
                (
                    update_gather_prompt,
                    update_inventory_display,
                    update_crafting_menu,
                    update_build_prompt,
                ),
            );
    }
}

#[derive(Component)]
struct GatherPrompt;

#[derive(Component)]
struct BuildPrompt;

#[derive(Component)]
struct InventoryBar;

#[derive(Component)]
struct InventorySlot {
    item: ItemKind,
}

#[derive(Component)]
struct InventoryCount {
    item: ItemKind,
}

#[derive(Component)]
struct CraftingMenu;

#[derive(Component)]
struct CraftingRecipeRow {
    index: usize,
}

#[derive(Component)]
struct CraftingResultText {
    index: usize,
}

#[derive(Component)]
struct CraftingIngredientText {
    index: usize,
}

#[derive(Component)]
struct CraftingStatusText {
    index: usize,
}

const ALL_ITEMS: &[ItemKind] = &[
    ItemKind::Wood,
    ItemKind::PineWood,
    ItemKind::Stone,
    ItemKind::Flower,
    ItemKind::Mushroom,
    ItemKind::Bush,
    ItemKind::Cactus,
    ItemKind::Plank,
    ItemKind::StoneBrick,
    ItemKind::Fence,
    ItemKind::Bench,
    ItemKind::Lantern,
    ItemKind::FlowerPot,
    ItemKind::Stew,
    ItemKind::Wreath,
];

fn spawn_ui(mut commands: Commands) {
    // Gather prompt
    commands
        .spawn((
            GatherPrompt,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(100.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont { font_size: 18.0, ..default() },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.9)),
            ));
        });

    // Build mode prompt
    commands
        .spawn((
            BuildPrompt,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(16.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(""),
                TextFont { font_size: 16.0, ..default() },
                TextColor(Color::srgba(1.0, 1.0, 0.7, 0.9)),
            ));
        });

    // Inventory hotbar
    commands
        .spawn((
            InventoryBar,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::End,
                column_gap: Val::Px(4.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            for &item in ALL_ITEMS {
                parent
                    .spawn((
                        InventorySlot { item },
                        Node {
                            width: Val::Px(58.0),
                            height: Val::Px(72.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
                        BorderRadius::all(Val::Px(6.0)),
                        Visibility::Hidden,
                    ))
                    .with_children(|slot| {
                        slot.spawn((
                            Node {
                                width: Val::Px(28.0),
                                height: Val::Px(28.0),
                                margin: UiRect::bottom(Val::Px(2.0)),
                                ..default()
                            },
                            BackgroundColor(item.color().into()),
                            BorderRadius::all(Val::Px(4.0)),
                        ));
                        slot.spawn((
                            Text::new(item.display_name()),
                            TextFont { font_size: 10.0, ..default() },
                            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7)),
                        ));
                        slot.spawn((
                            InventoryCount { item },
                            Text::new("0"),
                            TextFont { font_size: 14.0, ..default() },
                            TextColor(Color::WHITE),
                        ));
                    });
            }
        });

    // --- Crafting menu (center panel) ---
    commands
        .spawn((
            CraftingMenu,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                margin: UiRect {
                    left: Val::Px(-220.0),
                    top: Val::Px(-200.0),
                    ..default()
                },
                width: Val::Px(440.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.06, 0.04, 0.92)),
            BorderRadius::all(Val::Px(12.0)),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            // Header
            panel.spawn((
                Node {
                    margin: UiRect::bottom(Val::Px(12.0)),
                    ..default()
                },
                Text::new("Crafting"),
                TextFont { font_size: 22.0, ..default() },
                TextColor(Color::srgb(0.95, 0.88, 0.70)),
            ));

            // Recipe rows
            for (i, recipe) in RECIPES.iter().enumerate() {
                panel
                    .spawn((
                        CraftingRecipeRow { index: i },
                        Node {
                            width: Val::Percent(100.0),
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(8.0)),
                            flex_direction: FlexDirection::Row,
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(8.0),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
                        BorderRadius::all(Val::Px(6.0)),
                    ))
                    .with_children(|row| {
                        // Result color swatch
                        row.spawn((
                            Node {
                                width: Val::Px(22.0),
                                height: Val::Px(22.0),
                                ..default()
                            },
                            BackgroundColor(recipe.result.color().into()),
                            BorderRadius::all(Val::Px(4.0)),
                        ));

                        // Result name + count
                        row.spawn((
                            CraftingResultText { index: i },
                            Text::new(format!(
                                "{} x{}",
                                recipe.result.display_name(),
                                recipe.result_count
                            )),
                            TextFont { font_size: 15.0, ..default() },
                            TextColor(Color::WHITE),
                            Node { width: Val::Px(90.0), ..default() },
                        ));

                        // Arrow
                        row.spawn((
                            Text::new("<-"),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.4)),
                        ));

                        // Ingredients
                        let ingredients_str: String = recipe
                            .ingredients
                            .iter()
                            .map(|(item, count)| format!("{} x{}", item.display_name(), count))
                            .collect::<Vec<_>>()
                            .join("  +  ");

                        row.spawn((
                            CraftingIngredientText { index: i },
                            Text::new(ingredients_str),
                            TextFont { font_size: 13.0, ..default() },
                            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.6)),
                        ));

                        // Status indicator (right side)
                        row.spawn((
                            CraftingStatusText { index: i },
                            Text::new(""),
                            TextFont { font_size: 12.0, ..default() },
                            TextColor(Color::srgb(0.5, 0.8, 0.4)),
                            Node { margin: UiRect::left(Val::Auto), ..default() },
                        ));
                    });
            }

            // Footer
            panel.spawn((
                Node {
                    margin: UiRect::top(Val::Px(12.0)),
                    ..default()
                },
                Text::new("[W/S] Select    [E] Craft    [Tab] Close"),
                TextFont { font_size: 12.0, ..default() },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 0.4)),
            ));
        });
}

fn update_gather_prompt(
    nearby: Option<Res<NearbyGatherable>>,
    crafting: Res<CraftingState>,
    build_mode: Option<Res<BuildMode>>,
    input: Res<crate::input::GameInput>,
    prompt_query: Query<&Children, With<GatherPrompt>>,
    mut text_query: Query<&mut Text>,
) {
    let Ok(children) = prompt_query.single() else { return };
    let gp = input.using_gamepad;

    for child in children.iter() {
        if let Ok(mut text) = text_query.get_mut(child) {
            if crafting.open || build_mode.is_some() {
                **text = String::new();
            } else {
                let interact = if gp { "[A]" } else { "[E/Click]" };
                let craft = if gp { "[X] Craft" } else { "[Tab] Craft" };
                let build = if gp { "[Y] Build" } else { "[B] Build" };

                match &nearby {
                    Some(nearby) => {
                        **text = format!(
                            "{} Gather {}    {}    {}",
                            interact,
                            nearby.item.display_name(),
                            craft,
                            build
                        );
                    }
                    None => {
                        **text = format!("{}    {}", craft, build);
                    }
                }
            }
        }
    }
}

fn update_build_prompt(
    build_mode: Option<Res<BuildMode>>,
    prompt_query: Query<&Children, With<BuildPrompt>>,
    mut text_query: Query<&mut Text>,
) {
    let Ok(children) = prompt_query.single() else { return };

    for child in children.iter() {
        if let Ok(mut text) = text_query.get_mut(child) {
            match &build_mode {
                Some(mode) => {
                    **text = format!(
                        "BUILD: {}  --  [1-5] Select  [Space] Place  [R] Rotate  [B] Exit",
                        mode.selected_item().display_name()
                    );
                }
                None => {
                    **text = String::new();
                }
            }
        }
    }
}

fn update_inventory_display(
    inventory: Res<Inventory>,
    mut inv_events: EventReader<InventoryChanged>,
    mut slots: Query<(&InventorySlot, &mut Visibility)>,
    mut counts: Query<(&InventoryCount, &mut Text)>,
) {
    if inv_events.read().next().is_none() {
        return;
    }

    for (slot, mut vis) in &mut slots {
        let count = inventory.count(slot.item);
        *vis = if count > 0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }

    for (count_comp, mut text) in &mut counts {
        let count = inventory.count(count_comp.item);
        if count > 0 {
            **text = count.to_string();
        }
    }
}

fn update_crafting_menu(
    crafting: Res<CraftingState>,
    inventory: Res<Inventory>,
    mut menu_vis: Query<&mut Visibility, With<CraftingMenu>>,
    mut rows: Query<(&CraftingRecipeRow, &mut BackgroundColor)>,
    mut result_texts: Query<(&CraftingResultText, &mut TextColor)>,
    mut ingredient_texts: Query<(&CraftingIngredientText, &mut TextColor), Without<CraftingResultText>>,
    mut status_texts: Query<(&CraftingStatusText, &mut Text), Without<CraftingIngredientText>>,
) {
    let Ok(mut vis) = menu_vis.single_mut() else { return };

    *vis = if crafting.open {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    if !crafting.open {
        return;
    }

    for (row, mut bg) in &mut rows {
        let recipe = &RECIPES[row.index];
        let can_craft = recipe
            .ingredients
            .iter()
            .all(|(item, count)| inventory.count(*item) >= *count);
        let is_selected = row.index == crafting.selected;

        *bg = if is_selected && can_craft {
            BackgroundColor(Color::srgba(0.4, 0.6, 0.3, 0.3))
        } else if is_selected {
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.1))
        } else {
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0))
        };

        // Result text color
        for (rt, mut color) in &mut result_texts {
            if rt.index == row.index {
                *color = if can_craft {
                    TextColor(Color::WHITE)
                } else {
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.3))
                };
            }
        }

        // Ingredient text color
        for (it, mut color) in &mut ingredient_texts {
            if it.index == row.index {
                *color = if can_craft {
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.7))
                } else {
                    TextColor(Color::srgba(1.0, 1.0, 1.0, 0.2))
                };
            }
        }

        // Status text
        for (st, mut text) in &mut status_texts {
            if st.index == row.index {
                if is_selected && can_craft {
                    **text = "[E]".to_string();
                } else if !can_craft {
                    **text = "need more".to_string();
                } else {
                    **text = String::new();
                }
            }
        }
    }
}
