use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

pub mod occluder_fade;

use crate::input::Action;
use crate::player::Player;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraOrbit>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (rotate_camera, follow_player).chain());
        occluder_fade::register(app);
    }
}

#[derive(Component)]
pub struct GameCamera;

/// 90° snap rotation around the player. `step` cycles 0..4 (each step is a
/// quarter turn around the +Y axis). Read by `follow_player` to rotate the
/// camera offset and by `iso_movement` to keep WASD aligned with the
/// on-screen "up" direction.
#[derive(Resource, Default, Clone, Copy)]
pub struct CameraOrbit {
    pub step: u8,
}

impl CameraOrbit {
    /// Yaw in radians of the current snap step (0, π/2, π, 3π/2).
    pub fn yaw(self) -> f32 {
        self.step as f32 * std::f32::consts::FRAC_PI_2
    }
}

const CAMERA_HEIGHT: f32 = 18.0;
const CAMERA_DISTANCE: f32 = 18.0;
const CAMERA_SMOOTHING: f32 = 5.0;

fn spawn_camera(mut commands: Commands) {
    let camera_offset = base_offset();

    commands.spawn((
        GameCamera,
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 20.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(camera_offset).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Default iso offset (camera at +X+Z corner, looking at origin). Rotated by
/// `CameraOrbit::yaw` in `follow_player` to produce the four snap views.
fn base_offset() -> Vec3 {
    Vec3::new(CAMERA_DISTANCE, CAMERA_HEIGHT, CAMERA_DISTANCE)
}

/// Q/E are taken by hotbar cycling; comma/period are the iso-game convention
/// (used for camera rotation in many ARPGs / iso builders) and stay free.
fn rotate_camera(action_state: Res<ActionState<Action>>, mut orbit: ResMut<CameraOrbit>) {
    if action_state.just_pressed(&Action::RotateCameraLeft) {
        orbit.step = (orbit.step + 3) % 4;
    }
    if action_state.just_pressed(&Action::RotateCameraRight) {
        orbit.step = (orbit.step + 1) % 4;
    }
}

fn follow_player(
    time: Res<Time>,
    orbit: Res<CameraOrbit>,
    player_query: Query<&Transform, (With<Player>, Without<GameCamera>)>,
    mut camera_query: Query<&mut Transform, With<GameCamera>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera_query.single_mut() else {
        return;
    };

    let offset = Quat::from_rotation_y(orbit.yaw()) * base_offset();
    let target_pos = player_transform.translation + offset;

    let t = (CAMERA_SMOOTHING * time.delta_secs()).min(1.0);
    camera_transform.translation = camera_transform.translation.lerp(target_pos, t);
    camera_transform.look_at(player_transform.translation, Vec3::Y);
}
