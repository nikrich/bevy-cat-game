//! Decoration mode bottom hotbar -- mirrors `building::ui::draw_build_tool_hotbar`
//! in shape but renders DecorationTool buttons + selected-piece thumbnail
//! and gates on `DecorationMode` instead of `BuildMode`.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use leafwing_input_manager::prelude::ActionState;

use crate::edit::EditHistory;
use crate::input::{Action, CursorState};
use crate::items::ItemRegistry;

use super::{DecorationMode, DecorationTool};

// Reuse the building module's parchment style so the two modes feel like one game.
use crate::building::ui::{panel_frame, GOLD, GOLD_DIM, TEXT_DIM};

/// `1` / `2` / `3` swap the active decoration tool. Mirrors
/// `building::select_build_tool` for the structural side. Suppressed when
/// egui has keyboard focus (e.g. the catalog search field) so typing
/// numbers doesn't switch tools.
pub fn select_tool_hotkeys(
    action_state: Res<ActionState<Action>>,
    decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
) {
    let Some(mut mode) = decoration_mode else { return };
    if cursor.keyboard_over_ui {
        return;
    }
    let slots: [(Action, DecorationTool); 3] = [
        (Action::Hotbar1, DecorationTool::Place),
        (Action::Hotbar2, DecorationTool::Move),
        (Action::Hotbar3, DecorationTool::Remove),
    ];
    for (action, tool) in slots {
        if action_state.just_pressed(&action) {
            mode.tool = tool;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_decoration_hotbar(
    mut contexts: EguiContexts,
    decoration_mode: Option<Res<DecorationMode>>,
    placeables: Res<crate::building::PlaceableItems>,
    registry: Res<ItemRegistry>,
    history: Res<EditHistory>,
) -> Result {
    let Some(mode) = decoration_mode else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let can_undo = history.can_undo();
    let can_redo = history.can_redo();

    egui::Window::new("decoration_hotbar")
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -16.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(panel_frame())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let tools = [
                    (1, "Place", DecorationTool::Place),
                    (2, "Move", DecorationTool::Move),
                    (3, "Remove", DecorationTool::Remove),
                ];
                for (digit, name, tool) in tools {
                    let active = tool == mode.tool;
                    let label = format!("{}  {}", digit, name);
                    let text = if active {
                        egui::RichText::new(label).color(GOLD).strong()
                    } else {
                        egui::RichText::new(label).color(TEXT_DIM)
                    };
                    ui.add(egui::Label::new(text).selectable(false));
                }

                ui.separator();

                let item_label = placeables
                    .0
                    .get(mode.selected)
                    .and_then(|id| registry.get(*id))
                    .map(|d| d.display_name.as_str())
                    .unwrap_or("(none)");
                ui.colored_label(GOLD, format!("piece: {}", item_label));

                ui.separator();

                ui.colored_label(GOLD_DIM, "Ctrl+Z / Ctrl+Shift+Z  |  N to exit");

                ui.separator();

                let undo_text = egui::RichText::new("⟲ Undo")
                    .color(if can_undo { GOLD } else { TEXT_DIM });
                ui.add_enabled(can_undo, egui::Label::new(undo_text).selectable(false));
                let redo_text = egui::RichText::new("⟳ Redo")
                    .color(if can_redo { GOLD } else { TEXT_DIM });
                ui.add_enabled(can_redo, egui::Label::new(redo_text).selectable(false));
            });
        });

    Ok(())
}
