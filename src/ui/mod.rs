pub mod chrome;
pub mod crafting_egui;

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::ScrollPosition;

use crate::building::{BuildMode, PlaceableItems};
use crate::crafting::{CraftRequest, CraftingState, Recipe, RecipeCategory, RecipeRegistry};
use crate::gathering::NearbyGatherable;
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{ItemId, ItemRegistry, ItemTags};
use chrome::{
    body_text, button_image, panel_image, plain_image, slot_image, title_text,
    UiAssets, ACCENT_GOLD, NEED_RED, TEXT_BODY, TEXT_BODY_DIM, TEXT_DARK_INK, TEXT_FAINT,
    TEXT_GOLD, TEXT_GOLD_DIM,
};

use crate::state::GameState;

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        // Spawning waits for `OnEnter(GameState::Playing)` so that
        // `UiAssets` (loaded by bevy_asset_loader during Loading) and the
        // item/recipe/placeable registries (seeded at Startup) are both in
        // place. Previously this ran at PostStartup, before any state machine
        // existed.
        app.add_systems(OnEnter(GameState::Playing), spawn_ui)
            .add_systems(
                Update,
                (
                    update_gather_prompt,
                    update_build_prompt,
                    scroll_hovered_panels,
                )
                    .run_if(in_state(GameState::Playing)),
            );
        // The crafting menu lives in egui now (W0.6). The Bevy UI tree is no
        // longer spawned (see `spawn_ui`) and the old `update_crafting_menu`
        // / `handle_crafting_clicks` / `handle_crafting_tab_clicks` systems
        // have no entities to operate on.
        crafting_egui::register(app);
    }
}

#[derive(Component)]
struct GatherPrompt;

#[derive(Component)]
struct BuildPrompt;


#[derive(Component)]
struct CraftingMenu;

/// Marker for any UI node that should respond to mouse-wheel scrolling
/// when its hover state (or any descendant's hover state) is active.
#[derive(Component)]
struct Scrollable;

#[derive(Component)]
struct CraftingRecipeRow {
    /// Global index into `RecipeRegistry.recipes`.
    index: usize,
}

#[derive(Component)]
struct CraftingTab {
    category: RecipeCategory,
}

#[derive(Component)]
struct CraftingScrollContainer;

#[derive(Component)]
struct CraftingResultText {
    index: usize,
}

#[derive(Component)]
struct CraftingStatusText {
    index: usize,
}

#[derive(Component)]
struct CraftingIngredientChip {
    recipe: usize,
    ingredient: usize,
}

// Row state tints layered on top of the parchment panel — used by the
// inventory hotbar's selected/hovered slot states.
const ROW_BG_IDLE: Color = Color::srgba(0.0, 0.0, 0.0, 0.0);
const ROW_BG_HOVER: Color = Color::srgba(0.30, 0.18, 0.08, 0.12);
const ROW_BG_SELECTED_OK: Color = Color::srgba(0.86, 0.66, 0.30, 0.40);
const ROW_BG_SELECTED_NO: Color = Color::srgba(0.78, 0.30, 0.26, 0.28);

fn spawn_ui(
    mut commands: Commands,
    assets: Res<UiAssets>,
    asset_server: Res<AssetServer>,
    registry: Res<ItemRegistry>,
    recipes: Res<RecipeRegistry>,
    placeables: Res<PlaceableItems>,
) {
    // Gather prompt (in-world hint band)
    commands
        .spawn((
            GatherPrompt,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(180.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(body_text(&assets, "", 18.0, TEXT_BODY));
        });

    // Build mode prompt (top band)
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
            parent.spawn(body_text(&assets, "", 16.0, TEXT_GOLD_DIM));
        });

    // The bottom inventory hotbar (Bevy UI panel showing every stackable
    // item with counts) was removed — the user is reworking it. The
    // crafting menu lives in egui (W0.6); `spawn_crafting_menu` is kept as
    // a fallback but not called. The build-mode tool palette also lives in
    // egui (`building::ui`).
    let _ = (&assets, &asset_server, &registry, &recipes, &placeables);
    let _ = spawn_crafting_menu;
}

/// Helper: spawn an item swatch in a slot. If the item has a Kenney photo,
/// use it. Otherwise fall back to a procedurally-shaped colour rectangle whose
/// proportions and rounding mirror the placed primitive (so a Wall reads as a
/// tall thin tile, a Lantern as a capsule, a Wreath as a ring, etc.).
fn spawn_item_swatch(
    parent: &mut ChildSpawnerCommands,
    asset_server: &AssetServer,
    def: &crate::items::ItemDef,
    size: f32,
) {
    if let Some(path) = def.form.icon_path() {
        parent.spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                ..default()
            },
            ImageNode::new(asset_server.load(path)),
        ));
        return;
    }

    // Procedural shape swatch -- centre the proportional rect inside a square
    // box of `size` so all slot icons share the same overall footprint.
    let (w_norm, h_norm, r_norm) = def.form.icon_shape();
    let scale = (w_norm.max(h_norm)).max(0.001);
    let sw = size * (w_norm / scale);
    let sh = size * (h_norm / scale);
    let radius = (sw.min(sh)) * r_norm;

    parent
        .spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|wrap| {
            wrap.spawn((
                Node {
                    width: Val::Px(sw),
                    height: Val::Px(sh),
                    border_radius: BorderRadius::all(Val::Px(radius)),
                    ..default()
                },
                BackgroundColor(def.material.base_color()),
            ));
        });
}


fn spawn_crafting_menu(
    commands: &mut Commands,
    assets: &UiAssets,
    asset_server: &AssetServer,
    registry: &ItemRegistry,
    recipes: &RecipeRegistry,
) {
    commands
        .spawn((
            CraftingMenu,
            Button,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                margin: UiRect {
                    left: Val::Px(-300.0),
                    top: Val::Px(-280.0),
                    ..default()
                },
                width: Val::Px(600.0),
                height: Val::Px(560.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    // extra top padding so the hanging banner overlaps cleanly
                    top: Val::Px(70.0),
                    bottom: Val::Px(28.0),
                    left: Val::Px(40.0),
                    right: Val::Px(40.0),
                },
                row_gap: Val::Px(2.0),
                ..default()
            },
            panel_image(assets.panel_bg.clone()),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            // Hanging banner with the title -- positioned absolute so it
            // protrudes above the panel chrome like a Spiritfarer / Cozy Grove
            // header plate.
            panel
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(-44.0),
                        left: Val::Percent(50.0),
                        margin: UiRect::left(Val::Px(-150.0)),
                        width: Val::Px(300.0),
                        height: Val::Px(96.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::top(Val::Px(20.0)),
                        ..default()
                    },
                    plain_image(assets.banner.clone()),
                ))
                .with_children(|b| {
                    b.spawn(title_text(assets, "Crafting", 26.0));
                });

            // Tab strip
            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(4.0),
                    margin: UiRect::bottom(Val::Px(8.0)),
                    ..default()
                })
                .with_children(|tabs| {
                    for &cat in RecipeCategory::ALL {
                        tabs.spawn((
                            Button,
                            CraftingTab { category: cat },
                            Node {
                                width: Val::Px(96.0),
                                height: Val::Px(36.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                                ..default()
                            },
                            button_image(assets.button.clone()),
                        ))
                        .with_children(|tab| {
                            tab.spawn((
                                Text::new(cat.label()),
                                TextFont {
                                    font: assets.body_font.clone(),
                                    font_size: 13.0,
                                    ..default()
                                },
                                TextColor(TEXT_BODY),
                            ));
                        });
                    }
                });

            // Top divider
            panel.spawn((
                Node {
                    width: Val::Percent(85.0),
                    height: Val::Px(8.0),
                    margin: UiRect {
                        left: Val::Auto,
                        right: Val::Auto,
                        bottom: Val::Px(6.0),
                        ..default()
                    },
                    ..default()
                },
                ImageNode::new(assets.divider.clone()),
            ));

            // Scrollable inner container -- holds ALL rows; visibility filtered
            // by the selected category in `update_crafting_menu`.
            panel
                .spawn((
                    Scrollable,
                    CraftingScrollContainer,
                    Interaction::None,
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition::default(),
                ))
                .with_children(|scroll| {
                    for (i, recipe) in recipes.recipes.iter().enumerate() {
                        spawn_recipe_row(scroll, assets, asset_server, registry, recipe, i);
                    }
                });

            // Footer divider + flourish + hint
            panel.spawn((
                Node {
                    width: Val::Percent(80.0),
                    height: Val::Px(8.0),
                    margin: UiRect {
                        left: Val::Auto,
                        right: Val::Auto,
                        top: Val::Px(10.0),
                        ..default()
                    },
                    ..default()
                },
                ImageNode::new(assets.divider.clone()),
            ));

            panel
                .spawn(Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(6.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                })
                .with_children(|footer| {
                    footer.spawn((
                        Node {
                            width: Val::Px(180.0),
                            height: Val::Px(28.0),
                            ..default()
                        },
                        ImageNode::new(assets.flourish.clone()),
                    ));
                    footer.spawn(body_text(
                        assets,
                        "[Tab] close   ·   [W/S] select   ·   [E] craft",
                        11.0,
                        TEXT_FAINT,
                    ));
                });
        });
}

fn spawn_recipe_row(
    panel: &mut ChildSpawnerCommands,
    assets: &UiAssets,
    asset_server: &AssetServer,
    registry: &ItemRegistry,
    recipe: &Recipe,
    i: usize,
) {
    let result_def = registry.get(recipe.result);
    let result_name = result_def
        .map(|d| d.display_name.clone())
        .unwrap_or_else(|| "???".to_string());

    panel
        .spawn((
            Button,
            CraftingRecipeRow { index: i },
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(46.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                margin: UiRect::vertical(Val::Px(2.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(ROW_BG_IDLE),
        ))
        .with_children(|row| {
            // Shortcut number badge
            let label = if i < 9 { format!("{}", i + 1) } else { String::new() };
            row.spawn((
                Node {
                    width: Val::Px(22.0),
                    height: Val::Px(22.0),
                    flex_shrink: 0.0,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(11.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.30)),
            ))
            .with_children(|b| {
                b.spawn(body_text(assets, &label, 11.0, TEXT_GOLD_DIM));
            });

            // Result swatch (icon if available, else colour)
            if let Some(def) = result_def {
                spawn_item_swatch(row, asset_server, def, 30.0);
            }

            // Result name + count -- fixed column so all rows align
            row.spawn((
                CraftingResultText { index: i },
                Text::new(format!("{} x{}", result_name, recipe.result_count)),
                TextFont {
                    font: assets.body_font.clone(),
                    font_size: 14.0,
                    ..default()
                },
                TextColor(TEXT_BODY),
                Node {
                    width: Val::Px(120.0),
                    flex_shrink: 0.0,
                    ..default()
                },
            ));

            // Ingredients column -- flexes to fill available row space, clips
            // overflow so status pill stays anchored to the right edge.
            row.spawn((Node {
                flex_grow: 1.0,
                flex_shrink: 1.0,
                min_width: Val::Px(0.0),
                height: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(4.0),
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },))
                .with_children(|chips| {
                    for (j, (item, count)) in recipe.ingredients.iter().enumerate() {
                        let ing_def = registry.get(*item);
                        let ing_name = ing_def
                            .map(|d| d.display_name.clone())
                            .unwrap_or_else(|| "???".to_string());
                        chips
                            .spawn((
                                CraftingIngredientChip {
                                    recipe: i,
                                    ingredient: j,
                                },
                                Node {
                                    flex_shrink: 0.0,
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                                    column_gap: Val::Px(3.0),
                                    border_radius: BorderRadius::all(Val::Px(5.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
                            ))
                            .with_children(|chip| {
                                if let Some(d) = ing_def {
                                    spawn_item_swatch(chip, asset_server, d, 18.0);
                                }
                                chip.spawn(body_text(
                                    assets,
                                    &format!("{}\u{00A0}x{}", ing_name, count),
                                    10.0,
                                    TEXT_BODY_DIM,
                                ));
                            });
                    }
                });

            // Status pill -- fixed-width final column anchored to row right edge
            row.spawn((
                CraftingStatusText { index: i },
                Text::new(""),
                TextFont {
                    font: assets.body_font.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(TEXT_BODY),
                Node {
                    width: Val::Px(72.0),
                    height: Val::Px(22.0),
                    flex_shrink: 0.0,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
            ));
        });
}

fn update_gather_prompt(
    nearby: Option<Res<NearbyGatherable>>,
    crafting: Res<CraftingState>,
    build_mode: Option<Res<BuildMode>>,
    cursor: Res<crate::input::CursorState>,
    registry: Res<ItemRegistry>,
    prompt_query: Query<&Children, With<GatherPrompt>>,
    mut text_query: Query<&mut Text>,
) {
    let Ok(children) = prompt_query.single() else { return };
    let gp = cursor.using_gamepad;

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
                        let name = registry
                            .get(nearby.item)
                            .map(|d| d.display_name.as_str())
                            .unwrap_or("?");
                        **text = format!("{} Gather {}    {}    {}", interact, name, craft, build);
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
    inventory: Res<Inventory>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    prompt_query: Query<&Children, With<BuildPrompt>>,
    mut text_query: Query<&mut Text>,
) {
    let Ok(children) = prompt_query.single() else { return };

    for child in children.iter() {
        if let Ok(mut text) = text_query.get_mut(child) {
            match &build_mode {
                Some(mode) => {
                    let has_any = placeables.0.iter().any(|id| inventory.count(*id) > 0);
                    **text = if has_any {
                        let name = mode
                            .selected_item(&placeables)
                            .and_then(|id| registry.get(id))
                            .map(|d| d.display_name.as_str())
                            .unwrap_or("?");
                        format!(
                            "BUILD: {}  --  click slot or [1-9] select  [click/Space] place  [R] rotate  [B] exit",
                            name
                        )
                    } else {
                        "BUILD: nothing to place yet -- open [Tab] crafting and make a Fence, Bench, Wall, Floor, ...".into()
                    };
                }
                None => {
                    **text = String::new();
                }
            }
        }
    }
}


fn update_crafting_menu(
    crafting: Res<CraftingState>,
    inventory: Res<Inventory>,
    recipes: Res<RecipeRegistry>,
    mut menu_vis: Query<&mut Visibility, With<CraftingMenu>>,
    mut tabs: Query<
        (&CraftingTab, &mut ImageNode),
        (Without<CraftingRecipeRow>, Without<CraftingIngredientChip>, Without<CraftingStatusText>),
    >,
    mut rows: Query<(&CraftingRecipeRow, &Interaction, &mut BackgroundColor, &mut Node)>,
    mut result_texts: Query<
        (&CraftingResultText, &mut TextColor),
        (Without<CraftingStatusText>,),
    >,
    mut chips: Query<
        (&CraftingIngredientChip, &mut BackgroundColor),
        Without<CraftingRecipeRow>,
    >,
    mut chip_texts: Query<
        (&ChildOf, &mut TextColor),
        (
            Without<CraftingResultText>,
            Without<CraftingStatusText>,
            With<Text>,
        ),
    >,
    chip_lookup: Query<(Entity, &CraftingIngredientChip)>,
    mut status: Query<
        (&CraftingStatusText, &mut Text, &mut TextColor, &mut BackgroundColor),
        (
            Without<CraftingRecipeRow>,
            Without<CraftingIngredientChip>,
            Without<CraftingResultText>,
        ),
    >,
) {
    let Ok(mut vis) = menu_vis.single_mut() else { return };
    *vis = if crafting.open { Visibility::Visible } else { Visibility::Hidden };
    if !crafting.open {
        return;
    }

    // Tabs: full-bright on active, dimmed on inactive (we tint the painted button image directly).
    for (tab, mut img) in &mut tabs {
        img.color = if tab.category == crafting.category {
            Color::srgb(1.15, 1.05, 0.75)
        } else {
            Color::srgba(0.78, 0.72, 0.62, 0.95)
        };
    }

    let selected_global = recipes
        .iter_category(crafting.category)
        .nth(crafting.selected_in_category)
        .map(|(g, _)| g);

    for (row, interaction, mut bg, mut node) in &mut rows {
        let Some(recipe) = recipes.recipes.get(row.index) else { continue };

        // Hide rows in other categories from the layout entirely.
        let in_category = recipe.category == crafting.category;
        node.display = if in_category {
            Display::Flex
        } else {
            Display::None
        };
        if !in_category {
            continue;
        }

        let can_craft = crate::crafting::recipe_satisfied(recipe, &inventory);
        let is_selected = Some(row.index) == selected_global;
        let is_hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);

        *bg = BackgroundColor(if is_selected && can_craft {
            ROW_BG_SELECTED_OK
        } else if is_selected {
            ROW_BG_SELECTED_NO
        } else if is_hovered {
            ROW_BG_HOVER
        } else {
            ROW_BG_IDLE
        });

        for (rt, mut color) in &mut result_texts {
            if rt.index == row.index {
                *color = TextColor(if can_craft { TEXT_BODY } else { TEXT_BODY_DIM });
            }
        }
    }

    for (chip, mut bg) in &mut chips {
        let Some(recipe) = recipes.recipes.get(chip.recipe) else { continue };
        let Some((item, need)) = recipe.ingredients.get(chip.ingredient).copied() else { continue };
        let have_enough = inventory.count(item) >= need;
        *bg = BackgroundColor(if have_enough {
            Color::srgba(0.30, 0.18, 0.08, 0.18)
        } else {
            Color::srgba(0.78, 0.30, 0.26, 0.32)
        });
    }

    let chip_index: std::collections::HashMap<Entity, &CraftingIngredientChip> =
        chip_lookup.iter().collect();
    for (parent, mut color) in &mut chip_texts {
        let Some(chip) = chip_index.get(&parent.parent()) else { continue };
        let Some(recipe) = recipes.recipes.get(chip.recipe) else { continue };
        let Some((item, need)) = recipe.ingredients.get(chip.ingredient).copied() else { continue };
        let have_enough = inventory.count(item) >= need;
        *color = TextColor(if have_enough { TEXT_BODY } else { NEED_RED });
    }

    for (st, mut text, mut color, mut bg) in &mut status {
        let Some(recipe) = recipes.recipes.get(st.index) else { continue };
        let can_craft = crate::crafting::recipe_satisfied(recipe, &inventory);
        let is_selected = Some(st.index) == selected_global;

        if can_craft {
            **text = "CRAFT".to_string();
            *color = TextColor(TEXT_DARK_INK);
            *bg = BackgroundColor(if is_selected {
                Color::srgb(0.92, 0.78, 0.40)
            } else {
                Color::srgba(0.92, 0.78, 0.40, 0.85)
            });
        } else {
            **text = "need more".to_string();
            *color = TextColor(Color::srgb(0.55, 0.20, 0.20));
            *bg = BackgroundColor(Color::srgba(0.78, 0.30, 0.26, 0.30));
        }
    }
}

fn handle_crafting_clicks(
    mut state: ResMut<CraftingState>,
    recipes: Res<RecipeRegistry>,
    rows: Query<(&Interaction, &CraftingRecipeRow), Changed<Interaction>>,
    mut craft_events: MessageWriter<CraftRequest>,
) {
    if !state.open {
        return;
    }
    for (interaction, row) in &rows {
        if !matches!(interaction, Interaction::Pressed) {
            continue;
        }
        if let Some(pos) = recipes
            .iter_category(state.category)
            .position(|(g, _)| g == row.index)
        {
            state.selected_in_category = pos;
        }
        craft_events.write(CraftRequest { index: row.index });
    }
}

fn handle_crafting_tab_clicks(
    mut state: ResMut<CraftingState>,
    tabs: Query<(&Interaction, &CraftingTab), Changed<Interaction>>,
    mut scroll: Query<&mut ScrollPosition, With<CraftingScrollContainer>>,
) {
    for (interaction, tab) in &tabs {
        if matches!(interaction, Interaction::Pressed) && state.category != tab.category {
            state.category = tab.category;
            state.selected_in_category = 0;
            for mut sp in &mut scroll {
                sp.0.y = 0.0;
            }
        }
    }
}


/// Mouse-wheel scroll for any node tagged `Scrollable` whose subtree contains
/// a hovered/pressed UI element. Bevy 0.16 supports `Overflow::scroll_y()` and
/// `ScrollPosition` for content offset, but does not auto-handle the wheel.
fn scroll_hovered_panels(
    mut wheel: MessageReader<MouseWheel>,
    interactions: Query<&Interaction>,
    children_q: Query<&Children>,
    mut scrollables: Query<(Entity, &mut ScrollPosition), With<Scrollable>>,
) {
    let dy: f32 = wheel
        .read()
        .map(|e| match e.unit {
            MouseScrollUnit::Line => e.y * 28.0,
            MouseScrollUnit::Pixel => e.y,
        })
        .sum();
    if dy == 0.0 {
        return;
    }

    for (root, mut scroll) in &mut scrollables {
        if subtree_has_active_interaction(root, &interactions, &children_q) {
            scroll.0.y = (scroll.0.y - dy).max(0.0);
        }
    }
}

fn subtree_has_active_interaction(
    root: Entity,
    interactions: &Query<&Interaction>,
    children_q: &Query<&Children>,
) -> bool {
    if matches!(
        interactions.get(root).copied().unwrap_or(Interaction::None),
        Interaction::Hovered | Interaction::Pressed
    ) {
        return true;
    }
    if let Ok(children) = children_q.get(root) {
        for child in children.iter() {
            if subtree_has_active_interaction(child, interactions, children_q) {
                return true;
            }
        }
    }
    false
}


