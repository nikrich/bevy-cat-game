//! Top-level game state machine. Closes DEBT-003 / DEBT-011 (DEC-014).
//!
//! `GameState` partitions runtime: assets load in `Loading`, the player picks
//! a save in `MainMenu`, gameplay runs in `Playing`, and `Paused` halts the
//! gameplay tick without unloading the world. `BuildState` is a sub-state of
//! `Playing` — when the user opens the build menu the engine flips to
//! `Building` so we can scope build-mode UI/preview entities with
//! `DespawnOnExit`.
//!
//! Until W0.9 (asset_loader) lands the `Loading` state has no real work to
//! do, so a one-shot transition flips `Loading -> Playing` on the first
//! frame. That keeps the state graph spec-shaped without a real loading
//! screen.

use bevy::prelude::*;

use crate::building::BuildMode;
use crate::crafting::CraftingState;

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
            .add_systems(OnEnter(GameState::Loading), bootstrap_into_playing)
            .add_systems(OnEnter(GameState::Paused), pause_world_clock)
            .add_systems(OnExit(GameState::Paused), resume_world_clock)
            .add_systems(
                Update,
                toggle_pause.run_if(
                    in_state(GameState::Playing).or(in_state(GameState::Paused)),
                ),
            );
    }
}

/// Until the real loading screen exists, treat boot as instant: the moment
/// `Loading` is entered, queue a transition into `Playing`. Once W0.9 lands
/// this is replaced by the asset_loader transition.
fn bootstrap_into_playing(mut next: ResMut<NextState<GameState>>) {
    next.set(GameState::Playing);
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
