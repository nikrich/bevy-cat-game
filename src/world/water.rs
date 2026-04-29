use bevy::prelude::*;

use super::terrain::WaterTile;

#[derive(Component, Default)]
pub struct WaterRipple {
    pub base_y: f32,
    pub initialized: bool,
}

pub fn init_water_ripples(
    mut commands: Commands,
    water_tiles: Query<(Entity, &Transform), (With<WaterTile>, Without<WaterRipple>)>,
) {
    for (entity, tf) in &water_tiles {
        // `try_insert` swallows the despawn race: chunks unload (and take
        // their water tiles with them) between the query collection here
        // and the deferred command apply, which without this would panic.
        // The race got more frequent once rapier was added — the player
        // rigid body churns ChunkManager.player_chunk faster while it
        // settles under gravity.
        commands.entity(entity).try_insert(WaterRipple {
            base_y: tf.translation.y,
            initialized: true,
        });
    }
}

/// Sample a continuous wave field at a world position. Wavelengths are tens of tiles wide
/// so neighboring tiles sit at nearly the same phase, giving a coupled "swell" look rather
/// than independent bobbing. Superposing several directions/frequencies keeps it organic.
fn wave_height(x: f32, z: f32, t: f32) -> f32 {
    let w1 = (x * 0.20 + z * 0.13 - t * 0.70).sin() * 0.06;
    let w2 = (x * 0.13 - z * 0.20 + t * 0.90).sin() * 0.05;
    let w3 = (x * 0.07 + z * 0.07 - t * 0.40).sin() * 0.04;
    let swell = (x * 0.04 - z * 0.05 + t * 0.25).sin() * 0.05;
    w1 + w2 + w3 + swell
}

pub fn update_water_ripples(
    mut water: Query<(&GlobalTransform, &WaterRipple, &mut Transform), With<WaterTile>>,
    time: Res<Time>,
) {
    let t = time.elapsed_secs();
    for (global_tf, ripple, mut transform) in &mut water {
        if !ripple.initialized {
            continue;
        }
        let pos = global_tf.translation();
        transform.translation.y = ripple.base_y + wave_height(pos.x, pos.z, t);
    }
}
