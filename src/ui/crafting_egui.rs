//! egui-based crafting menu (W0.6 / DEC-013 — pilot port off Bevy UI).
//!
//! The data layer is unchanged: `CraftingState` toggles open/closed,
//! `RecipeRegistry` is the source of recipes, and `CraftRequest` events
//! still drive `crafting::handle_craft_requests`. Only the rendering moved
//! out of Bevy UI nodes into an immediate-mode egui pass.
//!
//! Visual goal: keep the Spiritfarer-inspired warm parchment chrome from
//! DEBT-010. egui doesn't ship the painted PNG, so we approximate the look
//! with a custom `Frame` (warm dark fill, gold stroke, generous inner
//! margin) and a slightly larger heading. The painted-PNG version of the
//! menu is removed in `src/ui/mod.rs` (no more `CraftingMenu` Bevy UI tree
//! is spawned), so there's only one renderer.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::crafting::{CraftRequest, CraftingState, RecipeCategory, RecipeRegistry};
use crate::inventory::Inventory;
use crate::items::ItemRegistry;

pub fn register(app: &mut App) {
    app.add_systems(EguiPrimaryContextPass, draw_crafting_menu);
}

const PARCHMENT: egui::Color32 = egui::Color32::from_rgb(54, 38, 24);
const GOLD: egui::Color32 = egui::Color32::from_rgb(220, 168, 76);
const GOLD_DIM: egui::Color32 = egui::Color32::from_rgb(140, 105, 50);
const TEXT_BODY: egui::Color32 = egui::Color32::from_rgb(232, 214, 178);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(172, 158, 130);
const NEED_RED: egui::Color32 = egui::Color32::from_rgb(196, 96, 80);

fn panel_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(PARCHMENT)
        .stroke(egui::Stroke::new(2.0, GOLD))
        .inner_margin(egui::Margin::symmetric(20, 16))
        .corner_radius(egui::CornerRadius::same(6))
}

fn draw_crafting_menu(
    mut contexts: EguiContexts,
    mut state: ResMut<CraftingState>,
    inventory: Res<Inventory>,
    recipes: Res<RecipeRegistry>,
    item_registry: Res<ItemRegistry>,
    mut craft_events: MessageWriter<CraftRequest>,
) -> Result {
    if !state.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;

    egui::Window::new("Crafting")
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(panel_frame())
        .min_width(560.0)
        .max_width(620.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new("Crafting")
                        .color(GOLD)
                        .size(22.0)
                        .strong(),
                );
                ui.add_space(2.0);
                ui.colored_label(GOLD_DIM, "— select a recipe —");
            });
            ui.add_space(8.0);

            // Tab strip.
            ui.horizontal(|ui| {
                for &cat in RecipeCategory::ALL {
                    let selected = state.category == cat;
                    let label = if selected {
                        egui::RichText::new(cat.label()).color(GOLD).strong()
                    } else {
                        egui::RichText::new(cat.label()).color(TEXT_DIM)
                    };
                    if ui.selectable_label(selected, label).clicked() {
                        state.category = cat;
                        state.selected_in_category = 0;
                    }
                }
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    let cat_recipes: Vec<(usize, &crate::crafting::Recipe)> =
                        recipes.iter_category(state.category).collect();

                    if cat_recipes.is_empty() {
                        ui.colored_label(TEXT_DIM, "No recipes in this category yet.");
                        return;
                    }

                    for (visible_idx, (global_idx, recipe)) in cat_recipes.iter().enumerate() {
                        draw_recipe_row(
                            ui,
                            *global_idx,
                            visible_idx,
                            recipe,
                            &inventory,
                            &item_registry,
                            &mut state,
                            &mut craft_events,
                        );
                    }
                });

            ui.add_space(4.0);
            ui.separator();
            ui.vertical_centered(|ui| {
                ui.colored_label(
                    GOLD_DIM,
                    "Tab to close   ·   Click to craft",
                );
            });
        });

    Ok(())
}

fn draw_recipe_row(
    ui: &mut egui::Ui,
    global_idx: usize,
    visible_idx: usize,
    recipe: &crate::crafting::Recipe,
    inventory: &Inventory,
    item_registry: &ItemRegistry,
    state: &mut CraftingState,
    craft_events: &mut MessageWriter<CraftRequest>,
) {
    let result_name = item_registry
        .get(recipe.result)
        .map(|d| d.display_name.clone())
        .unwrap_or_else(|| "???".into());
    let can_craft = crate::crafting::recipe_satisfied(recipe, inventory);
    let is_selected = state.selected_in_category == visible_idx;

    let row_fill = if is_selected && can_craft {
        egui::Color32::from_rgb(78, 60, 36)
    } else if is_selected {
        egui::Color32::from_rgb(64, 44, 30)
    } else {
        egui::Color32::from_rgba_premultiplied(0, 0, 0, 32)
    };

    let response = egui::Frame::default()
        .fill(row_fill)
        .stroke(egui::Stroke::new(1.0, if is_selected { GOLD } else { GOLD_DIM }))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .corner_radius(egui::CornerRadius::same(4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::Vec2::new(180.0, 20.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.label(
                            egui::RichText::new(format!("{} ×{}", result_name, recipe.result_count))
                                .color(if can_craft { TEXT_BODY } else { TEXT_DIM })
                                .strong(),
                        );
                    },
                );

                ui.allocate_ui_with_layout(
                    egui::Vec2::new(280.0, 20.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        for (i, (item, need)) in recipe.ingredients.iter().enumerate() {
                            if i > 0 {
                                ui.colored_label(GOLD_DIM, "·");
                            }
                            let name = item_registry
                                .get(*item)
                                .map(|d| d.display_name.clone())
                                .unwrap_or_else(|| "???".into());
                            let have = inventory.count(*item);
                            let enough = have >= *need;
                            ui.colored_label(
                                if enough { TEXT_BODY } else { NEED_RED },
                                format!("{} {}/{}", name, have, need),
                            );
                        }
                    },
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn_text = if can_craft { "CRAFT" } else { "need more" };
                    let btn_color = if can_craft { GOLD } else { NEED_RED };
                    let btn = ui.add_enabled(
                        can_craft,
                        egui::Button::new(
                            egui::RichText::new(btn_text)
                                .color(if can_craft {
                                    egui::Color32::from_rgb(28, 18, 10)
                                } else {
                                    NEED_RED
                                })
                                .strong(),
                        )
                        .fill(if can_craft {
                            btn_color
                        } else {
                            egui::Color32::from_rgba_premultiplied(120, 50, 40, 60)
                        })
                        .min_size(egui::Vec2::new(80.0, 22.0)),
                    );
                    if btn.clicked() {
                        state.selected_in_category = visible_idx;
                        craft_events.write(CraftRequest { index: global_idx });
                    }
                });
            });
        })
        .response;

    if response.clicked() {
        state.selected_in_category = visible_idx;
    }
    ui.add_space(4.0);
}
