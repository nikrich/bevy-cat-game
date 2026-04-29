//! Top-level game state machine. Closes DEBT-003 / DEBT-011 (DEC-014).
//!
//! `GameState` partitions runtime: `bevy_asset_loader` runs in `Loading`
//! and waits for `UiAssets` to materialise before flipping to `MainMenu`,
//! the player picks a save in `MainMenu`, gameplay runs in `Playing`, and
//! `Paused` halts the gameplay tick without unloading the world.
//! `BuildState` is a sub-state of `Playing` — when the user opens the build
//! menu the engine flips to `Building` so we can scope build-mode UI/preview
//! entities with `DespawnOnExit`.
//!
//! The `Loading` and `MainMenu` screens are both rendered by egui (W0.6 +
//! W0.9 share the egui plumbing). `Loading` shows a centred "Loading…"
//! panel; `MainMenu` shows a "Start Game" / "Quit" pair. New Game / Load
//! Game / Settings split lands in a later phase when the save-slot UI is
//! designed.

use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_asset_loader::loading_state::{
    config::ConfigureLoadingState, LoadingState, LoadingStateAppExt,
};
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::building::BuildMode;
use crate::crafting::CraftingState;
use crate::ui::chrome::UiAssets;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    #[default]
    Loading,
    MainMenu,
    Playing,
    Paused,
}

#[derive(SubStates, Debug, Clone, PartialEq, Eq, Hash, Default)]
#[source(GameState = GameState::Playing)]
pub enum BuildState {
    #[default]
    Idle,
    Building,
}

pub struct StatePlugin;

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
            .add_sub_state::<BuildState>()
            .add_loading_state(
                LoadingState::new(GameState::Loading)
                    .continue_to_state(GameState::MainMenu)
                    .load_collection::<UiAssets>(),
            )
            .add_systems(OnEnter(GameState::Paused), pause_world_clock)
            .add_systems(OnExit(GameState::Paused), resume_world_clock)
            .add_systems(
                Update,
                toggle_pause.run_if(
                    in_state(GameState::Playing).or(in_state(GameState::Paused)),
                ),
            )
            // Egui screens for non-gameplay states. Both run inside the
            // primary egui context pass so they layer correctly with the
            // crafting menu (which also lives there).
            .add_systems(
                EguiPrimaryContextPass,
                draw_loading_screen.run_if(in_state(GameState::Loading)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                draw_main_menu.run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                EguiPrimaryContextPass,
                draw_pause_overlay.run_if(in_state(GameState::Paused)),
            );
    }
}

fn toggle_pause(
    keys: Res<ButtonInput<KeyCode>>,
    state: Res<State<GameState>>,
    mut next: ResMut<NextState<GameState>>,
    build_mode: Option<Res<BuildMode>>,
    crafting: Res<CraftingState>,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    // Esc is overloaded: in build mode it cancels the placement, in the
    // crafting menu it closes the menu. Only treat Esc as pause when the
    // gameplay surface is otherwise idle.
    if build_mode.is_some() || crafting.open {
        return;
    }
    match state.get() {
        GameState::Playing => next.set(GameState::Paused),
        GameState::Paused => next.set(GameState::Playing),
        _ => {}
    }
}

fn pause_world_clock(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn resume_world_clock(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

fn draw_loading_screen(mut contexts: EguiContexts) -> Result {
    let ctx = contexts.ctx_mut()?;
    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(egui::Color32::from_rgb(38, 28, 18)))
        .show(ctx, |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.label(
                        egui::RichText::new("Cat World")
                            .color(egui::Color32::from_rgb(220, 168, 76))
                            .size(48.0)
                            .strong(),
                    );
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("Loading…")
                            .color(egui::Color32::from_rgb(232, 214, 178))
                            .size(20.0),
                    );
                    ui.add_space(8.0);
                    ui.spinner();
                },
            );
        });
    Ok(())
}

fn draw_main_menu(
    mut contexts: EguiContexts,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    egui::CentralPanel::default()
        .frame(egui::Frame::default().fill(egui::Color32::from_rgb(38, 28, 18)))
        .show(ctx, |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.label(
                            egui::RichText::new("Cat World")
                                .color(egui::Color32::from_rgb(220, 168, 76))
                                .size(64.0)
                                .strong(),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("a peaceful crafting life")
                                .color(egui::Color32::from_rgb(172, 158, 130))
                                .size(16.0)
                                .italics(),
                        );
                        ui.add_space(48.0);

                        if menu_button(ui, "Start Game").clicked() {
                            next.set(GameState::Playing);
                        }
                        ui.add_space(8.0);
                        if menu_button(ui, "Quit").clicked() {
                            exit.write(AppExit::Success);
                        }
                    });
                },
            );
        });
    Ok(())
}

fn draw_pause_overlay(
    mut contexts: EguiContexts,
    mut next: ResMut<NextState<GameState>>,
    mut exit: MessageWriter<AppExit>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    egui::Area::new(egui::Id::new("pause_overlay"))
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            egui::Frame::default()
                .fill(egui::Color32::from_rgba_premultiplied(38, 28, 18, 230))
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(220, 168, 76)))
                .inner_margin(egui::Margin::symmetric(36, 28))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Paused")
                                .color(egui::Color32::from_rgb(220, 168, 76))
                                .size(28.0)
                                .strong(),
                        );
                        ui.add_space(16.0);
                        if menu_button(ui, "Resume").clicked() {
                            next.set(GameState::Playing);
                        }
                        ui.add_space(6.0);
                        if menu_button(ui, "Main Menu").clicked() {
                            next.set(GameState::MainMenu);
                        }
                        ui.add_space(6.0);
                        if menu_button(ui, "Quit").clicked() {
                            exit.write(AppExit::Success);
                        }
                        ui.add_space(8.0);
                        ui.colored_label(
                            egui::Color32::from_rgb(140, 105, 50),
                            "Esc to resume",
                        );
                    });
                });
        });
    Ok(())
}

fn menu_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .color(egui::Color32::from_rgb(28, 18, 10))
                .size(18.0)
                .strong(),
        )
        .fill(egui::Color32::from_rgb(220, 168, 76))
        .min_size(egui::Vec2::new(220.0, 40.0)),
    )
}
