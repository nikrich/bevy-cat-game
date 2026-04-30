//! Item registry: combinatorial Form x Material item system.
//!
//! Forms are visual archetypes (Plank, Chair, Wall, ...). Materials are
//! palette + properties (Pine, Oak, Stone, ...). Each `(Form, Material)`
//! pair becomes a concrete `ItemDef` keyed by an `ItemId`.
//!
//! Recipes are templated against `MaterialFamily` so one "Chair" recipe
//! covers Pine Chair / Oak Chair / Birch Chair, resolved on craft.

pub mod form;
pub mod material;
pub mod registry;
pub mod tags;

use bevy::prelude::*;

pub use form::{Form, PlacementStyle, SnapMode};
pub use material::{Material, MaterialFamily};
pub use registry::{ItemDef, ItemId, ItemRegistry};
pub use tags::ItemTags;

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemRegistry>()
            .add_systems(Startup, registry::seed_default_items);
    }
}
