//! Headless smoke tests. First brick of DEBT-006 (W0.14).
//!
//! These tests prove the test harness can boot a Bevy `App` without a window
//! or GPU, tick the schedule, and exit cleanly. Game-internal modules cannot
//! be reached from `tests/` while the project is bin-only, so for now we
//! verify the runtime substrate (state machine, time, schedule) the game
//! depends on. When `src/lib.rs` is introduced (Phase 1+) these tests will
//! grow to cover plugin wiring directly.

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum SmokeState {
    #[default]
    Loading,
    Playing,
}

#[test]
fn app_boots_and_ticks_30_frames() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(StatesPlugin);
    app.init_state::<SmokeState>();

    for _ in 0..30 {
        app.update();
    }

    // If we got here, the schedule ran 30 times without panicking.
    assert!(app.world().contains_resource::<State<SmokeState>>());
}

#[test]
fn state_transition_resolves_in_one_tick() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(StatesPlugin);
    app.init_state::<SmokeState>();
    app.update();

    {
        let mut next = app.world_mut().resource_mut::<NextState<SmokeState>>();
        next.set(SmokeState::Playing);
    }
    app.update();

    assert_eq!(
        app.world().resource::<State<SmokeState>>().get(),
        &SmokeState::Playing
    );
}
