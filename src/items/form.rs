use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Visual / behavioural archetype. Pairs with a `Material` to form an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Form {
    // Raws -- "raw chunk of X"
    Log,
    StoneChunk,
    Flower,
    Mushroom,
    BushSprig,
    CactusFlesh,

    // Refined materials
    Plank,
    Brick,

    // Furniture / placeables
    Fence,
    Bench,
    Lantern,
    FlowerPot,
    Wreath,
    Chair,
    Table,

    // Modular building
    Floor,
    Wall,
    Door,
    Window,
    Roof,

    // Consumables
    Stew,
}

impl Form {
    /// English noun used in display names. "Pine {Plank}" -> "Pine Plank".
    pub fn display_noun(self) -> &'static str {
        match self {
            Form::Log => "Log",
            Form::StoneChunk => "Stone",
            Form::Flower => "Flower",
            Form::Mushroom => "Mushroom",
            Form::BushSprig => "Bush",
            Form::CactusFlesh => "Cactus",
            Form::Plank => "Plank",
            Form::Brick => "Brick",
            Form::Fence => "Fence",
            Form::Bench => "Bench",
            Form::Lantern => "Lantern",
            Form::FlowerPot => "Pot",
            Form::Wreath => "Wreath",
            Form::Chair => "Chair",
            Form::Table => "Table",
            Form::Floor => "Floor",
            Form::Wall => "Wall",
            Form::Door => "Door",
            Form::Window => "Window",
            Form::Roof => "Roof",
            Form::Stew => "Stew",
        }
    }

    /// Stable lowercase key used in save files.
    pub fn save_key(self) -> &'static str {
        match self {
            Form::Log => "log",
            Form::StoneChunk => "stone",
            Form::Flower => "flower",
            Form::Mushroom => "mushroom",
            Form::BushSprig => "bush",
            Form::CactusFlesh => "cactus",
            Form::Plank => "plank",
            Form::Brick => "brick",
            Form::Fence => "fence",
            Form::Bench => "bench",
            Form::Lantern => "lantern",
            Form::FlowerPot => "flowerpot",
            Form::Wreath => "wreath",
            Form::Chair => "chair",
            Form::Table => "table",
            Form::Floor => "floor",
            Form::Wall => "wall",
            Form::Door => "door",
            Form::Window => "window",
            Form::Roof => "roof",
            Form::Stew => "stew",
        }
    }

    /// glTF scene path for placed-building rendering. When `Some`, the building
    /// system spawns a `SceneRoot` instead of the procedural `make_mesh()` cube.
    /// When `None`, the procedural primitive is used as a fallback.
    pub fn scene_path(self) -> Option<&'static str> {
        match self {
            Form::Fence => Some("models/kenney_survival/fence.glb#Scene0"),
            Form::Floor => Some("models/kenney_survival/floor.glb#Scene0"),
            Form::Door => Some("models/kenney_survival/fence-doorway.glb#Scene0"),
            Form::Wall => Some("models/kenney_survival/structure-metal-wall.glb#Scene0"),
            Form::Lantern => Some("models/kenney_survival/campfire-pit.glb#Scene0"),
            Form::Bench => Some("models/kenney_survival/workbench.glb#Scene0"),
            Form::Table => Some("models/kenney_survival/workbench.glb#Scene0"),
            Form::Stew => Some("models/kenney_food/pot-stew.glb#Scene0"),
            // Raws and decor without a clean Kenney equivalent stay procedural for now.
            _ => None,
        }
    }

    /// Geometry used when this Form is placed/rendered in the world.
    /// Returns the mesh in its natural size; placement does not need to scale it.
    pub fn make_mesh(self) -> Mesh {
        match self {
            Form::Log => Mesh::from(Cylinder::new(0.18, 0.6)),
            Form::StoneChunk => Mesh::from(Cuboid::new(0.4, 0.3, 0.4)),
            Form::Flower => Mesh::from(Sphere::new(0.18)),
            Form::Mushroom => Mesh::from(Sphere::new(0.18)),
            Form::BushSprig => Mesh::from(Sphere::new(0.22)),
            Form::CactusFlesh => Mesh::from(Cuboid::new(0.3, 0.6, 0.3)),
            Form::Plank => Mesh::from(Cuboid::new(1.0, 0.08, 0.3)),
            Form::Brick => Mesh::from(Cuboid::new(0.4, 0.2, 0.2)),
            Form::Fence => Mesh::from(Cuboid::new(1.0, 0.6, 0.08)),
            Form::Bench => Mesh::from(Cuboid::new(1.0, 0.35, 0.4)),
            Form::Lantern => Mesh::from(Cylinder::new(0.1, 0.5)),
            Form::FlowerPot => Mesh::from(Cylinder::new(0.15, 0.25)),
            Form::Wreath => Mesh::from(Torus::new(0.05, 0.2)),
            Form::Chair => Mesh::from(Cuboid::new(0.5, 0.7, 0.5)),
            Form::Table => Mesh::from(Cuboid::new(1.1, 0.5, 0.7)),
            Form::Floor => Mesh::from(Cuboid::new(1.0, 0.12, 1.0)),
            Form::Wall => Mesh::from(Cuboid::new(1.0, 1.6, 0.15)),
            Form::Door => Mesh::from(Cuboid::new(0.9, 1.7, 0.12)),
            Form::Window => Mesh::from(Cuboid::new(0.9, 0.8, 0.12)),
            Form::Roof => Mesh::from(Cuboid::new(1.2, 0.18, 1.2)),
            Form::Stew => Mesh::from(Sphere::new(0.22)),
        }
    }
}
