use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;
use leafwing_input_manager::prelude::ActionState;

pub mod catalog_ui;
pub mod hotbar_ui;
pub mod interior;
pub mod move_tool;
pub mod physics;
pub mod place_tool;
pub mod preview;
pub mod remove_tool;
pub mod rotation;

use crate::input::{Action, CursorState};

pub struct DecorationPlugin;

impl Plugin for DecorationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<move_tool::MoveCarry>();
        app.init_resource::<rotation::RotationHold>();
        app.add_systems(
            Update,
            (
                toggle_decoration_mode,
                interior::resolve_interior_spawns,
                preview::update_preview,
                place_tool::place_decoration,
                remove_tool::remove_decoration,
                (
                    move_tool::drop_decoration,
                    move_tool::pickup_decoration,
                    move_tool::carry_follow_cursor,
                )
                    .chain(),
                hotbar_ui::select_tool_hotkeys,
                rotation::rotate_decoration,
            ),
        );
        app.add_systems(
            EguiPrimaryContextPass,
            (
                catalog_ui::draw_decoration_catalog,
                hotbar_ui::draw_decoration_hotbar,
            ),
        );
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
    placeables: Res<crate::building::PlaceableItems>,
    registry: Res<crate::items::ItemRegistry>,
) {
    // Allow N to override when crafting is open -- egui claims keyboard
    // for its own navigation while the menu shows, but the mode key should
    // still flip modes (mutual exclusion below closes crafting on entry).
    if cursor.keyboard_over_ui && !crafting.open {
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
            let selected = first_decoration_index(&placeables, &registry);
            commands.insert_resource(DecorationMode {
                selected,
                ..Default::default()
            });
        }
    }
}

/// Find the index (in `PlaceableItems`) of the first item tagged DECORATION
/// or FURNITURE. Falls back to 0 if no such item exists.
fn first_decoration_index(
    placeables: &crate::building::PlaceableItems,
    registry: &crate::items::ItemRegistry,
) -> usize {
    use crate::items::ItemTags;
    placeables
        .0
        .iter()
        .position(|id| {
            registry
                .get(*id)
                .map(|d| {
                    d.tags.contains(ItemTags::DECORATION)
                        || d.tags.contains(ItemTags::FURNITURE)
                })
                .unwrap_or(false)
        })
        .unwrap_or(0)
}
