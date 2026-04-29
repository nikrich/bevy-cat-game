use bevy::prelude::*;
use noise::{NoiseFn, Perlin};

const CHUNK_SIZE: i32 = 32;
const TILE_SIZE: f32 = 1.0;

#[derive(Component)]
pub struct Terrain;

#[derive(Component)]
pub struct Tile {
    pub height: f32,
}

pub fn spawn_terrain(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let perlin = Perlin::new(42);

    // Color palette matching the warm, earthy art style
    let grass_colors = [
        Color::srgb(0.45, 0.65, 0.35), // dark grass
        Color::srgb(0.55, 0.72, 0.40), // mid grass
        Color::srgb(0.62, 0.78, 0.45), // light grass
    ];

    let dirt_color = Color::srgb(0.60, 0.48, 0.35);
    let sand_color = Color::srgb(0.82, 0.76, 0.62);

    let tile_mesh = meshes.add(Mesh::from(Cuboid::new(TILE_SIZE, 0.2, TILE_SIZE)));

    let grass_materials: Vec<_> = grass_colors
        .iter()
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: *c,
                perceptual_roughness: 0.9,
                ..default()
            })
        })
        .collect();

    let dirt_material = materials.add(StandardMaterial {
        base_color: dirt_color,
        perceptual_roughness: 0.95,
        ..default()
    });

    let sand_material = materials.add(StandardMaterial {
        base_color: sand_color,
        perceptual_roughness: 0.85,
        ..default()
    });

    let terrain_entity = commands
        .spawn((
            Terrain,
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    for x in -CHUNK_SIZE..CHUNK_SIZE {
        for z in -CHUNK_SIZE..CHUNK_SIZE {
            let nx = x as f64 * 0.05;
            let nz = z as f64 * 0.05;

            // Layer noise for natural-looking terrain
            let height = perlin.get([nx, nz]) * 2.0
                + perlin.get([nx * 2.0, nz * 2.0]) * 0.5
                + perlin.get([nx * 4.0, nz * 4.0]) * 0.25;
            let height = height as f32;

            // Pick material based on height
            let material = if height < -0.5 {
                sand_material.clone()
            } else if height < -0.2 {
                dirt_material.clone()
            } else {
                // Vary grass shade with secondary noise
                let shade_noise = perlin.get([nx * 3.0 + 100.0, nz * 3.0 + 100.0]);
                let idx = if shade_noise < -0.3 {
                    0
                } else if shade_noise < 0.3 {
                    1
                } else {
                    2
                };
                grass_materials[idx].clone()
            };

            // Quantize height to give a low-poly stepped look
            let step_height = (height * 4.0).round() / 4.0;

            let child = commands
                .spawn((
                    Tile { height },
                    Mesh3d(tile_mesh.clone()),
                    MeshMaterial3d(material),
                    Transform::from_xyz(
                        x as f32 * TILE_SIZE,
                        step_height * 0.5,
                        z as f32 * TILE_SIZE,
                    ),
                ))
                .id();

            commands.entity(terrain_entity).add_child(child);
        }
    }

    // Ambient light for warm feel
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: true,
            color: Color::srgb(1.0, 0.95, 0.85),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(
            EulerRot::XYZ,
            -std::f32::consts::FRAC_PI_4,
            std::f32::consts::FRAC_PI_4,
            0.0,
        )),
    ));
}
