use bevy::prelude::Color;
use serde::{Deserialize, Serialize};

/// Palette + physical properties paired with a `Form` to make a concrete item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Material {
    // Wood family
    Pine,
    Oak,
    Birch,

    // Stone family
    Stone,
    Sandstone,
    Brick,

    // Plant family
    FlowerMix,
    Mushroom,
    Bush,
    Cactus,

    // Special / placeholders
    Iron,
    Linen,
    Wool,

    /// Used for Forms whose Material is implicit (Stew, Wreath).
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaterialFamily {
    Wood,
    Stone,
    Plant,
    Metal,
    Fabric,
    None,
}

impl Material {
    pub fn family(self) -> MaterialFamily {
        match self {
            Material::Pine | Material::Oak | Material::Birch => MaterialFamily::Wood,
            Material::Stone | Material::Sandstone | Material::Brick => MaterialFamily::Stone,
            Material::FlowerMix | Material::Mushroom | Material::Bush | Material::Cactus => {
                MaterialFamily::Plant
            }
            Material::Iron => MaterialFamily::Metal,
            Material::Linen | Material::Wool => MaterialFamily::Fabric,
            Material::None => MaterialFamily::None,
        }
    }

    pub fn base_color(self) -> Color {
        match self {
            Material::Pine => Color::srgb(0.35, 0.28, 0.18),
            Material::Oak => Color::srgb(0.55, 0.38, 0.22),
            Material::Birch => Color::srgb(0.85, 0.78, 0.62),
            Material::Stone => Color::srgb(0.55, 0.52, 0.48),
            Material::Sandstone => Color::srgb(0.78, 0.68, 0.50),
            Material::Brick => Color::srgb(0.62, 0.40, 0.32),
            Material::FlowerMix => Color::srgb(0.85, 0.45, 0.50),
            Material::Mushroom => Color::srgb(0.75, 0.35, 0.30),
            Material::Bush => Color::srgb(0.32, 0.52, 0.28),
            Material::Cactus => Color::srgb(0.35, 0.55, 0.30),
            Material::Iron => Color::srgb(0.45, 0.45, 0.50),
            Material::Linen => Color::srgb(0.90, 0.85, 0.70),
            Material::Wool => Color::srgb(0.92, 0.90, 0.82),
            Material::None => Color::srgb(0.65, 0.55, 0.40),
        }
    }

    pub fn roughness(self) -> f32 {
        match self.family() {
            MaterialFamily::Wood => 0.85,
            MaterialFamily::Stone => 0.95,
            MaterialFamily::Metal => 0.30,
            MaterialFamily::Fabric => 0.80,
            _ => 0.80,
        }
    }

    /// Adjective used in display names. "{Pine} Plank" -> "Pine Plank".
    pub fn display_adjective(self) -> &'static str {
        match self {
            Material::Pine => "Pine",
            Material::Oak => "Oak",
            Material::Birch => "Birch",
            Material::Stone => "Stone",
            Material::Sandstone => "Sandstone",
            Material::Brick => "Clay",
            Material::FlowerMix => "Wild",
            Material::Mushroom => "Mushroom",
            Material::Bush => "Bush",
            Material::Cactus => "Cactus",
            Material::Iron => "Iron",
            Material::Linen => "Linen",
            Material::Wool => "Woolen",
            Material::None => "",
        }
    }

    pub fn save_key(self) -> &'static str {
        match self {
            Material::Pine => "pine",
            Material::Oak => "oak",
            Material::Birch => "birch",
            Material::Stone => "stone",
            Material::Sandstone => "sandstone",
            Material::Brick => "brick",
            Material::FlowerMix => "flower",
            Material::Mushroom => "mushroom",
            Material::Bush => "bush",
            Material::Cactus => "cactus",
            Material::Iron => "iron",
            Material::Linen => "linen",
            Material::Wool => "wool",
            Material::None => "none",
        }
    }
}
