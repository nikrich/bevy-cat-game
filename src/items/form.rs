use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Placement interaction model for a `Form`. Drives the routing decision
/// in `building::place_building` and `building::update_preview` — the
/// alternative would be hardcoded `matches!(form, Form::Wall)` checks
/// scattered through both systems, which doesn't extend to the next batch
/// of forms (fences, floor tiling, door-into-wall replacement, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementStyle {
    /// One click = one piece. Cursor-aware face-based stacking.
    /// Used by all furniture, decorations, and the unfinished Door/Window
    /// (which will move to `Replace` in Stage 2 of Phase 2).
    Single,
    /// Two-click line tool with a continuous-mode anchor. First click sets
    /// the anchor; subsequent clicks fill cells from anchor to cursor and
    /// advance the anchor to the last placed cube. Used by `Form::Wall`
    /// today; future fences / floor-tile chains slot in here.
    Line,
    /// Click-and-hold paint. Each frame the mouse is held, a piece is
    /// stamped at the cursor cell (skipping cells that already have a
    /// piece). The whole drag becomes one undo entry. Used by `Form::Floor`
    /// so paving a room feels like painting tiles instead of clicking each.
    Paint,
    /// Click an existing wall to swap it for this piece. Refunds the wall's
    /// item to inventory. Used by `Form::Door` and `Form::Window` — opens
    /// a door / window in a built wall instead of forcing the player to
    /// remove the wall first then place a 1-cell-wide piece by hand.
    Replace,
}

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

    // Asset-backed decorations (kenney_survival, kaykit_restaurant)
    Bed,
    Chest,
    Campfire,
    Barrel,
    Bucket,

    // Modular building
    Floor,
    Wall,
    Door,
    Window,
    Roof,

    // Consumables
    Stew,

    /// Marker variant for items resolved through `InteriorCatalog`. The
    /// actual mesh + material come from a runtime-loaded GLB node lookup
    /// keyed off `ItemDef::interior_name` — this variant carries no payload
    /// itself so `Form` stays `Copy`. Used for the LowPoly Interior pack
    /// (~1000 nodes) without authoring 1000 enum variants.
    Interior,
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
            Form::Bed => "Bed",
            Form::Chest => "Chest",
            Form::Campfire => "Campfire",
            Form::Barrel => "Barrel",
            Form::Bucket => "Bucket",
            Form::Floor => "Floor",
            Form::Wall => "Wall",
            Form::Door => "Door",
            Form::Window => "Window",
            Form::Roof => "Roof",
            Form::Stew => "Stew",
            Form::Interior => "Decoration",
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
            Form::Bed => "bed",
            Form::Chest => "chest",
            Form::Campfire => "campfire",
            Form::Barrel => "barrel",
            Form::Bucket => "bucket",
            Form::Floor => "floor",
            Form::Wall => "wall",
            Form::Door => "door",
            Form::Window => "window",
            Form::Roof => "roof",
            Form::Stew => "stew",
            Form::Interior => "interior",
        }
    }

    /// Pre-rendered preview PNG used as the UI icon for this Form. Building
    /// placeables intentionally fall through to the procedural shape swatch
    /// (see `icon_shape()`) since the procedural primitive doesn't match the
    /// Kenney 3D preview. Raws + Stew keep their Kenney photos.
    pub fn icon_path(self) -> Option<&'static str> {
        match self {
            // Raws gathered from the world -- Kenney photos match the prop.
            Form::Log => Some("ui/icons/survival/tree-log.png"),
            Form::StoneChunk => Some("ui/icons/survival/rock-a.png"),
            Form::Mushroom => Some("ui/icons/food/mushroom.png"),
            Form::BushSprig => Some("ui/icons/survival/grass-large.png"),

            // Refined materials.
            Form::Plank => Some("ui/icons/survival/resource-planks.png"),
            Form::Brick => Some("ui/icons/survival/resource-stone.png"),

            // Stew is food, fine to keep the pot icon.
            Form::Stew => Some("ui/icons/food/pot-stew.png"),

            // Everything placeable falls back to a procedural shape swatch.
            _ => None,
        }
    }

    /// Returns `(width_norm, height_norm, corner_radius_norm)` describing the
    /// rough silhouette of the procedural primitive (from `make_mesh()`),
    /// normalised to a unit box. UI swatches scale this into their slot so a
    /// Wall reads as a tall thin rectangle, a Floor as a flat wide one, a
    /// Lantern as a tall capsule, etc.
    pub fn icon_shape(self) -> (f32, f32, f32) {
        match self {
            // (w, h, radius_frac of min(w,h))
            Form::Floor => (1.0, 0.18, 0.10),
            Form::Wall => (1.0, 1.0, 0.05),
            Form::Door => (0.50, 1.0, 0.15),
            Form::Window => (0.95, 0.85, 0.18),
            Form::Roof => (1.0, 0.20, 0.08),
            Form::Fence => (1.0, 0.55, 0.10),
            Form::Bench => (1.0, 0.40, 0.18),
            Form::Lantern => (0.30, 1.0, 0.50),    // tall capsule
            Form::Table => (1.0, 0.55, 0.18),
            Form::Chair => (0.55, 0.90, 0.18),
            Form::FlowerPot => (0.55, 0.55, 0.45), // round pot
            Form::Wreath => (0.85, 0.85, 0.50),    // ring
            Form::Bed => (1.0, 0.40, 0.10),
            Form::Chest => (0.85, 0.55, 0.12),
            Form::Campfire => (0.85, 0.30, 0.40),
            Form::Barrel => (0.55, 0.95, 0.50),
            Form::Bucket => (0.50, 0.55, 0.30),
            _ => (1.0, 1.0, 0.20),
        }
    }

    /// glTF/GLB scene path for placed-building rendering. When `Some`,
    /// `spawn_placed_building` loads the scene; otherwise it falls back to
    /// the procedural mesh from `make_mesh()`. Decorations + furniture
    /// route to hand-authored Kenney / Kaykit models so they read as
    /// actual furniture instead of cuboid stand-ins. Cubes and structural
    /// pieces stay procedural — they're geometric primitives, by design.
    pub fn scene_path(self) -> Option<&'static str> {
        match self {
            Form::Bed => Some("models/kenney_survival/bedroll.glb#Scene0"),
            Form::Chest => Some("models/kenney_survival/chest.glb#Scene0"),
            Form::Campfire => Some("models/kenney_survival/campfire-pit.glb#Scene0"),
            Form::Barrel => Some("models/kenney_survival/barrel.glb#Scene0"),
            Form::Bucket => Some("models/kenney_survival/bucket.glb#Scene0"),
            Form::Chair => Some("models/kaykit_restaurant/chair_A.gltf#Scene0"),
            Form::Table => Some("models/kaykit_restaurant/table_round_A.gltf#Scene0"),
            _ => None,
        }
    }

    /// SceneRoot scale applied at placement. Interior items get 2× because
    /// the LowPoly Interior pack ships at roughly half cube-grid scale —
    /// without the bump a "bed" reads as a dollhouse prop. Footprint snap
    /// uses this same scale (see `AabbBounds::footprint_cells`) so the
    /// no-overlap grid stays consistent with what's drawn.
    pub fn placement_scale(self) -> f32 {
        match self {
            Form::Interior => 2.0,
            _ => 1.0,
        }
    }

    /// World-space distance from the spawn origin to the bottom of the visible
    /// mesh -- i.e. set translation.y = ground + placement_lift() and the model
    /// will look like it's resting on the ground. Calibrated to `make_mesh()`
    /// dimensions (centre-origin cuboids, so half the model height).
    pub fn placement_lift(self) -> f32 {
        match self {
            Form::Floor => 0.06,
            Form::Wall => 0.50,
            // Doors and windows replace a wall cube exactly: same lift +
            // height = 0.5 / 1.0 means they sit dead-centre in the wall slot
            // so the surrounding cubes stay aligned. Visual frame is built
            // by `spawn_door_composite` / `spawn_window_composite` to fit
            // inside the 1×1×0.18 footprint.
            Form::Door => 0.50,
            Form::Window => 0.50,
            Form::Roof => 0.09,
            Form::Fence => 0.30,
            Form::Bench => 0.175,
            Form::Lantern => 0.25,
            Form::Table => 0.25,
            Form::Chair => 0.35,
            Form::FlowerPot => 0.125,
            Form::Wreath => 0.10,
            // Asset-backed forms — heights are estimates from the source
            // GLBs; tweak when they don't sit flush after first placetest.
            Form::Bed => 0.15,
            Form::Chest => 0.30,
            Form::Campfire => 0.10,
            Form::Barrel => 0.50,
            Form::Bucket => 0.20,
            Form::Stew => 0.22,
            // Interior items: per-item AABB-derived lift would be ideal,
            // but lacking that, 0.5 keeps medium furniture roughly on the
            // ground. Some items will float or sink — tune in a polish pass.
            Form::Interior => 0.5,
            _ => 0.05,
        }
    }

    /// How the build system should interact with this form. See
    /// [`PlacementStyle`] for the routing semantics.
    pub fn placement_style(self) -> PlacementStyle {
        match self {
            Form::Wall => PlacementStyle::Line,
            Form::Floor => PlacementStyle::Paint,
            Form::Door | Form::Window => PlacementStyle::Replace,
            _ => PlacementStyle::Single,
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
            Form::Wall => Mesh::from(Cuboid::new(1.0, 1.0, 1.0)),
            // Ghost only: the actual placed door/window spawns as a
            // composite frame (see `spawn_door_composite`,
            // `spawn_window_composite`). The 1×1×0.18 cuboid here shows
            // the player exactly which wall slot the piece will land in.
            Form::Door => Mesh::from(Cuboid::new(1.0, 1.0, 0.18)),
            Form::Window => Mesh::from(Cuboid::new(1.0, 1.0, 0.18)),
            Form::Roof => Mesh::from(Cuboid::new(1.2, 0.18, 1.2)),
            // Asset-backed forms render via SceneRoot (see scene_path);
            // these procedural fallbacks are only used if asset loading
            // fails. Tuned to roughly match the source models so a
            // missing-asset frame doesn't look catastrophic.
            Form::Bed => Mesh::from(Cuboid::new(1.0, 0.3, 2.0)),
            Form::Chest => Mesh::from(Cuboid::new(0.8, 0.6, 0.5)),
            Form::Campfire => Mesh::from(Cylinder::new(0.45, 0.2)),
            Form::Barrel => Mesh::from(Cylinder::new(0.3, 1.0)),
            Form::Bucket => Mesh::from(Cylinder::new(0.2, 0.4)),
            Form::Stew => Mesh::from(Sphere::new(0.22)),
            // Interior items resolve their mesh asynchronously via the
            // catalog; this fallback shows up only if the parent GLB
            // failed to load.
            Form::Interior => Mesh::from(Cuboid::new(0.6, 0.6, 0.6)),
        }
    }
}

