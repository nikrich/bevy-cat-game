use bevy::prelude::*;
use crate::crafting::CraftingState;
use crate::input::GameInput;
use crate::save::LoadedPlayerPos;
use crate::world::biome::WorldNoise;
use crate::world::chunks::ChunkManager;
use crate::world::terrain::step_height;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(Update, (move_player, snap_to_terrain, apply_loaded_position));
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
    input: Res<GameInput>,
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Player>>,
    crafting: Res<CraftingState>,
    build_mode: Option<Res<crate::building::BuildMode>>,
) -> Result {
    if crafting.open {
        return Ok(());
    }

    let mut transform = query.single_mut()?;

    let dir = input.movement;
    if dir.length_squared() > 0.0 {
        let direction = Vec3::new(dir.x, 0.0, -dir.y);
        transform.translation += direction * PLAYER_SPEED * time.delta_secs();

        // Face movement direction (unless in build mode where we might want to face cursor)
        if build_mode.is_none() {
            let angle = direction.x.atan2(direction.z);
            transform.rotation = Quat::from_rotation_y(angle);
        }
    }

    // In build mode, face toward cursor
    if let Some(_) = &build_mode {
        if let Some(cursor) = input.cursor_world {
            let to_cursor = cursor - transform.translation;
            if to_cursor.length_squared() > 0.1 {
                let angle = to_cursor.x.atan2(to_cursor.z);
                transform.rotation = Quat::from_rotation_y(angle);
            }
        }
    }

    Ok(())
}

fn snap_to_terrain(
    mut query: Query<&mut Transform, With<Player>>,
    chunk_manager: Res<ChunkManager>,
    time: Res<Time>,
) -> Result {
    let mut transform = query.single_mut()?;

    let noise = WorldNoise::new(chunk_manager.seed);
    let sample = noise.sample(
        transform.translation.x as f64,
        transform.translation.z as f64,
    );
    let sh = step_height(sample.elevation * sample.biome.height_scale());
    let target_y = sh * 0.5 + 0.5;

    let smoothing = (8.0 * time.delta_secs()).min(1.0);
    transform.translation.y = transform.translation.y + (target_y - transform.translation.y) * smoothing;

    Ok(())
}

fn apply_loaded_position(
    mut commands: Commands,
    loaded: Option<Res<LoadedPlayerPos>>,
    mut query: Query<&mut Transform, With<Player>>,
) {
    let Some(loaded) = loaded else { return };
    let Ok(mut transform) = query.single_mut() else { return };
    transform.translation = loaded.0;
    commands.remove_resource::<LoadedPlayerPos>();
}
