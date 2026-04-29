use bevy::prelude::*;
use rand::prelude::*;

use crate::player::Player;
use crate::world::biome::{Biome, WorldNoise};
use crate::world::chunks::ChunkManager;
use crate::world::daynight::WorldTime;

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (spawn_particles, update_particles));
    }
}

#[derive(Component)]
struct Particle {
    velocity: Vec3,
    lifetime: f32,
    age: f32,
    kind: ParticleKind,
}

#[derive(Clone, Copy)]
enum ParticleKind {
    Leaf,
    Firefly,
    Snowflake,
    SandWisp,
    Pollen,
}

const MAX_PARTICLES: usize = 150;
const SPAWN_RATE: f32 = 0.08;

fn spawn_particles(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_query: Query<&GlobalTransform, With<Player>>,
    particle_query: Query<&Particle>,
    chunk_manager: Res<ChunkManager>,
    world_time: Res<WorldTime>,
    time: Res<Time>,
    mut timer: Local<f32>,
) {
    *timer += time.delta_secs();
    if *timer < SPAWN_RATE {
        return;
    }
    *timer = 0.0;

    if particle_query.iter().count() >= MAX_PARTICLES {
        return;
    }

    let Ok(player_gt) = player_query.single() else { return };
    let player_pos = player_gt.translation();

    let noise = WorldNoise::new(chunk_manager.seed);
    let mut rng = rand::thread_rng();

    // Sample biome at a random position near player
    let offset_x = rng.gen_range(-8.0..8.0f32);
    let offset_z = rng.gen_range(-8.0..8.0f32);
    let spawn_x = player_pos.x + offset_x;
    let spawn_z = player_pos.z + offset_z;
    let sample = noise.sample(spawn_x as f64, spawn_z as f64);

    let t = world_time.time_of_day;
    let is_dusk = (17.0..=20.0).contains(&t);
    let is_night = t > 20.0 || t < 5.0;

    let kind = match sample.biome {
        Biome::Forest | Biome::Taiga => {
            if is_dusk || is_night {
                ParticleKind::Firefly
            } else {
                ParticleKind::Leaf
            }
        }
        Biome::Snow | Biome::Tundra => ParticleKind::Snowflake,
        Biome::Desert => ParticleKind::SandWisp,
        Biome::Meadow | Biome::Grassland => {
            if is_dusk || is_night {
                ParticleKind::Firefly
            } else {
                ParticleKind::Pollen
            }
        }
        _ => return,
    };

    let spawn_y = player_pos.y + rng.gen_range(1.0..4.0f32);

    let (velocity, lifetime, mesh, color, emissive) = match kind {
        ParticleKind::Leaf => (
            Vec3::new(
                rng.gen_range(-0.5..0.5f32),
                rng.gen_range(-0.8..-0.3f32),
                rng.gen_range(-0.3..0.3f32),
            ),
            rng.gen_range(3.0..5.0f32),
            Mesh::from(Cuboid::new(0.06, 0.02, 0.06)),
            Color::srgb(0.55, 0.45, 0.20),
            Color::BLACK,
        ),
        ParticleKind::Firefly => (
            Vec3::new(
                rng.gen_range(-0.3..0.3f32),
                rng.gen_range(-0.1..0.2f32),
                rng.gen_range(-0.3..0.3f32),
            ),
            rng.gen_range(2.0..4.0f32),
            Mesh::from(Sphere::new(0.03)),
            Color::srgb(0.80, 0.90, 0.30),
            Color::srgb(0.6, 0.7, 0.2),
        ),
        ParticleKind::Snowflake => (
            Vec3::new(
                rng.gen_range(-0.4..0.4f32),
                rng.gen_range(-1.0..-0.4f32),
                rng.gen_range(-0.4..0.4f32),
            ),
            rng.gen_range(3.0..6.0f32),
            Mesh::from(Sphere::new(0.025)),
            Color::srgb(0.95, 0.96, 0.98),
            Color::BLACK,
        ),
        ParticleKind::SandWisp => (
            Vec3::new(
                rng.gen_range(0.3..1.0f32),
                rng.gen_range(-0.1..0.1f32),
                rng.gen_range(-0.2..0.2f32),
            ),
            rng.gen_range(1.5..3.0f32),
            Mesh::from(Cuboid::new(0.04, 0.02, 0.04)),
            Color::srgb(0.82, 0.75, 0.55),
            Color::BLACK,
        ),
        ParticleKind::Pollen => (
            Vec3::new(
                rng.gen_range(-0.2..0.2f32),
                rng.gen_range(0.0..0.15f32),
                rng.gen_range(-0.2..0.2f32),
            ),
            rng.gen_range(3.0..5.0f32),
            Mesh::from(Sphere::new(0.015)),
            Color::srgb(0.95, 0.90, 0.60),
            Color::BLACK,
        ),
    };

    let mesh_handle = meshes.add(mesh);
    let mut mat = StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        ..default()
    };
    if emissive != Color::BLACK {
        mat.emissive = emissive.into();
        mat.unlit = true;
    }
    let mat_handle = materials.add(mat);

    commands.spawn((
        Particle {
            velocity,
            lifetime,
            age: 0.0,
            kind,
        },
        Mesh3d(mesh_handle),
        MeshMaterial3d(mat_handle),
        Transform::from_xyz(spawn_x, spawn_y, spawn_z),
    ));
}

fn update_particles(
    mut commands: Commands,
    mut particles: Query<(Entity, &mut Particle, &mut Transform)>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    for (entity, mut particle, mut transform) in &mut particles {
        particle.age += dt;

        if particle.age >= particle.lifetime {
            commands.entity(entity).despawn();
            continue;
        }

        // Move
        transform.translation += particle.velocity * dt;

        // Per-type behavior
        match particle.kind {
            ParticleKind::Leaf => {
                // Gentle swaying
                transform.translation.x += (t * 2.0 + transform.translation.z).sin() * 0.3 * dt;
                transform.rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    t * 1.5,
                    t * 2.0,
                    0.0,
                );
            }
            ParticleKind::Firefly => {
                // Bobbing, pulsing
                transform.translation.y += (t * 3.0 + transform.translation.x * 2.0).sin() * 0.4 * dt;
                transform.translation.x += (t * 2.5 + transform.translation.z).cos() * 0.3 * dt;
            }
            ParticleKind::Snowflake => {
                // Gentle drift
                transform.translation.x += (t * 1.5 + transform.translation.z * 0.5).sin() * 0.2 * dt;
            }
            ParticleKind::SandWisp => {
                // Wind-driven streaks
                transform.translation.y += (t * 4.0).sin() * 0.1 * dt;
            }
            ParticleKind::Pollen => {
                // Float lazily
                transform.translation.x += (t * 1.0 + transform.translation.z).sin() * 0.15 * dt;
                transform.translation.y += (t * 0.8 + transform.translation.x).cos() * 0.1 * dt;
            }
        }

        // Fade out near end of life (scale down)
        let remaining = 1.0 - (particle.age / particle.lifetime);
        if remaining < 0.3 {
            let scale = remaining / 0.3;
            transform.scale = Vec3::splat(scale.max(0.01));
        }
    }
}
