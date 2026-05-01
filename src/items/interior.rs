//! Runtime catalog for the LowPoly Interior asset pack.
//!
//! The pack ships as two big GLBs (~1000 nodes total). Authoring 1000
//! `Form` enum variants is a non-starter, so each interior item lives as
//! a single `Form::Interior` registry entry with a stable `interior_name`
//! identifying its node inside the parent GLB.
//!
//! Two-stage population:
//! 1. **Startup**: the parent GLBs are queued for async loading, and we
//!    parse their JSON chunks synchronously to extract node names.
//!    Every interior item is registered in the `ItemRegistry` immediately
//!    so save/load and the placeables hotbar work from frame zero.
//! 2. **Async**: once the `Gltf` assets actually finish loading, a
//!    separate system in `crate::building` (`resolve_interior_spawns`)
//!    fills in the mesh + material on any entity carrying an
//!    `InteriorSpawnRequest` component.
//!
//! This split lets `spawn_placed_building` attach an interior request at
//! any time — the entity sits empty for the few frames between spawn and
//! Gltf-load completion.

use bevy::gltf::Gltf;
use bevy::prelude::*;
use std::collections::HashMap;

use super::registry::ItemRegistry;
use super::tags::ItemTags;

const INTERIOR_FIRST_PATH: &str = "models/lowpoly_interior/interior_1-500.glb";
const INTERIOR_SECOND_PATH: &str = "models/lowpoly_interior/interior_501-1000.glb";

pub struct InteriorPlugin;

impl Plugin for InteriorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InteriorCatalog>().add_systems(
            Startup,
            register_interior_items
                .after(super::registry::seed_default_items)
                .before(crate::building::init_placeable_items),
        );
    }
}

/// All interior items + handles to their parent GLB assets.
#[derive(Resource, Default)]
pub struct InteriorCatalog {
    pub gltf_first: Handle<Gltf>,
    pub gltf_second: Handle<Gltf>,
    pub items: Vec<InteriorItem>,
    /// `name -> index into items`. Used by spawn to look up source GLB.
    pub by_name: HashMap<String, usize>,
    /// `category -> indices into items`, sorted alphabetically by category.
    /// Used by the catalog UI for grouped display.
    pub categories: Vec<(String, Vec<usize>)>,
}

#[derive(Clone, Debug)]
pub struct InteriorItem {
    /// Node name in the parent GLB, e.g. `"armchair.008"`.
    pub name: String,
    /// Coarse category, derived by stripping the trailing `.NNN` suffix.
    pub category: String,
    pub source: InteriorSource,
    /// Pre-scale local-space AABB of the asset, computed at startup from
    /// the GLB's POSITION accessor min/max with the node's local TRS
    /// (rotation + scale, translation dropped) applied. Drives footprint
    /// calculation + bottom-of-AABB lift at placement time. `None` only if
    /// the GLB JSON parser failed to find bounds (item then defaults to a
    /// 1×1×1 footprint centred at the entity origin).
    pub aabb_local: Option<AabbBounds>,
}

/// Min/max corners of an axis-aligned bounding box in node-local space.
/// Plain Vec3 pair (instead of bevy's `Aabb`) so it stays serializable and
/// dependency-free for the catalog.
#[derive(Clone, Copy, Debug)]
pub struct AabbBounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl AabbBounds {
    pub fn size(self) -> Vec3 {
        self.max - self.min
    }
    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
    /// Round size.x and size.z up to integer cube cells. Used to pick the
    /// asset's footprint on the cube grid for snap-to-grid placement.
    pub fn footprint_cells(self, scale: f32) -> IVec2 {
        let s = self.size() * scale;
        IVec2::new(s.x.ceil() as i32, s.z.ceil() as i32).max(IVec2::splat(1))
    }
}

#[derive(Clone, Copy, Debug)]
pub enum InteriorSource {
    /// `interior_1-500.glb`
    First,
    /// `interior_501-1000.glb`
    Second,
}

impl InteriorCatalog {
    pub fn gltf_handle(&self, source: InteriorSource) -> &Handle<Gltf> {
        match source {
            InteriorSource::First => &self.gltf_first,
            InteriorSource::Second => &self.gltf_second,
        }
    }

    /// Look up an interior item's pre-scale AABB by node name. `None` if the
    /// name isn't in the catalog or the GLB parser couldn't extract bounds.
    pub fn aabb_for(&self, name: &str) -> Option<AabbBounds> {
        let idx = *self.by_name.get(name)?;
        self.items.get(idx)?.aabb_local
    }
}

pub fn register_interior_items(
    asset_server: Res<AssetServer>,
    mut catalog: ResMut<InteriorCatalog>,
    mut registry: ResMut<ItemRegistry>,
) {
    catalog.gltf_first = asset_server.load(INTERIOR_FIRST_PATH);
    catalog.gltf_second = asset_server.load(INTERIOR_SECOND_PATH);

    let mut items: Vec<InteriorItem> = Vec::new();
    let manifest_root = format!("{}/assets", env!("CARGO_MANIFEST_DIR"));

    for (asset_rel, source) in [
        (INTERIOR_FIRST_PATH, InteriorSource::First),
        (INTERIOR_SECOND_PATH, InteriorSource::Second),
    ] {
        let disk_path = format!("{}/{}", manifest_root, asset_rel);
        match read_glb_nodes(&disk_path) {
            Ok(parsed) => {
                for node in parsed {
                    let category = strip_index_suffix(&node.name);
                    items.push(InteriorItem {
                        name: node.name,
                        category,
                        source,
                        aabb_local: node.aabb,
                    });
                }
            }
            Err(err) => {
                warn!("[interior] failed to parse {}: {err}", asset_rel);
            }
        }
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));

    let tags = ItemTags::PLACEABLE | ItemTags::DECORATION | ItemTags::STACKABLE;
    let mut by_name = HashMap::new();
    let mut categories: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        registry.register_interior(item.name.clone(), item.category.clone(), tags);
        by_name.insert(item.name.clone(), idx);
        categories
            .entry(item.category.clone())
            .or_default()
            .push(idx);
    }

    let mut category_list: Vec<(String, Vec<usize>)> = categories.into_iter().collect();
    category_list.sort_by(|a, b| a.0.cmp(&b.0));

    let n = items.len();
    let cat_count = category_list.len();
    catalog.items = items;
    catalog.by_name = by_name;
    catalog.categories = category_list;

    info!(
        "[interior] registered {} items in {} categories",
        n, cat_count
    );
}

/// `"armchair.008"` -> `"armchair"`. Returns the input unchanged if there
/// is no `.NNN` suffix (so e.g. `"floor_lamp"` stays as one category).
fn strip_index_suffix(name: &str) -> String {
    if let Some((prefix, suffix)) = name.rsplit_once('.') {
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            return prefix.to_string();
        }
    }
    name.to_string()
}

struct ParsedNode {
    name: String,
    aabb: Option<AabbBounds>,
}

/// Pull node names + per-node AABBs out of a GLB's JSON chunk synchronously.
/// We want this at Startup so registration + footprint lookups complete
/// before save loading or the first placement; the async Gltf loader gives
/// us the same info eventually but can't be awaited from a startup system.
///
/// AABBs are computed by walking each node's mesh primitives, reading the
/// POSITION accessor's `min`/`max` (mandatory in glTF for that accessor),
/// transforming the 8 corners by the node's local TRS (rotation + scale —
/// translation is dropped because it represents the source scene's grid
/// layout, not intrinsic asset geometry), and unioning. The result is the
/// asset's bounding box in the orientation it'll be spawned with.
fn read_glb_nodes(disk_path: &str) -> Result<Vec<ParsedNode>, String> {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    let mut f = File::open(disk_path).map_err(|e| format!("open: {e}"))?;
    f.seek(SeekFrom::Start(12)).map_err(|e| format!("seek: {e}"))?;
    let mut len_bytes = [0u8; 4];
    f.read_exact(&mut len_bytes)
        .map_err(|e| format!("read len: {e}"))?;
    let chunk_len = u32::from_le_bytes(len_bytes) as usize;
    f.seek(SeekFrom::Current(4))
        .map_err(|e| format!("seek type: {e}"))?;
    let mut json_bytes = vec![0u8; chunk_len];
    f.read_exact(&mut json_bytes)
        .map_err(|e| format!("read json: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_slice(&json_bytes).map_err(|e| format!("parse: {e}"))?;

    let nodes = value
        .get("nodes")
        .and_then(|n| n.as_array())
        .ok_or_else(|| "no nodes array".to_string())?;
    let meshes = value.get("meshes").and_then(|m| m.as_array());
    let accessors = value.get("accessors").and_then(|a| a.as_array());

    let mut out = Vec::with_capacity(nodes.len());
    for n in nodes {
        let Some(name) = n.get("name").and_then(|n| n.as_str()).map(String::from) else {
            continue;
        };
        let aabb = node_aabb(n, meshes, accessors);
        out.push(ParsedNode { name, aabb });
    }
    Ok(out)
}

fn node_aabb(
    node: &serde_json::Value,
    meshes: Option<&Vec<serde_json::Value>>,
    accessors: Option<&Vec<serde_json::Value>>,
) -> Option<AabbBounds> {
    let mesh_idx = node.get("mesh").and_then(|m| m.as_u64())? as usize;
    let mesh = meshes?.get(mesh_idx)?;
    let primitives = mesh.get("primitives").and_then(|p| p.as_array())?;

    // Local rotation + scale from the node's TRS. Translation is intentionally
    // dropped (see fn doc) so the AABB describes the asset alone, not its
    // position in the source scene.
    let rotation = node
        .get("rotation")
        .and_then(|r| r.as_array())
        .and_then(|arr| {
            if arr.len() == 4 {
                Some(Quat::from_xyzw(
                    arr[0].as_f64()? as f32,
                    arr[1].as_f64()? as f32,
                    arr[2].as_f64()? as f32,
                    arr[3].as_f64()? as f32,
                ))
            } else {
                None
            }
        })
        .unwrap_or(Quat::IDENTITY);
    let scale = node
        .get("scale")
        .and_then(|s| s.as_array())
        .and_then(|arr| {
            if arr.len() == 3 {
                Some(Vec3::new(
                    arr[0].as_f64()? as f32,
                    arr[1].as_f64()? as f32,
                    arr[2].as_f64()? as f32,
                ))
            } else {
                None
            }
        })
        .unwrap_or(Vec3::ONE);

    let mut total: Option<AabbBounds> = None;
    for prim in primitives {
        let pos_idx = prim
            .get("attributes")
            .and_then(|a| a.get("POSITION"))
            .and_then(|p| p.as_u64())? as usize;
        let acc = accessors?.get(pos_idx)?;
        let min = acc.get("min").and_then(|m| m.as_array())?;
        let max = acc.get("max").and_then(|m| m.as_array())?;
        if min.len() != 3 || max.len() != 3 {
            continue;
        }
        let lo = Vec3::new(
            min[0].as_f64()? as f32,
            min[1].as_f64()? as f32,
            min[2].as_f64()? as f32,
        );
        let hi = Vec3::new(
            max[0].as_f64()? as f32,
            max[1].as_f64()? as f32,
            max[2].as_f64()? as f32,
        );
        // Transform the 8 corners by node TRS and refit the AABB. Required
        // because rotation can produce a tighter or looser AABB than just
        // rotating min/max independently.
        let corners = [
            Vec3::new(lo.x, lo.y, lo.z),
            Vec3::new(hi.x, lo.y, lo.z),
            Vec3::new(lo.x, hi.y, lo.z),
            Vec3::new(hi.x, hi.y, lo.z),
            Vec3::new(lo.x, lo.y, hi.z),
            Vec3::new(hi.x, lo.y, hi.z),
            Vec3::new(lo.x, hi.y, hi.z),
            Vec3::new(hi.x, hi.y, hi.z),
        ];
        let mut prim_min = Vec3::splat(f32::INFINITY);
        let mut prim_max = Vec3::splat(f32::NEG_INFINITY);
        for c in corners {
            let p = rotation * (c * scale);
            prim_min = prim_min.min(p);
            prim_max = prim_max.max(p);
        }
        total = Some(match total {
            Some(t) => AabbBounds {
                min: t.min.min(prim_min),
                max: t.max.max(prim_max),
            },
            None => AabbBounds { min: prim_min, max: prim_max },
        });
    }
    total
}
