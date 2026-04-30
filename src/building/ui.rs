//! Build mode tool palette (mirrors `world::edit_egui` in shape).
//!
//! Bottom-centre egui panel showing the active `BuildTool`, hotkey labels,
//! and (for Place) the currently selected placeable plus the `[ / ]`
//! cycle hint. Hidden when build mode is off.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use super::history::{apply_redo, apply_undo, BuildHistory};
use super::{
    refresh_build_preview, BuildMode, BuildTool, PlaceableItems, PlacedBuilding,
};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{ItemRegistry, ItemTags};

const PARCHMENT: egui::Color32 = egui::Color32::from_rgb(54, 38, 24);
const GOLD: egui::Color32 = egui::Color32::from_rgb(220, 168, 76);
const GOLD_DIM: egui::Color32 = egui::Color32::from_rgb(140, 105, 50);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(172, 158, 130);

pub fn register(app: &mut App) {
    app.add_systems(
        EguiPrimaryContextPass,
        (draw_build_tool_hotbar, draw_decoration_catalog),
    );
}

fn panel_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(PARCHMENT)
        .stroke(egui::Stroke::new(2.0, GOLD))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .corner_radius(egui::CornerRadius::same(6))
}

#[allow(clippy::too_many_arguments)]
fn draw_build_tool_hotbar(
    mut contexts: EguiContexts,
    build_mode: Option<Res<BuildMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    mut history: ResMut<BuildHistory>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    placed_q: Query<(Entity, &Transform, &PlacedBuilding)>,
) -> Result {
    let Some(mode) = build_mode else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let can_undo = history.can_undo();
    let can_redo = history.can_redo();

    egui::Window::new("build_tool_hotbar")
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -16.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(panel_frame())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, &tool) in BuildTool::ALL.iter().enumerate() {
                    let active = tool == mode.tool;
                    let label = format!("{}  {}", i + 1, tool.label());
                    let text = if active {
                        egui::RichText::new(label).color(GOLD).strong()
                    } else {
                        egui::RichText::new(label).color(TEXT_DIM)
                    };
                    ui.add(egui::Label::new(text).selectable(false));
                }

                ui.separator();

                match mode.tool {
                    BuildTool::Place => {
                        let item_label = mode
                            .selected_item(&placeables)
                            .and_then(|id| registry.get(id))
                            .map(|d| d.display_name.as_str())
                            .unwrap_or("(none)");
                        ui.colored_label(GOLD, format!("piece: {}", item_label));
                        ui.colored_label(GOLD_DIM, "[ / ]   shift+click = line");
                    }
                    BuildTool::Remove => {
                        ui.colored_label(GOLD_DIM, "click a placed cube to remove");
                    }
                }

                ui.separator();

                let undo_btn = egui::Button::new(
                    egui::RichText::new("⟲ Undo").color(if can_undo { GOLD } else { TEXT_DIM }),
                );
                if ui.add_enabled(can_undo, undo_btn).clicked() {
                    apply_undo(
                        &mut history,
                        &mut commands,
                        &registry,
                        &asset_server,
                        &mut meshes,
                        &mut materials,
                        &mut inventory,
                        &mut inv_events,
                    );
                }
                let redo_btn = egui::Button::new(
                    egui::RichText::new("⟳ Redo").color(if can_redo { GOLD } else { TEXT_DIM }),
                );
                if ui.add_enabled(can_redo, redo_btn).clicked() {
                    apply_redo(
                        &mut history,
                        &mut commands,
                        &registry,
                        &asset_server,
                        &mut meshes,
                        &mut materials,
                        &mut inventory,
                        &mut inv_events,
                        &placed_q,
                    );
                }
                ui.colored_label(GOLD_DIM, "Ctrl+Z / Ctrl+Shift+Z");
            });
        });

    Ok(())
}

/// Right-side decoration catalog — DECORATION + FURNITURE placeables in
/// a clickable list. Selecting one switches BuildMode.tool to Place and
/// snaps mode.selected to that item. Always visible while in build mode
/// so the player can flip between cubes (via the bottom tool palette
/// + [/]/Q/E/scroll) and decorations (via this panel) without a chord.
#[allow(clippy::too_many_arguments)]
fn draw_decoration_catalog(
    mut contexts: EguiContexts,
    build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) -> Result {
    let Some(mut mode) = build_mode else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;

    egui::Window::new("build_decoration_catalog")
        .anchor(egui::Align2::RIGHT_TOP, [-16.0, 80.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(panel_frame())
        .show(ctx, |ui| {
            ui.colored_label(GOLD, "Decorations");
            ui.add_space(4.0);
            for (idx, item_id) in placeables.0.iter().enumerate() {
                let Some(def) = registry.get(*item_id) else { continue };
                let is_decor = def.tags.contains(ItemTags::DECORATION)
                    || def.tags.contains(ItemTags::FURNITURE);
                if !is_decor {
                    continue;
                }
                let active = idx == mode.selected && mode.tool == BuildTool::Place;
                let text = if active {
                    egui::RichText::new(&def.display_name).color(GOLD).strong()
                } else {
                    egui::RichText::new(&def.display_name).color(TEXT_DIM)
                };
                if ui.add(egui::Button::new(text).frame(false)).clicked() {
                    mode.tool = BuildTool::Place;
                    mode.selected = idx;
                    refresh_build_preview(
                        &mut commands,
                        &mut mode,
                        *item_id,
                        &registry,
                        &asset_server,
                        &mut meshes,
                        &mut materials,
                    );
                }
            }
        });

    Ok(())
}

