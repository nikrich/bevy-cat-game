use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use leafwing_input_manager::prelude::ActionState;

pub mod catalog_ui;
pub mod hotbar_ui;
pub mod interior;
pub mod move_tool;
pub mod placement;

use crate::input::{Action, CursorState};

pub struct DecorationPlugin;

impl Plugin for DecorationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                toggle_decoration_mode,
                interior::resolve_interior_spawns,
            ),
        );
        app.add_systems(EguiPrimaryContextPass, catalog_ui::draw_decoration_catalog);
    }
}

#[derive(Resource, Default)]
pub struct DecorationMode {
    pub tool: DecorationTool,
    pub selected: usize,
    pub rotation_radians: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DecorationTool {
    #[default]
    Place,
    Move,
    Remove,
}

fn toggle_decoration_mode(
    mut commands: Commands,
    action_state: Res<ActionState<Action>>,
    decoration_mode: Option<Res<DecorationMode>>,
    cursor: Res<CursorState>,
    build_mode: Option<ResMut<crate::building::BuildMode>>,
    mut history: ResMut<crate::edit::EditHistory>,
    mut edit_mode: ResMut<crate::world::edit::EditMode>,
    mut crafting: ResMut<crate::crafting::CraftingState>,
) {
    if cursor.keyboard_over_ui {
        return;
    }
    if !action_state.just_pressed(&Action::ToggleDecoration) {
        return;
    }
    match decoration_mode {
        Some(_) => commands.remove_resource::<DecorationMode>(),
        None => {
            // Mutual exclusion: entering decoration mode exits all other modes.
            if let Some(mut bm) = build_mode {
                crate::building::exit_build_mode(&mut commands, &mut bm, &mut history);
            }
            edit_mode.active = false;
            crafting.open = false;
            commands.insert_resource(DecorationMode::default());
        }
    }
}
