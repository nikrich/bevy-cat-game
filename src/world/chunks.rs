use bevy::prelude::*;
use std::collections::HashMap;

use super::terrain::spawn_chunk_terrain;

pub const CHUNK_SIZE: i32 = 16;
const RENDER_DISTANCE: i32 = 3;
const WORLD_SEED: u32 = 7;

#[derive(Component)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
}

#[derive(Resource)]
pub struct ChunkManager {
    pub loaded: HashMap<(i32, i32), Entity>,
    pub seed: u32,
    pub player_chunk: (i32, i32),
}

impl Default for ChunkManager {
    fn default() -> Self {
        Self {
            loaded: HashMap::new(),
            seed: WORLD_SEED,
            player_chunk: (0, 0),
        }
    }
}

#[derive(Event)]
pub struct ChunkLoaded {
    pub x: i32,
    pub z: i32,
    pub entity: Entity,
}

pub fn track_player_chunk(
    player_query: Query<&Transform, With<crate::player::Player>>,
    mut chunk_manager: ResMut<ChunkManager>,
) -> Result {
    let transform = player_query.single()?;
    let chunk_x = (transform.translation.x / CHUNK_SIZE as f32).floor() as i32;
    let chunk_z = (transform.translation.z / CHUNK_SIZE as f32).floor() as i32;
    chunk_manager.player_chunk = (chunk_x, chunk_z);
    Ok(())
}

pub fn load_nearby_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut chunk_events: EventWriter<ChunkLoaded>,
) {
    let (cx, cz) = chunk_manager.player_chunk;
    let seed = chunk_manager.seed;

    let mut to_load = Vec::new();

    for dx in -RENDER_DISTANCE..=RENDER_DISTANCE {
        for dz in -RENDER_DISTANCE..=RENDER_DISTANCE {
            let coord = (cx + dx, cz + dz);
            if !chunk_manager.loaded.contains_key(&coord) {
                to_load.push(coord);
            }
        }
    }

    // Limit chunks spawned per frame to avoid hitches
    for coord in to_load.into_iter().take(4) {
        let entity = spawn_chunk_terrain(
            &mut commands,
            &mut meshes,
            &mut materials,
            coord.0,
            coord.1,
            seed,
        );

        chunk_manager.loaded.insert(coord, entity);
        chunk_events.write(ChunkLoaded {
            x: coord.0,
            z: coord.1,
            entity,
        });
    }
}

pub fn unload_distant_chunks(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    let (cx, cz) = chunk_manager.player_chunk;
    let unload_distance = RENDER_DISTANCE + 2;

    let to_unload: Vec<(i32, i32)> = chunk_manager
        .loaded
        .keys()
        .filter(|(x, z)| {
            (x - cx).abs() > unload_distance || (z - cz).abs() > unload_distance
        })
        .copied()
        .collect();

    for coord in to_unload {
        if let Some(entity) = chunk_manager.loaded.remove(&coord) {
            commands.entity(entity).despawn();
        }
    }
}
