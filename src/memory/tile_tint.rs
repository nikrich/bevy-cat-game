//! Phase C (revised): instead of floating amber motes, paint warm cells with
//! an emissive tint on the terrain tile itself. Reads exactly the same
//! `WorldMemory.warmth` signal -- napped, marked, and frequented tiles glow;
//! abandoned tiles fade as warmth decays.
//!
//! Implementation: tiles share materials by (biome, shade) within a chunk.
//! When a tile first becomes warm we clone its material into a per-tile
//! instance and store the original handle in `WarmTint` so we can revert
//! later. Once cloned, the per-tile material's emissive is mutated each
//! frame to track warmth -- no asset churn during the warm window.

use bevy::prelude::*;

use super::{world_to_cell, WorldMemory};
use crate::world::terrain::Tile;

/// Below this warmth the tint reverts entirely. Slightly higher than the
/// per-step bump (0.005) so casual walking does not light a trail.
const TINT_THRESHOLD: f32 = 0.04;
/// How quickly the tinted emissive eases toward its target each frame.
const TINT_LERP_SPEED: f32 = 4.0;
/// Only update / scan tiles within this radius of the player. Keeps the
/// per-frame work bounded as more chunks load.
const TINT_SCAN_RADIUS: f32 = 24.0;

/// Marker on tiles that currently own a cloned, mutable material. Stores the
/// original shared handle so we can swap back when the tile cools.
#[derive(Component)]
pub struct WarmTint {
    pub original: Handle<StandardMaterial>,
}

pub fn register(app: &mut App) {
    app.add_systems(Update, tint_warm_tiles);
}

fn tint_warm_tiles(
    mut commands: Commands,
    world_memory: Res<WorldMemory>,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_q: Query<&Transform, (With<crate::player::Player>, Without<Tile>)>,
    mut tiles: Query<
        (
            Entity,
            &GlobalTransform,
            &mut MeshMaterial3d<StandardMaterial>,
            Option<&WarmTint>,
        ),
        With<Tile>,
    >,
) {
    let Ok(player_tf) = player_q.single() else { return };
    let player_pos = player_tf.translation;
    let radius_sq = TINT_SCAN_RADIUS * TINT_SCAN_RADIUS;
    let lerp = (TINT_LERP_SPEED * time.delta_secs()).min(1.0);

    for (entity, gt, mut mesh_mat, warm_opt) in &mut tiles {
        let pos = gt.translation();
        let dx = pos.x - player_pos.x;
        let dz = pos.z - player_pos.z;
        if dx * dx + dz * dz > radius_sq {
            continue;
        }

        let cell = world_to_cell(pos);
        let warmth = world_memory.warmth_at(cell);

        match warm_opt {
            None => {
                if warmth < TINT_THRESHOLD {
                    continue;
                }
                // First time warming this tile: clone the shared material so
                // we can mutate it without affecting the rest of the chunk.
                let original = mesh_mat.0.clone();
                let Some(base) = materials.get(&original) else { continue };
                let mut cloned_mat = base.clone();
                cloned_mat.emissive = warm_emissive(warmth).into();
                let cloned_handle = materials.add(cloned_mat);
                mesh_mat.0 = cloned_handle;
                commands.entity(entity).insert(WarmTint { original });
            }
            Some(warm) => {
                if warmth < TINT_THRESHOLD {
                    // Cooled: hand the shared base material back. The cloned
                    // material asset is no longer referenced and will be
                    // dropped by Bevy's asset reaper.
                    mesh_mat.0 = warm.original.clone();
                    commands.entity(entity).remove::<WarmTint>();
                    continue;
                }
                // Warm and already cloned: ease the emissive toward the
                // target warmth so changes feel like a slow ember rather
                // than a step.
                let Some(mat) = materials.get_mut(&mesh_mat.0) else { continue };
                let target = warm_emissive(warmth);
                let cur = LinearRgba::from(mat.emissive);
                let target_lin = LinearRgba::from(target);
                let next = LinearRgba::new(
                    cur.red + (target_lin.red - cur.red) * lerp,
                    cur.green + (target_lin.green - cur.green) * lerp,
                    cur.blue + (target_lin.blue - cur.blue) * lerp,
                    1.0,
                );
                mat.emissive = next;
            }
        }
    }
}

/// Map warmth (0..1) to an emissive tint. Curve is biased toward the warm
/// end so even mid-warmth cells read as visibly amber.
fn warm_emissive(warmth: f32) -> Color {
    let w = warmth.clamp(0.0, 1.0);
    Color::srgb(0.55 * w, 0.28 * w, 0.08 * w)
}
