use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{Form, ItemTags, Material};

/// Opaque numeric handle into `ItemRegistry`. Session-local; saves use
/// `ItemDef::save_key` instead so registry rebuilds remain compatible.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemId(pub u32);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ItemDef {
    pub id: ItemId,
    pub form: Form,
    pub material: Material,
    pub tags: ItemTags,
    pub display_name: String,
    pub save_key: String,
}

#[derive(Resource, Default)]
pub struct ItemRegistry {
    defs: Vec<ItemDef>,
    by_pair: HashMap<(Form, Material), ItemId>,
    by_save_key: HashMap<String, ItemId>,
}

impl ItemRegistry {
    pub fn register(&mut self, form: Form, material: Material, tags: ItemTags) -> ItemId {
        if let Some(id) = self.by_pair.get(&(form, material)) {
            return *id;
        }

        let id = ItemId(self.defs.len() as u32);
        let adj = material.display_adjective();
        let noun = form.display_noun();
        let display_name = if matches!(material, Material::None) || adj.is_empty() {
            noun.to_string()
        } else if adj == noun {
            // Avoid "Stone Stone" / "Mushroom Mushroom" -- raw material drops
            // the redundant adjective.
            noun.to_string()
        } else {
            format!("{adj} {noun}")
        };
        let save_key = format!("{}.{}", form.save_key(), material.save_key());

        let def = ItemDef {
            id,
            form,
            material,
            tags,
            display_name,
            save_key: save_key.clone(),
        };

        self.by_pair.insert((form, material), id);
        self.by_save_key.insert(save_key, id);
        self.defs.push(def);
        id
    }

    pub fn get(&self, id: ItemId) -> Option<&ItemDef> {
        self.defs.get(id.0 as usize)
    }

    pub fn lookup(&self, form: Form, material: Material) -> Option<ItemId> {
        self.by_pair.get(&(form, material)).copied()
    }

    pub fn lookup_save_key(&self, key: &str) -> Option<ItemId> {
        self.by_save_key.get(key).copied()
    }

    pub fn all(&self) -> &[ItemDef] {
        &self.defs
    }

    pub fn iter_with_tag(&self, tag: ItemTags) -> impl Iterator<Item = &ItemDef> {
        self.defs.iter().filter(move |d| d.tags.contains(tag))
    }
}

/// Populate the registry with the starter catalog.
pub fn seed_default_items(mut registry: ResMut<ItemRegistry>) {
    let raw = ItemTags::RAW | ItemTags::STACKABLE;
    let refined = ItemTags::REFINED | ItemTags::STACKABLE;
    let furniture = ItemTags::PLACEABLE | ItemTags::FURNITURE | ItemTags::STACKABLE;
    let structural = ItemTags::PLACEABLE | ItemTags::STRUCTURAL | ItemTags::STACKABLE;
    let food = ItemTags::FOOD | ItemTags::STACKABLE;
    let decoration = ItemTags::PLACEABLE | ItemTags::DECORATION | ItemTags::STACKABLE;

    let pairs: &[(Form, Material, ItemTags)] = &[
        // Raws
        (Form::Log, Material::Oak, raw),
        (Form::Log, Material::Pine, raw),
        (Form::StoneChunk, Material::Stone, raw),
        (Form::Flower, Material::FlowerMix, raw),
        (Form::Mushroom, Material::Mushroom, raw),
        (Form::BushSprig, Material::Bush, raw),
        (Form::CactusFlesh, Material::Cactus, raw),
        // Refined
        (Form::Plank, Material::Oak, refined),
        (Form::Plank, Material::Pine, refined),
        (Form::Plank, Material::Birch, refined),
        (Form::Brick, Material::Stone, refined),
        (Form::Brick, Material::Sandstone, refined),
        // Furniture
        (Form::Fence, Material::Oak, furniture),
        (Form::Fence, Material::Pine, furniture),
        (Form::Bench, Material::Oak, furniture),
        (Form::Lantern, Material::Stone, furniture),
        (Form::FlowerPot, Material::Stone, furniture),
        (Form::Chair, Material::Oak, furniture),
        (Form::Chair, Material::Pine, furniture),
        (Form::Table, Material::Oak, furniture),
        (Form::Wreath, Material::None, decoration),
        // Structural
        (Form::Floor, Material::Oak, structural),
        (Form::Floor, Material::Pine, structural),
        (Form::Floor, Material::Stone, structural),
        (Form::Wall, Material::Oak, structural),
        (Form::Wall, Material::Pine, structural),
        (Form::Wall, Material::Stone, structural),
        (Form::Wall, Material::Brick, structural),
        (Form::Door, Material::Oak, structural),
        // Food
        (Form::Stew, Material::None, food),
    ];

    for (form, material, tags) in pairs {
        registry.register(*form, *material, *tags);
    }

    info!("ItemRegistry seeded with {} items", registry.defs.len());
}
