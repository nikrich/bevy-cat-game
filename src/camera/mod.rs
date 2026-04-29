use bevy::prelude::*;

pub mod occluder_fade;

use crate::player::Player;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            .add_systems(Update, follow_player);
        occluder_fade::register(app);
    }
}

#[derive(Component)]
pub struct GameCamera;

const CAMERA_HEIGHT: f32 = 18.0;
const CAMERA_DISTANCE: f32 = 18.0;
const CAMERA_SMOOTHING: f32 = 5.0;

fn spawn_camera(mut commands: Commands) {
    // Isometric-style camera at ~45 degree angle
    let camera_offset = Vec3::new(CAMERA_DISTANCE, CAMERA_HEIGHT, CAMERA_DISTANCE);

    commands.spawn((
        GameCamera,
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::render::camera::ScalingMode::FixedVertical {
                viewport_height: 20.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(camera_offset).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn follow_player(
    time: Res<Time>,
    player_query: Query<&Transform, (With<Player>, Without<GameCamera>)>,
    mut camera_query: Query<&mut Transform, With<GameCamera>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    let offset = Vec3::new(CAMERA_DISTANCE, CAMERA_HEIGHT, CAMERA_DISTANCE);
    let target_pos = player_transform.translation + offset;

    // Smooth follow
    let t = (CAMERA_SMOOTHING * time.delta_secs()).min(1.0);
    camera_transform.translation = camera_transform.translation.lerp(target_pos, t);

    // Always look at player
    camera_transform.look_at(player_transform.translation, Vec3::Y);
}
