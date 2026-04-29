use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::building::{spawn_placed_building, PlacedBuilding};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::ItemRegistry;
use crate::player::Player;

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

fn save_path() -> PathBuf {
    PathBuf::from(SAVE_FILE)
}

#[derive(Serialize, Deserialize)]
struct SaveData {
    player: [f32; 3],
    /// Keys are item save_keys (e.g. "plank.oak", "log.pine").
    inventory: HashMap<String, u32>,
    buildings: Vec<BuildingSave>,
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

#[derive(Resource)]
pub struct LoadedPlayerPos(pub Vec3);

/// Translate the pre-registry ItemKind variant names ("Wood", "Plank", etc.)
/// to the new save_key scheme. Returns None for unknown legacy names.
fn legacy_item_to_save_key(legacy: &str) -> Option<&'static str> {
    Some(match legacy {
        "Wood" => "log.oak",
        "PineWood" => "log.pine",
        "Stone" => "stone.stone",
        "Flower" => "flower.flower",
        "Mushroom" => "mushroom.mushroom",
        "Bush" => "bush.bush",
        "Cactus" => "cactus.cactus",
        "Plank" => "plank.oak",
        "StoneBrick" => "brick.stone",
        "Fence" => "fence.oak",
        "Bench" => "bench.oak",
        "Lantern" => "lantern.stone",
        "FlowerPot" => "flowerpot.stone",
        "Stew" => "stew.none",
        "Wreath" => "wreath.none",
        _ => return None,
    })
}

/// Returns the canonical save_key for whatever the file key was -- either
/// already a registry save_key, or a legacy ItemKind variant name we still
/// understand.
fn canonical_save_key(raw: &str) -> Option<String> {
    if raw.contains('.') {
        Some(raw.to_string())
    } else {
        legacy_item_to_save_key(raw).map(|s| s.to_string())
    }
}

fn auto_save(
    mut save_timer: ResMut<SaveTimer>,
    time: Res<Time>,
    inventory: Res<Inventory>,
    registry: Res<ItemRegistry>,
    player_query: Query<&Transform, With<Player>>,
    buildings: Query<(&PlacedBuilding, &Transform)>,
    input: Res<crate::input::GameInput>,
) {
    save_timer.timer.tick(time.delta());
    let manual_save = input.save;

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

    let data = SaveData {
        player: [
            player_tf.translation.x,
            player_tf.translation.y,
            player_tf.translation.z,
        ],
        inventory: inv_map,
        buildings: buildings_vec,
    };

    match serde_json::to_string_pretty(&data) {
        Ok(json) => {
            if let Err(e) = fs::write(save_path(), json) {
                warn!("Failed to save: {}", e);
            } else if manual_save {
                info!("Game saved!");
            }
        }
        Err(e) => warn!("Save serialization failed: {}", e),
    }
}

fn load_game(
    mut commands: Commands,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: EventWriter<InventoryChanged>,
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

    for (raw_key, count) in save.inventory {
        if count == 0 {
            continue;
        }
        let Some(key) = canonical_save_key(&raw_key) else {
            warn!("Unknown inventory key in save: {raw_key}");
            continue;
        };
        let Some(id) = registry.lookup_save_key(&key) else {
            warn!("Inventory key not in registry: {key}");
            continue;
        };
        inventory.add(id, count);
        inv_events.write(InventoryChanged { item: id, new_count: count });
    }

    for b in save.buildings {
        let Some(key) = canonical_save_key(&b.item) else {
            warn!("Unknown building key in save: {}", b.item);
            continue;
        };
        let Some(id) = registry.lookup_save_key(&key) else {
            warn!("Building key not in registry: {key}");
            continue;
        };
        spawn_placed_building(
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
