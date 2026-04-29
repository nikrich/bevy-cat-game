use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;

use crate::building::PlacedBuilding;
use crate::inventory::{Inventory, InventoryChanged, ItemKind};
use crate::player::Player;

pub struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveTimer>()
            .add_systems(Startup, load_game)
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

/// Simple JSON-compatible save format using manual serialization
/// (avoids adding serde dependency)
fn auto_save(
    mut save_timer: ResMut<SaveTimer>,
    time: Res<Time>,
    inventory: Res<Inventory>,
    player_query: Query<&Transform, With<Player>>,
    buildings: Query<(&PlacedBuilding, &Transform)>,
    input: Res<crate::input::GameInput>,
) {
    save_timer.timer.tick(time.delta());

    let manual_save = input.save;

    if !save_timer.timer.just_finished() && !manual_save {
        return;
    }

    let Ok(player_tf) = player_query.single() else {
        return;
    };

    let mut save_data = String::from("{\n");

    // Player position
    save_data.push_str(&format!(
        "  \"player\": [{:.2}, {:.2}, {:.2}],\n",
        player_tf.translation.x, player_tf.translation.y, player_tf.translation.z
    ));

    // Inventory
    save_data.push_str("  \"inventory\": {");
    let inv_entries: Vec<String> = inventory
        .items
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(item, count)| format!("\"{:?}\": {}", item, count))
        .collect();
    save_data.push_str(&inv_entries.join(", "));
    save_data.push_str("},\n");

    // Buildings
    save_data.push_str("  \"buildings\": [\n");
    let building_entries: Vec<String> = buildings
        .iter()
        .map(|(building, tf)| {
            format!(
                "    {{\"item\": \"{:?}\", \"x\": {:.2}, \"y\": {:.2}, \"z\": {:.2}, \"rot\": {:.4}}}",
                building.item,
                tf.translation.x,
                tf.translation.y,
                tf.translation.z,
                tf.rotation.to_euler(EulerRot::YXZ).0
            )
        })
        .collect();
    save_data.push_str(&building_entries.join(",\n"));
    save_data.push_str("\n  ]\n");

    save_data.push('}');

    if let Err(e) = fs::write(save_path(), &save_data) {
        warn!("Failed to save: {}", e);
    } else if manual_save {
        info!("Game saved!");
    }
}

fn load_game(
    mut commands: Commands,
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

    // Simple JSON parsing for our known format
    // Parse player position
    if let Some(player_str) = extract_json_array(&data, "player") {
        let coords: Vec<f32> = player_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        if coords.len() == 3 {
            // We'll set player position via a resource
            commands.insert_resource(LoadedPlayerPos(Vec3::new(coords[0], coords[1], coords[2])));
        }
    }

    // Parse inventory
    if let Some(inv_str) = extract_json_object(&data, "inventory") {
        for entry in inv_str.split(',') {
            let parts: Vec<&str> = entry.split(':').collect();
            if parts.len() == 2 {
                let key = parts[0].trim().trim_matches('"');
                let count: u32 = parts[1].trim().parse().unwrap_or(0);
                if let Some(item) = parse_item_kind(key) {
                    if count > 0 {
                        inventory.add(item, count);
                        inv_events.write(InventoryChanged {
                            item,
                            new_count: count,
                        });
                    }
                }
            }
        }
    }

    // Parse buildings
    if let Some(buildings_str) = extract_json_array_objects(&data, "buildings") {
        for building_str in buildings_str {
            let item_str = extract_field(&building_str, "item").unwrap_or_default();
            let x: f32 = extract_field(&building_str, "x")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let y: f32 = extract_field(&building_str, "y")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let z: f32 = extract_field(&building_str, "z")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let rot: f32 = extract_field(&building_str, "rot")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);

            if let Some(item) = parse_item_kind(&item_str) {
                let (mesh, color, scale) = crate::building::building_visual_pub(item);
                let mesh_handle = meshes.add(mesh);
                let mat_handle = materials.add(StandardMaterial {
                    base_color: color,
                    perceptual_roughness: 0.8,
                    ..default()
                });

                commands.spawn((
                    PlacedBuilding { item },
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(mat_handle),
                    Transform::from_xyz(x, y, z)
                        .with_rotation(Quat::from_rotation_y(rot))
                        .with_scale(scale),
                ));
            }
        }
    }
}

#[derive(Resource)]
pub struct LoadedPlayerPos(pub Vec3);

fn parse_item_kind(s: &str) -> Option<ItemKind> {
    match s {
        "Wood" => Some(ItemKind::Wood),
        "Stone" => Some(ItemKind::Stone),
        "Flower" => Some(ItemKind::Flower),
        "Mushroom" => Some(ItemKind::Mushroom),
        "Bush" => Some(ItemKind::Bush),
        "Cactus" => Some(ItemKind::Cactus),
        "PineWood" => Some(ItemKind::PineWood),
        "Plank" => Some(ItemKind::Plank),
        "StoneBrick" => Some(ItemKind::StoneBrick),
        "Fence" => Some(ItemKind::Fence),
        "Bench" => Some(ItemKind::Bench),
        "Lantern" => Some(ItemKind::Lantern),
        "FlowerPot" => Some(ItemKind::FlowerPot),
        "Stew" => Some(ItemKind::Stew),
        "Wreath" => Some(ItemKind::Wreath),
        _ => None,
    }
}

// Simple JSON helpers (avoiding serde dependency)
fn extract_json_array(data: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    let start = data.find(&pattern)? + pattern.len();
    let bracket_start = data[start..].find('[')? + start + 1;
    let bracket_end = data[bracket_start..].find(']')? + bracket_start;
    Some(data[bracket_start..bracket_end].to_string())
}

fn extract_json_object(data: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    let start = data.find(&pattern)? + pattern.len();
    let brace_start = data[start..].find('{')? + start + 1;
    let brace_end = data[brace_start..].find('}')? + brace_start;
    Some(data[brace_start..brace_end].to_string())
}

fn extract_json_array_objects(data: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("\"{}\":", key);
    let start = data.find(&pattern)? + pattern.len();
    let bracket_start = data[start..].find('[')? + start + 1;
    let bracket_end = data[bracket_start..].find(']')? + bracket_start;
    let inner = &data[bracket_start..bracket_end];

    let mut objects = Vec::new();
    let mut depth = 0;
    let mut obj_start = None;
    for (i, c) in inner.char_indices() {
        match c {
            '{' => {
                if depth == 0 {
                    obj_start = Some(i);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(start) = obj_start {
                        objects.push(inner[start + 1..i].to_string());
                    }
                }
            }
            _ => {}
        }
    }
    Some(objects)
}

fn extract_field(obj: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":", key);
    let start = obj.find(&pattern)? + pattern.len();
    let rest = obj[start..].trim();

    if rest.starts_with('"') {
        let end = rest[1..].find('"')? + 1;
        Some(rest[1..end].to_string())
    } else {
        let end = rest
            .find(|c: char| c == ',' || c == '}' || c == ']')
            .unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    }
}
