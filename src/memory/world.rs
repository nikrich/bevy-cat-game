use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Per-cell state describing the player's relationship with a tile of the
/// world. Stored on a 1-meter integer grid keyed on `IVec2` (XZ).
///
/// Future systems read these to bias behaviour:
/// - particle density biases toward warmer cells (cosier-looking)
/// - animals develop routes through cells the player frequents
/// - the journal reads `notes` to fetch entries linked to a place
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CellMemory {
    pub visit_count: u32,
    /// Wall-clock seconds (Time::elapsed_secs_f64) at the most recent visit.
    pub last_visited_secs: f64,
    pub slept_here: u16,
    pub gathered_here: u16,
    /// Cat marked this spot as "theirs" via the mark verb (Phase B).
    pub marked: bool,
    /// 0..1 accumulated presence; decays slowly when the player is absent.
    pub warmth: f32,
    /// Journal entry ids associated with this cell.
    pub notes: Vec<u32>,
}

#[derive(Resource, Default, Debug)]
pub struct WorldMemory {
    pub cells: HashMap<IVec2, CellMemory>,
}

impl WorldMemory {
    pub fn cell(&self, cell: IVec2) -> Option<&CellMemory> {
        self.cells.get(&cell)
    }

    pub fn warmth_at(&self, cell: IVec2) -> f32 {
        self.cells.get(&cell).map(|c| c.warmth).unwrap_or(0.0)
    }
}

/// Convert a world-space position to its grid-cell key. Cells are 1m squares
/// centred on integer (x, z); the round behaviour matches placement snapping
/// so a tile-centre prop and the cell that owns it agree.
pub fn world_to_cell(pos: Vec3) -> IVec2 {
    IVec2::new(pos.x.round() as i32, pos.z.round() as i32)
}
