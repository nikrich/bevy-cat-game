//! Brush hotbar readout (Phase 1 / W1.14).
//!
//! Tiny egui panel anchored to the bottom-centre of the screen that shows
//! the active brush, current radius, and (when Paint is selected) the
//! biome the brush will apply. Hidden when edit mode is off so it doesn't
//! fight the HUD or build hotbar.
//!
//! Intentionally read-only: the actual key bindings (Hotbar1..5, scroll,
//! `[` / `]`) live in `world::edit`. This panel exists so the player can
//! see what those bindings did.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use super::edit::{paint_biome_label, BrushTool, EditMode};

const PARCHMENT: egui::Color32 = egui::Color32::from_rgb(54, 38, 24);
const GOLD: egui::Color32 = egui::Color32::from_rgb(220, 168, 76);
const GOLD_DIM: egui::Color32 = egui::Color32::from_rgb(140, 105, 50);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(172, 158, 130);

pub fn register(app: &mut App) {
    app.add_systems(EguiPrimaryContextPass, draw_brush_hotbar);
}

fn panel_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(PARCHMENT)
        .stroke(egui::Stroke::new(2.0, GOLD))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .corner_radius(egui::CornerRadius::same(6))
}

fn draw_brush_hotbar(
    mut contexts: EguiContexts,
    edit_mode: Res<EditMode>,
) -> Result {
    if !edit_mode.active {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;

    egui::Window::new("brush_hotbar")
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -16.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(panel_frame())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, brush) in [
                    BrushTool::Raise,
                    BrushTool::Lower,
                    BrushTool::Flatten,
                    BrushTool::Smooth,
                    BrushTool::Paint,
                ]
                .into_iter()
                .enumerate()
                {
                    let active = brush == edit_mode.brush;
                    let label = format!("{}  {}", i + 1, brush.label());
                    let text = if active {
                        egui::RichText::new(label).color(GOLD).strong()
                    } else {
                        egui::RichText::new(label).color(TEXT_DIM)
                    };
                    ui.add(egui::Label::new(text).selectable(false));
                }
                ui.separator();
                ui.colored_label(
                    GOLD_DIM,
                    format!("radius {:.1}m", edit_mode.radius),
                );
                if edit_mode.brush == BrushTool::Paint {
                    ui.separator();
                    ui.colored_label(
                        GOLD,
                        format!("paint: {}", paint_biome_label(edit_mode.paint_biome())),
                    );
                    ui.colored_label(GOLD_DIM, "[ / ]");
                }
            });
        });

    Ok(())
}
