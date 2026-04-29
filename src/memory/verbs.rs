//! Phase B: cat-specific verbs that leave traces in `WorldMemory` and the
//! `Journal`. The actions are deliberately small -- the cat sits, looks,
//! marks, sneaks -- and their value is the residue they leave behind for
//! later phases (animal-AI biases, the journal book, warmth-aware ambience).

use bevy::prelude::*;

use crate::animals::Animal;
use crate::building::PlacedBuilding;
use leafwing_input_manager::prelude::ActionState;

use crate::crafting::CraftingState;
use crate::input::{iso_movement, Action};
use crate::items::ItemRegistry;
use crate::player::Player;
use crate::world::biome::{Biome, WorldNoise};
use crate::world::props::{PropCollision, PropKind};

use super::{world_to_cell, Journal, JournalKind, WorldMemory};

const NAP_HOLD_SECS: f32 = 2.5;
const MARK_HOLD_SECS: f32 = 1.5;
const EXAMINE_REACH: f32 = 2.0;
const MOVING_THRESHOLD_SQ: f32 = 0.05;

/// Per-frame state for the hold-to-act verbs. UI can read this to draw a
/// progress ring.
#[derive(Resource, Default, Debug)]
pub struct CatVerbState {
    pub nap_progress: f32,
    pub mark_progress: f32,
    pub napping_cell: Option<IVec2>,
    pub marking_cell: Option<IVec2>,
}

impl CatVerbState {
    pub fn nap_fraction(&self) -> f32 {
        (self.nap_progress / NAP_HOLD_SECS).clamp(0.0, 1.0)
    }

    pub fn mark_fraction(&self) -> f32 {
        (self.mark_progress / MARK_HOLD_SECS).clamp(0.0, 1.0)
    }
}

pub fn register(app: &mut App) {
    app.init_resource::<CatVerbState>()
        .add_systems(Update, (nap_system, examine_system, mark_system));
}

fn verbs_blocked(crafting: &CraftingState) -> bool {
    crafting.open
}

fn nap_system(
    time: Res<Time>,
    action_state: Res<ActionState<Action>>,
    crafting: Res<CraftingState>,
    mut state: ResMut<CatVerbState>,
    mut memory: ResMut<WorldMemory>,
    mut journal: ResMut<Journal>,
    noise: Res<WorldNoise>,
    player_q: Query<&Transform, With<Player>>,
) {
    let Ok(tf) = player_q.single() else { return };
    let cell = world_to_cell(tf.translation);
    let moving = iso_movement(&action_state).length_squared() > MOVING_THRESHOLD_SQ;

    let cancel = verbs_blocked(&crafting)
        || !action_state.pressed(&Action::Nap)
        || moving
        || state.napping_cell.map_or(false, |c| c != cell);

    if cancel {
        state.nap_progress = 0.0;
        state.napping_cell = None;
        return;
    }

    if state.napping_cell.is_none() {
        state.napping_cell = Some(cell);
    }
    state.nap_progress += time.delta_secs();
    if state.nap_progress < NAP_HOLD_SECS {
        return;
    }

    state.nap_progress = 0.0;
    state.napping_cell = None;

    let entry = memory.cells.entry(cell).or_default();
    entry.slept_here = entry.slept_here.saturating_add(1);
    entry.warmth = (entry.warmth + 0.25).min(1.0);

    let biome = noise
        .sample(tf.translation.x as f64, tf.translation.z as f64)
        .biome;
    let body = format!("Curled up in the {}.", biome_blurb(biome));
    info!("[cat] napped at {cell:?} -- {body}");
    let id = journal.add(0, JournalKind::Sleep, body, Some(cell));
    entry.notes.push(id);
}

fn examine_system(
    action_state: Res<ActionState<Action>>,
    crafting: Res<CraftingState>,
    mut memory: ResMut<WorldMemory>,
    mut journal: ResMut<Journal>,
    noise: Res<WorldNoise>,
    registry: Res<ItemRegistry>,
    player_q: Query<&Transform, With<Player>>,
    props: Query<(&GlobalTransform, &PropCollision, &PropKind)>,
    animals: Query<(&GlobalTransform, &Animal)>,
    buildings: Query<(&GlobalTransform, &PlacedBuilding)>,
) {
    if !action_state.just_pressed(&Action::Examine) || verbs_blocked(&crafting) {
        return;
    }
    let Ok(tf) = player_q.single() else { return };
    let player_pos = tf.translation;

    let mut best: Option<(f32, String)> = None;

    for (gt, _animal) in &animals {
        let pos = gt.translation();
        let dx = pos.x - player_pos.x;
        let dz = pos.z - player_pos.z;
        let d2 = dx * dx + dz * dz;
        // Animals get a slightly wider examine reach so the cat can take note
        // of a creature mid-flee rather than only when shoulder-to-shoulder.
        let reach = EXAMINE_REACH + 1.0;
        if d2 > reach * reach {
            continue;
        }
        let label = animal_label(&_animal.kind);
        if best.as_ref().map_or(true, |(b, _)| d2 < *b) {
            best = Some((d2, label.to_string()));
        }
    }

    for (gt, _col, kind) in &props {
        let pos = gt.translation();
        let dx = pos.x - player_pos.x;
        let dz = pos.z - player_pos.z;
        let d2 = dx * dx + dz * dz;
        if d2 > EXAMINE_REACH * EXAMINE_REACH {
            continue;
        }
        if best.as_ref().map_or(true, |(b, _)| d2 < *b) {
            best = Some((d2, prop_label(kind).to_string()));
        }
    }

    for (gt, b) in &buildings {
        let pos = gt.translation();
        let dx = pos.x - player_pos.x;
        let dz = pos.z - player_pos.z;
        let d2 = dx * dx + dz * dz;
        if d2 > EXAMINE_REACH * EXAMINE_REACH {
            continue;
        }
        let label = registry
            .get(b.item)
            .map(|d| d.display_name.clone())
            .unwrap_or_else(|| "something I built".into());
        if best.as_ref().map_or(true, |(d, _)| d2 < *d) {
            best = Some((d2, label));
        }
    }

    let cell = world_to_cell(player_pos);
    let body = if let Some((_, label)) = best {
        format!("Looked closely at the {label}.")
    } else {
        let biome = noise
            .sample(player_pos.x as f64, player_pos.z as f64)
            .biome;
        format!("Studied the {} for a quiet moment.", biome_blurb(biome))
    };

    let entry = memory.cells.entry(cell).or_default();
    entry.warmth = (entry.warmth + 0.05).min(1.0);
    info!("[cat] examined at {cell:?} -- {body}");
    let id = journal.add(0, JournalKind::Examine, body, Some(cell));
    entry.notes.push(id);
}

fn mark_system(
    time: Res<Time>,
    action_state: Res<ActionState<Action>>,
    crafting: Res<CraftingState>,
    mut state: ResMut<CatVerbState>,
    mut memory: ResMut<WorldMemory>,
    mut journal: ResMut<Journal>,
    player_q: Query<&Transform, With<Player>>,
) {
    let Ok(tf) = player_q.single() else { return };
    let cell = world_to_cell(tf.translation);
    let moving = iso_movement(&action_state).length_squared() > MOVING_THRESHOLD_SQ;

    let cancel = verbs_blocked(&crafting)
        || !action_state.pressed(&Action::Mark)
        || moving
        || state.marking_cell.map_or(false, |c| c != cell);

    if cancel {
        state.mark_progress = 0.0;
        state.marking_cell = None;
        return;
    }

    if state.marking_cell.is_none() {
        state.marking_cell = Some(cell);
    }
    state.mark_progress += time.delta_secs();
    if state.mark_progress < MARK_HOLD_SECS {
        return;
    }

    state.mark_progress = 0.0;
    state.marking_cell = None;

    let entry = memory.cells.entry(cell).or_default();
    if entry.marked {
        return; // Already marked: don't accrete duplicate journal lines.
    }
    entry.marked = true;
    entry.warmth = (entry.warmth + 0.5).min(1.0);
    info!("[cat] marked {cell:?} as mine");
    let id = journal.add(
        0,
        JournalKind::Examine,
        "Marked this place as mine.",
        Some(cell),
    );
    entry.notes.push(id);
}

fn biome_blurb(biome: Biome) -> &'static str {
    match biome {
        Biome::Ocean => "shallows",
        Biome::Beach => "warm sand",
        Biome::Desert => "dry sand",
        Biome::Grassland => "grassland",
        Biome::Meadow => "meadow",
        Biome::Forest => "forest",
        Biome::Taiga => "pine wood",
        Biome::Tundra => "tundra",
        Biome::Snow => "snow",
        Biome::Mountain => "stones",
    }
}

fn prop_label(kind: &PropKind) -> &'static str {
    match kind {
        PropKind::Tree => "oak tree",
        PropKind::PineTree => "pine tree",
        PropKind::Cactus => "cactus",
        PropKind::Rock => "rock",
        PropKind::Boulder => "boulder",
        PropKind::Flower => "flower",
        PropKind::Bush => "bush",
        PropKind::Mushroom => "mushroom",
        PropKind::DeadBush => "dead bush",
        PropKind::IceRock => "frosted rock",
        PropKind::TundraGrass => "tundra grass",
    }
}

fn animal_label(kind: &crate::animals::AnimalKind) -> &'static str {
    use crate::animals::AnimalKind;
    match kind {
        AnimalKind::Rabbit => "rabbit",
        AnimalKind::Fox => "fox",
        AnimalKind::Deer => "deer",
        AnimalKind::Penguin => "penguin",
        AnimalKind::Lizard => "lizard",
    }
}
