use bevy::prelude::*;
use crate::crafting::CraftingState;
use crate::input::GameInput;
use crate::save::LoadedPlayerPos;
use crate::world::biome::{WorldNoise, SEA_LEVEL};
use crate::world::chunks::ChunkManager;
use crate::world::props::PropCollision;
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
    props: Query<(&GlobalTransform, &PropCollision)>,
    time: Res<Time>,
) -> Result {
    let mut transform = query.single_mut()?;

    let noise = WorldNoise::new(chunk_manager.seed);
    let sample = noise.sample(
        transform.translation.x as f64,
        transform.translation.z as f64,
    );
    let mut target_y = if sample.biome.is_water() {
        // Wading in water: capsule centre sits just above the painted water
        // surface so the cat looks half-submerged rather than floating.
        // Water surface = step_height(SEA_LEVEL) * 0.5 - 0.15 (matches terrain).
        let water_surface = step_height(SEA_LEVEL) * 0.5 - 0.15;
        water_surface + 0.4
    } else {
        // Tile is a 1.0x0.6x1.0 cuboid centred on `sh * 0.5`, so its top sits
        // at `sh * 0.5 + 0.3`. Capsule3d::new(0.3, 0.8) extends 0.7 below its
        // centre, so we need centre = tile_top + 0.7 to plant the feet on the
        // dirt surface rather than burying them.
        let sh = step_height(sample.elevation * sample.biome.height_scale());
        sh * 0.5 + 1.0
    };

    // Climb onto any nearby prop with a PropCollision -- pick the highest top
    // within reach so stacked / overlapping props don't fight each other.
    let p = transform.translation;
    for (gt, col) in &props {
        let pos = gt.translation();
        let dx = pos.x - p.x;
        let dz = pos.z - p.z;
        let dist_sq = dx * dx + dz * dz;
        if dist_sq < col.radius * col.radius {
            // Capsule3d::new(0.3, 0.8) extends 0.7 below its centre, so put the
            // centre 0.7 above the prop top to plant the cat's feet on top.
            let stand_y = col.top_y + 0.7;
            if stand_y > target_y {
                target_y = stand_y;
            }
        }
    }

    let smoothing = (8.0 * time.delta_secs()).min(1.0);
    transform.translation.y =
        transform.translation.y + (target_y - transform.translation.y) * smoothing;

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
