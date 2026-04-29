use bevy::prelude::*;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, move_player);
    }
}

#[derive(Component)]
pub struct Player;

const PLAYER_SPEED: f32 = 5.0;

fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Placeholder capsule for the cat -- will be replaced with the actual model
    let body_color = Color::srgb(0.76, 0.60, 0.42); // warm tan matching the cat asset

    commands.spawn((
        Player,
        Mesh3d(meshes.add(Mesh::from(Capsule3d::new(0.3, 0.8)))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: body_color,
            perceptual_roughness: 0.8,
            ..default()
        })),
        Transform::from_xyz(0.0, 1.0, 0.0),
    ));
}

fn move_player(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Ok(mut transform) = query.single_mut() else {
        return;
    };

    let mut direction = Vec3::ZERO;

    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        direction.z -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        direction.z += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        direction.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        direction.x += 1.0;
    }

    // Rotate movement to align with isometric camera
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
    direction = rotation * direction;

    if direction.length_squared() > 0.0 {
        direction = direction.normalize();
        transform.translation += direction * PLAYER_SPEED * time.delta_secs();

        // Face movement direction
        let target = transform.translation + direction;
        transform.look_at(target, Vec3::Y);
    }
}
