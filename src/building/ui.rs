//! Build mode tool palette (mirrors `world::edit_egui` in shape).
//!
//! Bottom-centre egui panel showing the active `BuildTool`, hotkey labels,
//! and (for Place) the currently selected placeable plus the `[ / ]`
//! cycle hint. Hidden when build mode is off.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::edit::{apply_redo, apply_undo, EditHistory};
use crate::edit::PlacedItem;
use super::{BuildMode, BuildTool, PlaceableItems};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{Form, InteriorCatalog, ItemRegistry};

pub(super) const PARCHMENT: egui::Color32 = egui::Color32::from_rgb(54, 38, 24);
pub(crate) const GOLD: egui::Color32 = egui::Color32::from_rgb(220, 168, 76);
pub(crate) const GOLD_DIM: egui::Color32 = egui::Color32::from_rgb(140, 105, 50);
pub(crate) const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(172, 158, 130);

pub fn register(app: &mut App) {
    app.add_systems(
        EguiPrimaryContextPass,
        draw_build_tool_hotbar,
    );
}

pub(crate) fn panel_frame() -> egui::Frame {
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
    mut history: ResMut<EditHistory>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    placed_q: Query<(Entity, &Transform, &PlacedItem)>,
    catalog: Res<InteriorCatalog>,
    mut indoor_settings: ResMut<crate::camera::occluder_fade::IndoorRevealSettings>,
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
                        // Resolve the currently-selected form so we can
                        // highlight the matching swatch below.
                        let active_form = mode
                            .selected_item(&placeables)
                            .and_then(|id| registry.get(id))
                            .map(|d| d.form);

                        // 6-swatch piece selector — one label per structural
                        // form, numbered 1-6 for discoverability.
                        const SWATCHES: &[(u8, &str, Form)] = &[
                            (1, "Wall",   Form::Wall),
                            (2, "Floor",  Form::Floor),
                            (3, "Door",   Form::Door),
                            (4, "Window", Form::Window),
                            (5, "Roof",   Form::Roof),
                            (6, "Fence",  Form::Fence),
                        ];
                        for (num, label, form) in SWATCHES {
                            let is_active = active_form.map(|f| f == *form).unwrap_or(false);
                            let text = format!("[{}] {}", num, label);
                            let rich = if is_active {
                                egui::RichText::new(text).color(GOLD).strong()
                            } else {
                                egui::RichText::new(text).color(TEXT_DIM)
                            };
                            ui.add(egui::Label::new(rich).selectable(false));
                        }
                        ui.colored_label(GOLD_DIM, "1-6 select  |  shift+click = line");
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
                        &catalog,
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
                        &catalog,
                    );
                }
                ui.colored_label(GOLD_DIM, "Ctrl+Z / Ctrl+Shift+Z");

                ui.separator();

                // Indoor reveal controls — let the player toggle the
                // ceiling-fade effect off (e.g. while admiring the
                // exterior) and tweak how see-through it is when on.
                ui.checkbox(&mut indoor_settings.enabled, "X-ray (X)");
                ui.add_enabled(
                    indoor_settings.enabled,
                    egui::Slider::new(&mut indoor_settings.alpha, 0.0..=1.0)
                        .show_value(false)
                        .text("α"),
                );
            });
        });

    Ok(())
}


