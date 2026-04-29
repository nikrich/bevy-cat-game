use bevy::prelude::*;
use std::collections::HashMap;

use super::biome::WorldNoise;
use super::terrain::{Terrain, CHUNK_CELLS};

/// Cells per chunk side, re-exported as `CHUNK_SIZE` so callers that loop
/// over chunk-local cells keep reading naturally.
pub const CHUNK_SIZE: i32 = CHUNK_CELLS;
const RENDER_DISTANCE: i32 = 2;
const WORLD_SEED: u32 = 7;
/// Cap on chunk *data* generated per frame. The mesh + collider build for
/// each newly-loaded chunk is paced separately by `regenerate_dirty_chunks`
/// (see W1.2 in the Phase 1 spec).
const LOAD_BUDGET_PER_FRAME: usize = 4;

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

#[derive(Message)]
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

/// For every chunk coord within `RENDER_DISTANCE` that isn't loaded yet,
/// generate its terrain data into [`Terrain`] and spawn a chunk entity at
/// the chunk's NW corner. The entity starts mesh-less; the regen system
/// adds `Mesh3d` + the heightfield collider next.
pub fn load_nearby_chunks(
    mut commands: Commands,
    mut chunk_manager: ResMut<ChunkManager>,
    mut terrain: ResMut<Terrain>,
    noise: Res<WorldNoise>,
    mut chunk_events: MessageWriter<ChunkLoaded>,
) {
    let (cx, cz) = chunk_manager.player_chunk;

    let mut to_load = Vec::new();
    for dx in -RENDER_DISTANCE..=RENDER_DISTANCE {
        for dz in -RENDER_DISTANCE..=RENDER_DISTANCE {
            let coord = (cx + dx, cz + dz);
            if !chunk_manager.loaded.contains_key(&coord) {
                to_load.push(coord);
            }
        }
    }

    for coord in to_load.into_iter().take(LOAD_BUDGET_PER_FRAME) {
        terrain.generate_chunk(coord, &noise);

        let world_x = (coord.0 * CHUNK_SIZE) as f32;
        let world_z = (coord.1 * CHUNK_SIZE) as f32;
        let entity = commands
            .spawn((
                Chunk {
                    x: coord.0,
                    z: coord.1,
                },
                Transform::from_xyz(world_x, 0.0, world_z),
                Visibility::default(),
            ))
            .id();

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
    mut terrain: ResMut<Terrain>,
) {
    let (cx, cz) = chunk_manager.player_chunk;
    let unload_distance = RENDER_DISTANCE + 2;

    let to_unload: Vec<(i32, i32)> = chunk_manager
        .loaded
        .keys()
        .filter(|(x, z)| (x - cx).abs() > unload_distance || (z - cz).abs() > unload_distance)
        .copied()
        .collect();

    for coord in to_unload {
        if let Some(entity) = chunk_manager.loaded.remove(&coord) {
            commands.entity(entity).despawn();
        }
        terrain.unload_chunk(coord);
    }
}
