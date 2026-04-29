use bevy::prelude::*;
use rand::prelude::*;

use crate::player::Player;
use crate::world::biome::{Biome, WorldNoise};
use crate::world::chunks::{ChunkLoaded, CHUNK_SIZE};
use crate::world::terrain::step_height;

pub struct AnimalPlugin;

impl Plugin for AnimalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_animals, wander_animals, flee_from_player));
    }
}

#[derive(Component)]
pub struct Animal {
    pub kind: AnimalKind,
    pub wander_timer: f32,
    pub wander_dir: Vec3,
    pub speed: f32,
}

#[derive(Clone, Copy)]
pub enum AnimalKind {
    Rabbit,
    Fox,
    Deer,
    Penguin,
    Lizard,
}

impl AnimalKind {
    fn speed(&self) -> f32 {
        match self {
            AnimalKind::Rabbit => 3.0,
            AnimalKind::Fox => 2.5,
            AnimalKind::Deer => 3.5,
            AnimalKind::Penguin => 1.5,
            AnimalKind::Lizard => 2.0,
        }
    }

    fn color(&self) -> Color {
        match self {
            AnimalKind::Rabbit => Color::srgb(0.82, 0.78, 0.72),
            AnimalKind::Fox => Color::srgb(0.85, 0.55, 0.25),
            AnimalKind::Deer => Color::srgb(0.65, 0.50, 0.35),
            AnimalKind::Penguin => Color::srgb(0.15, 0.15, 0.18),
            AnimalKind::Lizard => Color::srgb(0.55, 0.60, 0.40),
        }
    }

    fn size(&self) -> Vec3 {
        match self {
            AnimalKind::Rabbit => Vec3::new(0.12, 0.15, 0.18),
            AnimalKind::Fox => Vec3::new(0.15, 0.18, 0.3),
            AnimalKind::Deer => Vec3::new(0.15, 0.35, 0.3),
            AnimalKind::Penguin => Vec3::new(0.12, 0.22, 0.12),
            AnimalKind::Lizard => Vec3::new(0.08, 0.06, 0.2),
        }
    }

    fn for_biome(biome: Biome) -> Option<AnimalKind> {
        match biome {
            Biome::Grassland | Biome::Meadow => Some(AnimalKind::Rabbit),
            Biome::Forest => Some(AnimalKind::Fox),
            Biome::Taiga => Some(AnimalKind::Deer),
            Biome::Snow | Biome::Tundra => Some(AnimalKind::Penguin),
            Biome::Desert => Some(AnimalKind::Lizard),
            _ => None,
        }
    }
}

#[derive(Component)]
struct Fleeing {
    timer: f32,
}

fn spawn_animals(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut chunk_events: MessageReader<ChunkLoaded>,
    noise: Res<WorldNoise>,
) {
    let mut rng = rand::thread_rng();

    for event in chunk_events.read() {
        // Only spawn 0-2 animals per chunk
        let animal_count = rng.gen_range(0..=2u32);
        if animal_count == 0 {
            continue;
        }

        for _ in 0..animal_count {
            let lx = rng.gen_range(0..CHUNK_SIZE);
            let lz = rng.gen_range(0..CHUNK_SIZE);
            let wx = event.x * CHUNK_SIZE + lx;
            let wz = event.z * CHUNK_SIZE + lz;

            let sample = noise.sample(wx as f64, wz as f64);

            let Some(kind) = AnimalKind::for_biome(sample.biome) else {
                continue;
            };

            let sh = step_height(sample.elevation * sample.biome.height_scale());
            let y = sh * 0.5 + 0.2;

            let mesh = meshes.add(Mesh::from(Cuboid::new(
                kind.size().x * 2.0,
                kind.size().y * 2.0,
                kind.size().z * 2.0,
            )));
            let mat = materials.add(StandardMaterial {
                base_color: kind.color(),
                perceptual_roughness: 0.85,
                ..default()
            });

            // Animals are spawned as top-level entities rather than chunk
            // children: they already carry absolute world Transforms, and
            // making them children of a chunk that may despawn before this
            // command applies (load-save race) panicked add_child at apply
            // time. They simply outlive their original chunk.
            commands.spawn((
                Animal {
                    kind,
                    wander_timer: rng.gen_range(0.0..3.0f32),
                    wander_dir: Vec3::ZERO,
                    speed: kind.speed(),
                },
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                Transform::from_xyz(wx as f32, y, wz as f32),
            ));
        }
    }
}

fn wander_animals(
    mut animals: Query<(&mut Animal, &mut Transform), Without<Fleeing>>,
    noise: Res<WorldNoise>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (mut animal, mut transform) in &mut animals {
        animal.wander_timer -= dt;

        if animal.wander_timer <= 0.0 {
            // Pick a new random direction
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            animal.wander_dir = Vec3::new(angle.cos(), 0.0, angle.sin());
            animal.wander_timer = rng.gen_range(1.5..4.0f32);
        }

        // Move
        let move_speed = animal.speed * 0.3; // wander is slower
        transform.translation += animal.wander_dir * move_speed * dt;

        // Snap to terrain
        let sample = noise.sample(
            transform.translation.x as f64,
            transform.translation.z as f64,
        );
        let sh = step_height(sample.elevation * sample.biome.height_scale());
        transform.translation.y = sh * 0.5 + 0.2;

        // Face movement direction
        if animal.wander_dir.length_squared() > 0.01 {
            let angle = animal.wander_dir.x.atan2(animal.wander_dir.z);
            transform.rotation = Quat::from_rotation_y(angle);
        }
    }
}

fn flee_from_player(
    mut commands: Commands,
    player_query: Query<&GlobalTransform, With<Player>>,
    mut animals: Query<(Entity, &Animal, &GlobalTransform, &mut Transform, Option<&mut Fleeing>)>,
    noise: Res<WorldNoise>,
    time: Res<Time>,
) {
    let Ok(player_gt) = player_query.single() else { return };
    let player_pos = player_gt.translation();
    let dt = time.delta_secs();
    let flee_radius = 4.0;

    for (entity, animal, global_tf, mut transform, fleeing) in &mut animals {
        let pos = global_tf.translation();
        let dx = pos.x - player_pos.x;
        let dz = pos.z - player_pos.z;
        let dist = (dx * dx + dz * dz).sqrt();

        if dist < flee_radius {
            // Flee away from player
            if dist > 0.1 {
                let flee_dir = Vec3::new(dx / dist, 0.0, dz / dist);
                transform.translation += flee_dir * animal.speed * dt;

                // Snap to terrain
                let sample = noise.sample(
                    transform.translation.x as f64,
                    transform.translation.z as f64,
                );
                let sh = step_height(sample.elevation * sample.biome.height_scale());
                transform.translation.y = sh * 0.5 + 0.2;

                let angle = flee_dir.x.atan2(flee_dir.z);
                transform.rotation = Quat::from_rotation_y(angle);
            }

            if fleeing.is_none() {
                commands.entity(entity).insert(Fleeing { timer: 2.0 });
            }
        } else if let Some(mut flee) = fleeing {
            flee.timer -= dt;
            if flee.timer <= 0.0 {
                commands.entity(entity).remove::<Fleeing>();
            }
        }
    }
}
