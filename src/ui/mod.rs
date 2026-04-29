pub mod chrome;

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::ui::ScrollPosition;

use crate::building::{BuildMode, PlaceableItems};
use crate::crafting::{CraftRequest, CraftingState, Recipe, RecipeCategory, RecipeRegistry};
use crate::gathering::NearbyGatherable;
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{ItemId, ItemRegistry, ItemTags};
use chrome::{
    body_text, button_image, load_ui_assets, panel_image, plain_image, slot_image, title_text,
    UiAssets, ACCENT_GOLD, NEED_RED, TEXT_BODY, TEXT_BODY_DIM, TEXT_DARK_INK, TEXT_FAINT,
    TEXT_GOLD, TEXT_GOLD_DIM,
};

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_ui_assets)
            // PostStartup so the item / recipe / placeable registries AND UiAssets are populated.
            .add_systems(PostStartup, spawn_ui)
            .add_systems(
                Update,
                (
                    update_gather_prompt,
                    update_inventory_display,
                    update_crafting_menu,
                    handle_crafting_clicks,
                    handle_crafting_tab_clicks,
                    update_build_prompt,
                    update_build_hotbar,
                    handle_build_hotbar_clicks,
                    handle_inventory_placeable_clicks,
                    scroll_hovered_panels,
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
    item: ItemId,
}

#[derive(Component)]
struct InventoryCount {
    item: ItemId,
}

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

#[derive(Component)]
struct BuildHotbar;

#[derive(Component)]
struct BuildHotbarSlot {
    index: usize,
}

#[derive(Component)]
struct BuildHotbarCount {
    index: usize,
}

// Row state tints layered on top of the parchment panel.
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

    spawn_inventory_bar(&mut commands, &assets, &asset_server, &registry);
    spawn_crafting_menu(&mut commands, &assets, &asset_server, &registry, &recipes);
    spawn_build_hotbar(&mut commands, &assets, &asset_server, &registry, &placeables);
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

fn spawn_inventory_bar(
    commands: &mut Commands,
    assets: &UiAssets,
    asset_server: &AssetServer,
    registry: &ItemRegistry,
) {
    let stackables: Vec<&crate::items::ItemDef> = registry
        .iter_with_tag(ItemTags::STACKABLE)
        .collect();

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
                flex_wrap: FlexWrap::Wrap,
                ..default()
            },
        ))
        .with_children(|parent| {
            for def in stackables {
                parent
                    .spawn((
                        Button,
                        InventorySlot { item: def.id },
                        Node {
                            width: Val::Px(56.0),
                            height: Val::Px(68.0),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            padding: UiRect::all(Val::Px(6.0)),
                            ..default()
                        },
                        slot_image(assets.panel_dark.clone()),
                        Visibility::Hidden,
                    ))
                    .with_children(|slot| {
                        spawn_item_swatch(slot, asset_server, def, 32.0);
                        slot.spawn(body_text(assets, &def.display_name, 9.0, TEXT_BODY_DIM));
                        slot.spawn((
                            InventoryCount { item: def.id },
                            body_text(assets, "0", 14.0, TEXT_GOLD),
                        ));
                    });
            }
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

fn spawn_build_hotbar(
    commands: &mut Commands,
    assets: &UiAssets,
    asset_server: &AssetServer,
    registry: &ItemRegistry,
    placeables: &PlaceableItems,
) {
    // Bottom horizontal hotbar with wrap. The inventory hotbar is hidden during
    // build mode (see `update_inventory_display`), so this slot location is
    // free of overlap. Wrap means 14+ placeables can reflow onto a second row
    // without running off the screen.
    commands
        .spawn((
            BuildHotbar,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(20.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::End,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::WrapReverse,
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(40.0), Val::Px(0.0)),
                ..default()
            },
            Visibility::Hidden,
        ))
        .with_children(|bar| {
            for (i, item_id) in placeables.0.iter().enumerate() {
                let def = registry.get(*item_id);
                let color = def.map(|d| d.material.base_color()).unwrap_or(Color::WHITE);
                let name = def
                    .map(|d| d.display_name.clone())
                    .unwrap_or_else(|| "?".into());
                bar.spawn((
                    Button,
                    BuildHotbarSlot { index: i },
                    Node {
                        width: Val::Px(74.0),
                        height: Val::Px(80.0),
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        padding: UiRect::all(Val::Px(6.0)),
                        row_gap: Val::Px(2.0),
                        ..default()
                    },
                    slot_image(assets.panel_dark.clone()),
                    BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                ))
                .with_children(|slot| {
                    // Shortcut number (top-left absolute, doesn't affect column layout)
                    let label = if i < 9 { format!("{}", i + 1) } else { String::new() };
                    slot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(2.0),
                            left: Val::Px(5.0),
                            ..default()
                        },
                        body_text(assets, &label, 9.0, TEXT_GOLD_DIM),
                    ));
                    // Item swatch (icon if available, else colour)
                    if let Some(d) = def {
                        spawn_item_swatch(slot, asset_server, d, 36.0);
                    }
                    // Item name (small text)
                    slot.spawn((
                        Text::new(name),
                        TextFont {
                            font: assets.body_font.clone(),
                            font_size: 9.0,
                            ..default()
                        },
                        TextColor(TEXT_GOLD),
                    ));
                    // Count (bottom-right absolute so it doesn't shift the swatch)
                    slot.spawn((
                        BuildHotbarCount { index: i },
                        Node {
                            position_type: PositionType::Absolute,
                            bottom: Val::Px(2.0),
                            right: Val::Px(6.0),
                            ..default()
                        },
                        body_text(assets, "0", 12.0, TEXT_GOLD),
                    ));
                });
            }
        });
}

fn update_gather_prompt(
    nearby: Option<Res<NearbyGatherable>>,
    crafting: Res<CraftingState>,
    build_mode: Option<Res<BuildMode>>,
    input: Res<crate::input::GameInput>,
    registry: Res<ItemRegistry>,
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

fn update_inventory_display(
    inventory: Res<Inventory>,
    build_mode: Option<Res<BuildMode>>,
    crafting: Res<CraftingState>,
    mut inv_events: MessageReader<InventoryChanged>,
    mut bar_vis: Query<&mut Visibility, (With<InventoryBar>, Without<InventorySlot>)>,
    mut slots: Query<(&InventorySlot, &mut Visibility), Without<InventoryBar>>,
    mut counts: Query<(&InventoryCount, &mut Text)>,
) {
    // Hide the whole inventory hotbar whenever a center panel (build mode or
    // crafting) is open, so it doesn't peek out from below.
    let hide_bar = build_mode.is_some() || crafting.open;
    let bar_target = if hide_bar {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut vis in &mut bar_vis {
        *vis = bar_target;
    }

    let inv_dirty = inv_events.read().next().is_some();
    let mode_changed = build_mode.as_ref().is_some_and(|m| m.is_changed());
    if !inv_dirty && !mode_changed {
        return;
    }

    for (slot, mut vis) in &mut slots {
        let count = inventory.count(slot.item);
        *vis = if count > 0 { Visibility::Visible } else { Visibility::Hidden };
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

fn update_build_hotbar(
    build_mode: Option<Res<BuildMode>>,
    inventory: Res<Inventory>,
    placeables: Res<PlaceableItems>,
    mut hotbar_vis: Query<&mut Visibility, With<BuildHotbar>>,
    mut slots: Query<(&BuildHotbarSlot, &Interaction, &mut ImageNode)>,
    mut counts: Query<(&BuildHotbarCount, &mut Text, &mut TextColor)>,
) {
    let Ok(mut vis) = hotbar_vis.single_mut() else { return };
    *vis = if build_mode.is_some() { Visibility::Visible } else { Visibility::Hidden };

    let Some(mode) = build_mode else { return };

    for (slot, interaction, mut img) in &mut slots {
        let item = placeables.0.get(slot.index).copied();
        let count = item.map(|id| inventory.count(id)).unwrap_or(0);
        let has_any = count > 0;
        let is_selected = slot.index == mode.selected;
        let is_hovered = matches!(interaction, Interaction::Hovered | Interaction::Pressed);

        img.color = if is_selected {
            Color::srgb(1.20, 1.05, 0.70)
        } else if is_hovered && has_any {
            Color::srgb(1.10, 1.05, 0.95)
        } else if has_any {
            Color::WHITE
        } else {
            Color::srgba(0.55, 0.50, 0.45, 0.85)
        };
    }

    for (count_comp, mut text, mut color) in &mut counts {
        let item = placeables.0.get(count_comp.index).copied();
        let count = item.map(|id| inventory.count(id)).unwrap_or(0);
        **text = count.to_string();
        *color = TextColor(if count > 0 { TEXT_BODY } else { TEXT_FAINT });
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

/// Click an inventory slot containing a placeable item to enter placing mode
/// without pressing B. Non-placeable items are ignored. Acts as both
/// "start placing" (when no BuildMode) and "switch selection" (when one
/// already exists).
fn handle_inventory_placeable_clicks(
    mut commands: Commands,
    mut build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    inventory: Res<Inventory>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    slots: Query<(&Interaction, &InventorySlot), Changed<Interaction>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (interaction, slot) in &slots {
        if !matches!(interaction, Interaction::Pressed) {
            continue;
        }
        if !placeables.0.contains(&slot.item) || inventory.count(slot.item) == 0 {
            continue;
        }
        crate::building::enter_placing_with(
            &mut commands,
            build_mode.as_deref_mut(),
            &placeables,
            &registry,
            &asset_server,
            &mut meshes,
            &mut materials,
            slot.item,
        );
        // Only one click per frame.
        break;
    }
}

fn handle_build_hotbar_clicks(
    build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    inventory: Res<Inventory>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    slots: Query<(&Interaction, &BuildHotbarSlot), Changed<Interaction>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut mode) = build_mode else { return };
    for (interaction, slot) in &slots {
        if !matches!(interaction, Interaction::Pressed) {
            continue;
        }
        let Some(item) = placeables.0.get(slot.index).copied() else { continue };
        if inventory.count(item) == 0 {
            continue;
        }
        if mode.selected == slot.index {
            continue;
        }
        mode.selected = slot.index;
        crate::building::refresh_build_preview(
            &mut commands,
            &mut mode,
            item,
            &registry,
            &asset_server,
            &mut meshes,
            &mut materials,
        );
    }
}
