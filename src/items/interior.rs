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
use crate::building::PlaceableItems;

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
        match read_glb_node_names(&disk_path) {
            Ok(names) => {
                for name in names {
                    let category = strip_index_suffix(&name);
                    items.push(InteriorItem { name, category, source });
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

/// Pull node names out of a GLB's JSON chunk synchronously. We want this
/// at Startup so registration completes before save loading runs; the
/// async Gltf loader gives us the same info eventually but we can't wait
/// on it for registration ordering.
fn read_glb_node_names(disk_path: &str) -> Result<Vec<String>, String> {
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
    Ok(nodes
        .iter()
        .filter_map(|n| n.get("name").and_then(|n| n.as_str()).map(String::from))
        .collect())
}
