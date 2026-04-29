pub mod biome;
pub mod chunks;
pub mod daynight;
pub mod props;
pub mod terrain;
pub mod water;

use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<chunks::ChunkManager>()
            .init_resource::<daynight::WorldTime>()
            .add_event::<chunks::ChunkLoaded>()
            .add_systems(Startup, spawn_light)
            .add_systems(
                Update,
                (
                    chunks::track_player_chunk,
                    chunks::load_nearby_chunks,
                    chunks::unload_distant_chunks,
                    props::spawn_chunk_props,
                    props::sway_props_near_player,
                    props::apply_prop_sway,
                    daynight::advance_time,
                    daynight::update_sun,
                    daynight::update_sky_color,
                    daynight::update_ambient_light,
                    water::init_water_ripples,
                    water::update_water_ripples,
                ),
            );
    }
}

fn spawn_light(mut commands: Commands) {
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        // Cascade shadow map only to cap how far shadows project (so dusk-angle
        // tree shadows don't streak across the whole biome). Bias values stay at
        // Bevy defaults to avoid peter-panning.
        CascadeShadowConfigBuilder {
            num_cascades: 3,
            minimum_distance: 0.1,
            first_cascade_far_bound: 12.0,
            maximum_distance: 60.0,
            overlap_proportion: 0.2,
        }
        .build(),
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
    ));
}
