use bevy::prelude::*;
use noise::Perlin;

use crate::world::chunks::ChunkManager;
use crate::world::terrain::{step_height, terrain_height};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, (move_player, snap_to_terrain));
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
    let body_color = Color::srgb(0.76, 0.60, 0.42);

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
) -> Result {
    let mut transform = query.single_mut()?;

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

    let iso_rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
    direction = iso_rotation * direction;

    if direction.length_squared() > 0.0 {
        direction = direction.normalize();
        transform.translation += direction * PLAYER_SPEED * time.delta_secs();

        let angle = direction.x.atan2(direction.z);
        transform.rotation = Quat::from_rotation_y(angle);
    }

    Ok(())
}

fn snap_to_terrain(
    mut query: Query<&mut Transform, With<Player>>,
    chunk_manager: Res<ChunkManager>,
    time: Res<Time>,
) -> Result {
    let mut transform = query.single_mut()?;

    let perlin = Perlin::new(chunk_manager.seed);
    let height = terrain_height(
        &perlin,
        transform.translation.x as f64,
        transform.translation.z as f64,
    );
    let sh = step_height(height);
    let target_y = sh * 0.5 + 0.5;

    // Smooth vertical movement to avoid jumpiness on stepped terrain
    let smoothing = (8.0 * time.delta_secs()).min(1.0);
    transform.translation.y = transform.translation.y + (target_y - transform.translation.y) * smoothing;

    Ok(())
}
