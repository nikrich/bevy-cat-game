use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use serde::{Deserialize, Serialize};

/// All biome types in the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Biome {
    Ocean,
    Beach,
    Desert,
    Grassland,
    Meadow,
    Forest,
    Taiga,
    Tundra,
    Snow,
    Mountain,
}

/// Sea level threshold -- anything below this is water.
pub const SEA_LEVEL: f32 = -0.6;
/// Beach extends slightly above sea level.
const BEACH_LEVEL: f32 = -0.35;
/// Mountains start at this elevation.
const MOUNTAIN_LEVEL: f32 = 1.8;
/// Snow caps above this elevation regardless of temperature.
const SNOW_CAP_LEVEL: f32 = 2.8;

/// Noise-based world parameters at a given position.
pub struct WorldSample {
    pub elevation: f32,
    pub temperature: f32,
    pub moisture: f32,
    pub biome: Biome,
    pub river: f32,
}

/// All noise generators for the world, derived from a single seed.
///
/// Stored as a `Resource` (built once at startup from `ChunkManager.seed`) so
/// per-frame systems can borrow it instead of rebuilding all six Perlin
/// generators every tick. Closes DEBT-012.
#[derive(Resource)]
pub struct WorldNoise {
    pub elevation: Perlin,
    pub temperature: Perlin,
    pub moisture: Perlin,
    pub river: Perlin,
    pub river_warp: Perlin,
    pub mountain: Perlin,
}

impl FromWorld for WorldNoise {
    fn from_world(world: &mut World) -> Self {
        let seed = world
            .get_resource::<crate::world::chunks::ChunkManager>()
            .map(|m| m.seed)
            .unwrap_or(0);
        Self::new(seed)
    }
}

impl WorldNoise {
    pub fn new(seed: u32) -> Self {
        Self {
            elevation: Perlin::new(seed),
            temperature: Perlin::new(seed.wrapping_add(100)),
            moisture: Perlin::new(seed.wrapping_add(200)),
            river: Perlin::new(seed.wrapping_add(300)),
            river_warp: Perlin::new(seed.wrapping_add(400)),
            mountain: Perlin::new(seed.wrapping_add(500)),
        }
    }

    /// Sample the world at a given position to get elevation, biome, etc.
    pub fn sample(&self, world_x: f64, world_z: f64) -> WorldSample {
        let elevation = self.sample_elevation(world_x, world_z);
        let temperature = self.sample_temperature(world_x, world_z, elevation);
        let moisture = self.sample_moisture(world_x, world_z);
        let river = self.sample_river(world_x, world_z);
        let biome = Self::classify_biome(elevation, temperature, moisture, river);

        WorldSample {
            elevation,
            temperature,
            moisture,
            biome,
            river,
        }
    }

    fn sample_elevation(&self, x: f64, z: f64) -> f32 {
        let nx = x * 0.03;
        let nz = z * 0.03;

        // Base continental shape (large scale)
        let continental = self.elevation.get([nx * 0.3, nz * 0.3]) * 1.5;

        // Medium detail
        let detail = self.elevation.get([nx, nz]) * 1.0
            + self.elevation.get([nx * 2.0, nz * 2.0]) * 0.5
            + self.elevation.get([nx * 4.0, nz * 4.0]) * 0.2;

        // Mountain ridges -- sharp, dramatic peaks using absolute noise (ridged)
        let mx = x * 0.015;
        let mz = z * 0.015;
        let ridge = 1.0 - self.mountain.get([mx, mz]).abs() as f32;
        let ridge = ridge * ridge * ridge; // sharpen peaks
        let mountain_mask = (continental as f32 - 0.2).max(0.0); // mountains only on high continents
        let mountains = ridge * mountain_mask * 4.0;

        let base = continental as f32 + detail as f32;

        // Combine: base terrain + mountains
        base + mountains
    }

    fn sample_temperature(&self, x: f64, z: f64, elevation: f32) -> f32 {
        let nx = x * 0.008;
        let nz = z * 0.008;

        // Large-scale temperature zones
        let base_temp = self.temperature.get([nx, nz]) as f32;

        // Temperature decreases with elevation (lapse rate)
        let altitude_cooling = (elevation - 0.5).max(0.0) * 0.4;

        (base_temp - altitude_cooling).clamp(-1.0, 1.0)
    }

    fn sample_moisture(&self, x: f64, z: f64) -> f32 {
        let nx = x * 0.01;
        let nz = z * 0.01;

        let base = self.moisture.get([nx, nz]) as f32;
        let detail = self.moisture.get([nx * 3.0, nz * 3.0]) as f32 * 0.3;

        (base + detail).clamp(-1.0, 1.0)
    }

    fn sample_river(&self, x: f64, z: f64) -> f32 {
        let scale = 0.012;
        // Domain warp for natural-looking meandering
        let warp_strength = 15.0;
        let wx = self.river_warp.get([x * scale * 0.5, z * scale * 0.5]) * warp_strength;
        let wz = self.river_warp.get([x * scale * 0.5 + 50.0, z * scale * 0.5 + 50.0]) * warp_strength;

        let nx = (x + wx) * scale;
        let nz = (z + wz) * scale;

        // River is where noise is close to zero (creates thin lines)
        let river_noise = self.river.get([nx, nz]).abs() as f32;
        river_noise
    }

    fn classify_biome(elevation: f32, temperature: f32, moisture: f32, river: f32) -> Biome {
        // Water
        if elevation < SEA_LEVEL {
            return Biome::Ocean;
        }

        // Rivers cut through terrain
        if river < 0.03 && elevation < MOUNTAIN_LEVEL && elevation > SEA_LEVEL + 0.1 {
            return Biome::Ocean; // river tiles become water
        }

        // Beach near water level
        if elevation < BEACH_LEVEL {
            return Biome::Beach;
        }

        // Snow caps on high mountains
        if elevation > SNOW_CAP_LEVEL {
            return Biome::Snow;
        }

        // High mountains
        if elevation > MOUNTAIN_LEVEL {
            return Biome::Mountain;
        }

        // Temperature/moisture driven biomes for normal terrain
        match (temperature, moisture) {
            // Cold regions
            (t, _) if t < -0.5 => Biome::Snow,
            (t, _) if t < -0.2 => Biome::Tundra,
            (t, m) if t < 0.1 && m > 0.0 => Biome::Taiga,

            // Hot regions
            (t, m) if t > 0.3 && m < -0.2 => Biome::Desert,

            // Temperate regions
            (_, m) if m > 0.3 => Biome::Forest,
            (_, m) if m > 0.0 => Biome::Meadow,
            _ => Biome::Grassland,
        }
    }
}

/// Colors for each biome's terrain tiles.
impl Biome {
    pub fn terrain_color(&self, shade: u8) -> Color {
        match (self, shade % 3) {
            (Biome::Ocean, 0) => Color::srgb(0.18, 0.35, 0.55),
            (Biome::Ocean, _) => Color::srgb(0.20, 0.38, 0.58),

            (Biome::Beach, 0) => Color::srgb(0.85, 0.78, 0.62),
            (Biome::Beach, 1) => Color::srgb(0.82, 0.76, 0.58),
            (Biome::Beach, _) => Color::srgb(0.88, 0.82, 0.66),

            (Biome::Desert, 0) => Color::srgb(0.82, 0.72, 0.50),
            (Biome::Desert, 1) => Color::srgb(0.78, 0.68, 0.46),
            (Biome::Desert, _) => Color::srgb(0.85, 0.75, 0.52),

            (Biome::Grassland, 0) => Color::srgb(0.45, 0.65, 0.35),
            (Biome::Grassland, 1) => Color::srgb(0.55, 0.72, 0.40),
            (Biome::Grassland, _) => Color::srgb(0.62, 0.78, 0.45),

            (Biome::Meadow, 0) => Color::srgb(0.52, 0.72, 0.38),
            (Biome::Meadow, 1) => Color::srgb(0.58, 0.76, 0.42),
            (Biome::Meadow, _) => Color::srgb(0.48, 0.68, 0.36),

            (Biome::Forest, 0) => Color::srgb(0.28, 0.50, 0.25),
            (Biome::Forest, 1) => Color::srgb(0.32, 0.55, 0.28),
            (Biome::Forest, _) => Color::srgb(0.25, 0.45, 0.22),

            (Biome::Taiga, 0) => Color::srgb(0.30, 0.45, 0.32),
            (Biome::Taiga, 1) => Color::srgb(0.35, 0.50, 0.36),
            (Biome::Taiga, _) => Color::srgb(0.28, 0.42, 0.30),

            (Biome::Tundra, 0) => Color::srgb(0.55, 0.58, 0.50),
            (Biome::Tundra, 1) => Color::srgb(0.50, 0.55, 0.48),
            (Biome::Tundra, _) => Color::srgb(0.58, 0.60, 0.52),

            (Biome::Snow, 0) => Color::srgb(0.92, 0.94, 0.96),
            (Biome::Snow, 1) => Color::srgb(0.88, 0.90, 0.95),
            (Biome::Snow, _) => Color::srgb(0.95, 0.96, 0.98),

            (Biome::Mountain, 0) => Color::srgb(0.52, 0.50, 0.48),
            (Biome::Mountain, 1) => Color::srgb(0.58, 0.55, 0.52),
            (Biome::Mountain, _) => Color::srgb(0.48, 0.46, 0.44),
        }
    }

    /// Height multiplier -- mountains get amplified, ocean gets flattened.
    pub fn height_scale(&self) -> f32 {
        match self {
            Biome::Ocean => 0.3,
            Biome::Beach => 0.4,
            Biome::Desert => 0.6,
            Biome::Grassland => 0.8,
            Biome::Meadow => 0.7,
            Biome::Forest => 0.9,
            Biome::Taiga => 0.9,
            Biome::Tundra => 0.7,
            Biome::Snow => 1.0,
            Biome::Mountain => 1.2,
        }
    }

    /// Whether this biome is water.
    pub fn is_water(&self) -> bool {
        matches!(self, Biome::Ocean)
    }

    /// Roughness for the material.
    pub fn roughness(&self) -> f32 {
        match self {
            Biome::Ocean => 0.3,
            Biome::Snow => 0.6,
            _ => 0.9,
        }
    }
}
