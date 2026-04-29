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
        commands.entity(entity).insert(WaterRipple {
            base_y: tf.translation.y,
            initialized: true,
        });
    }
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
        let wave = (pos.x * 0.5 + pos.z * 0.3 + t * 1.2).sin() * 0.02
            + (pos.x * 0.3 - pos.z * 0.7 + t * 1.8).sin() * 0.015;
        transform.translation.y = ripple.base_y + wave;
    }
}
