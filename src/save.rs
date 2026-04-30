use bevy::prelude::*;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::building::{spawn_placed_building, PlacedBuilding};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::ItemRegistry;
use crate::memory::{CellMemory, Journal, JournalEntry, WorldMemory};
use crate::player::Player;
use crate::world::biome::Biome;
use crate::world::chunks::ChunkManager;
use crate::world::terrain::Terrain;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveTimer>()
            .add_systems(
                Startup,
                load_game.after(crate::items::registry::seed_default_items),
            )
            .add_systems(Update, auto_save);
    }
}

#[derive(Resource)]
struct SaveTimer {
    timer: Timer,
}

impl Default for SaveTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(30.0, TimerMode::Repeating),
        }
    }
}

const SAVE_FILE: &str = "savegame.json";
const APP_DIR_NAME: &str = "Cat World";

/// Resolve the save directory. Steam Cloud syncs the platform-standard
/// per-user data dir, so we land there by default and let `--save-dir <path>`
/// override for tests / portable builds. macOS picks
/// `~/Library/Application Support/Cat World/`, Linux picks
/// `$XDG_DATA_HOME/Cat World/`, Windows picks
/// `%APPDATA%/Cat World/` — all writable locations Steam Cloud is happy with
/// (W0.12 / DEC-013).
fn save_dir() -> PathBuf {
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == "--save-dir" {
            if let Some(path) = args.next() {
                return PathBuf::from(path);
            }
        } else if let Some(path) = arg.strip_prefix("--save-dir=") {
            return PathBuf::from(path);
        }
    }
    if let Some(base) = BaseDirs::new() {
        return base.data_dir().join(APP_DIR_NAME);
    }
    PathBuf::from(".")
}

fn save_path() -> PathBuf {
    save_dir().join(SAVE_FILE)
}

#[derive(Serialize, Deserialize)]
struct SaveData {
    player: [f32; 3],
    /// Keys are item save_keys (e.g. "plank.oak", "log.pine").
    inventory: HashMap<String, u32>,
    buildings: Vec<BuildingSave>,
    /// World seed becomes part of the save in W0.13 (closes DEBT-004). Older
    /// saves without a seed default to the previous hardcoded value via
    /// `serde(default)` so they keep loading into the same world.
    #[serde(default = "default_seed")]
    seed: u32,
    /// Phase A substrate. Optional with `#[serde(default)]` so saves predating
    /// Phase A still load cleanly.
    #[serde(default)]
    world_memory: Vec<CellMemoryEntry>,
    #[serde(default)]
    journal: Vec<JournalEntry>,
    #[serde(default)]
    journal_next_id: u32,
    /// Phase 1 / W1.15: per-chunk vertex height edits relative to PCG.
    /// Stored as a flat list because JSON map keys must be strings, and
    /// IVec2-shaped keys would need a custom serializer.
    #[serde(default)]
    terrain_edits: Vec<ChunkEditsSave>,
    /// Phase 1 / W1.10 Paint: per-chunk vertex biome overrides relative to
    /// PCG. Same flat-list shape as `terrain_edits` for the same reason.
    #[serde(default)]
    biome_edits: Vec<ChunkBiomeEditsSave>,
}

#[derive(Serialize, Deserialize)]
struct ChunkEditsSave {
    cx: i32,
    cz: i32,
    edits: Vec<VertexEditSave>,
}

#[derive(Serialize, Deserialize)]
struct VertexEditSave {
    lx: u8,
    lz: u8,
    h: f32,
}

#[derive(Serialize, Deserialize)]
struct ChunkBiomeEditsSave {
    cx: i32,
    cz: i32,
    edits: Vec<VertexBiomeEditSave>,
}

#[derive(Serialize, Deserialize)]
struct VertexBiomeEditSave {
    lx: u8,
    lz: u8,
    biome: Biome,
}

fn default_seed() -> u32 {
    7
}

#[derive(Serialize, Deserialize)]
struct BuildingSave {
    /// Item save_key (e.g. "fence.oak").
    item: String,
    x: f32,
    y: f32,
    z: f32,
    rot: f32,
}

/// Flat row encoding for a single `WorldMemory` cell. We avoid serialising
/// `HashMap<IVec2, _>` directly because IVec2 isn't a string-shaped JSON key.
#[derive(Serialize, Deserialize)]
struct CellMemoryEntry {
    x: i32,
    z: i32,
    #[serde(flatten)]
    cell: CellMemory,
}

#[derive(Resource)]
pub struct LoadedPlayerPos(pub Vec3);

fn auto_save(
    mut save_timer: ResMut<SaveTimer>,
    time: Res<Time>,
    inventory: Res<Inventory>,
    registry: Res<ItemRegistry>,
    world_memory: Res<WorldMemory>,
    journal: Res<Journal>,
    chunks: Res<ChunkManager>,
    terrain: Res<Terrain>,
    player_query: Query<&Transform, With<Player>>,
    buildings: Query<(&PlacedBuilding, &Transform)>,
    action_state: Res<leafwing_input_manager::prelude::ActionState<crate::input::Action>>,
) {
    save_timer.timer.tick(time.delta());
    let manual_save = action_state.just_pressed(&crate::input::Action::Save);

    if !save_timer.timer.just_finished() && !manual_save {
        return;
    }

    let Ok(player_tf) = player_query.single() else { return };

    let mut inv_map = HashMap::new();
    for (id, count) in &inventory.items {
        if *count == 0 {
            continue;
        }
        if let Some(def) = registry.get(*id) {
            inv_map.insert(def.save_key.clone(), *count);
        }
    }

    let buildings_vec: Vec<BuildingSave> = buildings
        .iter()
        .filter_map(|(b, tf)| {
            let def = registry.get(b.item)?;
            Some(BuildingSave {
                item: def.save_key.clone(),
                x: tf.translation.x,
                y: tf.translation.y,
                z: tf.translation.z,
                rot: tf.rotation.to_euler(EulerRot::YXZ).0,
            })
        })
        .collect();

    let world_memory_vec: Vec<CellMemoryEntry> = world_memory
        .cells
        .iter()
        .map(|(coord, cell)| CellMemoryEntry {
            x: coord.x,
            z: coord.y,
            cell: cell.clone(),
        })
        .collect();

    let terrain_edits: Vec<ChunkEditsSave> = terrain
        .edits
        .iter()
        .filter(|(_, vmap)| !vmap.is_empty())
        .map(|(coord, vmap)| ChunkEditsSave {
            cx: coord.0,
            cz: coord.1,
            edits: vmap
                .iter()
                .map(|(&(lx, lz), &h)| VertexEditSave { lx, lz, h })
                .collect(),
        })
        .collect();

    let biome_edits: Vec<ChunkBiomeEditsSave> = terrain
        .biome_edits
        .iter()
        .filter(|(_, vmap)| !vmap.is_empty())
        .map(|(coord, vmap)| ChunkBiomeEditsSave {
            cx: coord.0,
            cz: coord.1,
            edits: vmap
                .iter()
                .map(|(&(lx, lz), &biome)| VertexBiomeEditSave { lx, lz, biome })
                .collect(),
        })
        .collect();

    let data = SaveData {
        player: [
            player_tf.translation.x,
            player_tf.translation.y,
            player_tf.translation.z,
        ],
        inventory: inv_map,
        buildings: buildings_vec,
        seed: chunks.seed,
        world_memory: world_memory_vec,
        journal: journal.entries.clone(),
        journal_next_id: journal.next_id,
        terrain_edits,
        biome_edits,
    };

    let path = save_path();
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            warn!("Failed to create save dir {}: {}", parent.display(), e);
            return;
        }
    }

    match serde_json::to_string_pretty(&data) {
        Ok(json) => {
            if let Err(e) = fs::write(&path, json) {
                warn!("Failed to save to {}: {}", path.display(), e);
            } else if manual_save {
                info!("Game saved to {}", path.display());
            }
        }
        Err(e) => warn!("Save serialization failed: {}", e),
    }
}

fn load_game(
    mut commands: Commands,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut chunks: ResMut<ChunkManager>,
    mut terrain: ResMut<Terrain>,
    mut inventory: ResMut<Inventory>,
    mut world_memory: ResMut<WorldMemory>,
    mut journal: ResMut<Journal>,
    mut inv_events: MessageWriter<InventoryChanged>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let path = save_path();
    let Ok(data) = fs::read_to_string(&path) else {
        info!("No save file found, starting fresh");
        return;
    };

    info!("Loading save from {}", path.display());

    let save: SaveData = match serde_json::from_str(&data) {
        Ok(s) => s,
        Err(e) => {
            warn!("Save file failed to parse, starting fresh: {}", e);
            return;
        }
    };

    commands.insert_resource(LoadedPlayerPos(Vec3::from(save.player)));
    chunks.seed = save.seed;

    // Phase A substrate restore.
    for entry in save.world_memory {
        world_memory
            .cells
            .insert(IVec2::new(entry.x, entry.z), entry.cell);
    }
    journal.entries = save.journal;
    journal.next_id = save.journal_next_id;

    // Terrain edits restore. The chunks themselves haven't loaded yet
    // (Startup runs before Update), so this just primes the overlay; each
    // chunk picks up its edits when `Terrain::generate_chunk` runs in
    // `load_nearby_chunks`.
    for chunk_save in save.terrain_edits {
        let mut vmap: HashMap<(u8, u8), f32> = HashMap::new();
        for v in chunk_save.edits {
            vmap.insert((v.lx, v.lz), v.h);
        }
        if !vmap.is_empty() {
            terrain
                .edits
                .insert((chunk_save.cx, chunk_save.cz), vmap);
        }
    }
    for chunk_save in save.biome_edits {
        let mut vmap: HashMap<(u8, u8), Biome> = HashMap::new();
        for v in chunk_save.edits {
            vmap.insert((v.lx, v.lz), v.biome);
        }
        if !vmap.is_empty() {
            terrain
                .biome_edits
                .insert((chunk_save.cx, chunk_save.cz), vmap);
        }
    }

    for (key, count) in save.inventory {
        if count == 0 {
            continue;
        }
        let Some(id) = registry.lookup_save_key(&key) else {
            warn!("Inventory key not in registry: {key}");
            continue;
        };
        inventory.add(id, count);
        inv_events.write(InventoryChanged { item: id, new_count: count });
    }

    for b in save.buildings {
        let Some(id) = registry.lookup_save_key(&b.item) else {
            warn!("Building key not in registry: {}", b.item);
            continue;
        };
        let _ = spawn_placed_building(
            &mut commands,
            &registry,
            &asset_server,
            &mut meshes,
            &mut materials,
            id,
            Transform::from_xyz(b.x, b.y, b.z).with_rotation(Quat::from_rotation_y(b.rot)),
        );
    }
}
