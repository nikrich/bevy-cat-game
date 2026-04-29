use bevy::prelude::*;

use crate::crafting::CraftingState;
use crate::input::GameInput;
use crate::inventory::{Inventory, InventoryChanged, ItemKind};
use crate::player::Player;
use crate::world::props::{Prop, PropKind};

pub struct GatheringPlugin;

impl Plugin for GatheringPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GatherEvent>()
            .add_systems(
                Update,
                (detect_nearby_gatherables, gather_on_interact, animate_gathering),
            );
    }
}

const GATHER_RADIUS: f32 = 1.5;

#[derive(Resource)]
pub struct NearbyGatherable {
    pub entity: Entity,
    pub distance: f32,
    pub item: ItemKind,
}

#[derive(Event)]
pub struct GatherEvent {
    pub entity: Entity,
    pub item: ItemKind,
}

#[derive(Component)]
pub struct Gathering {
    pub timer: f32,
    pub item: ItemKind,
}

fn prop_to_item(kind: &PropKind) -> Option<ItemKind> {
    match kind {
        PropKind::Tree => Some(ItemKind::Wood),
        PropKind::PineTree => Some(ItemKind::PineWood),
        PropKind::Bush => Some(ItemKind::Bush),
        PropKind::Flower => Some(ItemKind::Flower),
        PropKind::Mushroom => Some(ItemKind::Mushroom),
        PropKind::Cactus => Some(ItemKind::Cactus),
        PropKind::Rock | PropKind::Boulder => Some(ItemKind::Stone),
        PropKind::DeadBush => Some(ItemKind::Wood),
        PropKind::IceRock => Some(ItemKind::Stone),
        PropKind::TundraGrass => None,
    }
}

fn detect_nearby_gatherables(
    mut commands: Commands,
    player_query: Query<&GlobalTransform, With<Player>>,
    props: Query<(Entity, &GlobalTransform, &PropKind), (With<Prop>, Without<Gathering>)>,
) {
    let Ok(player_gt) = player_query.single() else {
        commands.remove_resource::<NearbyGatherable>();
        return;
    };
    let player_pos = player_gt.translation();

    let mut closest: Option<(Entity, f32, ItemKind)> = None;

    for (entity, gt, kind) in &props {
        let Some(item) = prop_to_item(kind) else { continue };

        let pos = gt.translation();
        let dx = pos.x - player_pos.x;
        let dz = pos.z - player_pos.z;
        let dist = (dx * dx + dz * dz).sqrt();

        if dist < GATHER_RADIUS {
            if closest.is_none() || dist < closest.unwrap().1 {
                closest = Some((entity, dist, item));
            }
        }
    }

    match closest {
        Some((entity, distance, item)) => {
            commands.insert_resource(NearbyGatherable { entity, distance, item });
        }
        None => {
            commands.remove_resource::<NearbyGatherable>();
        }
    }
}

fn gather_on_interact(
    mut commands: Commands,
    input: Res<GameInput>,
    nearby: Option<Res<NearbyGatherable>>,
    crafting: Res<CraftingState>,
    build_mode: Option<Res<crate::building::BuildMode>>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: EventWriter<InventoryChanged>,
    mut gather_events: EventWriter<GatherEvent>,
) {
    let Some(nearby) = nearby else { return };

    // Don't gather when other menus are active
    if crafting.open || build_mode.is_some() {
        return;
    }

    if input.interact {
        inventory.add(nearby.item, 1);
        let new_count = inventory.count(nearby.item);

        inv_events.write(InventoryChanged { item: nearby.item, new_count });
        gather_events.write(GatherEvent { entity: nearby.entity, item: nearby.item });

        commands.entity(nearby.entity).insert(Gathering {
            timer: 0.0,
            item: nearby.item,
        });
        commands.remove_resource::<NearbyGatherable>();
    }
}

const GATHER_ANIM_DURATION: f32 = 0.3;

fn animate_gathering(
    mut commands: Commands,
    mut gathering: Query<(Entity, &mut Gathering, &mut Transform)>,
    time: Res<Time>,
) {
    for (entity, mut gather, mut transform) in &mut gathering {
        gather.timer += time.delta_secs();

        let progress = (gather.timer / GATHER_ANIM_DURATION).min(1.0);
        let scale = 1.0 - progress;
        transform.scale = Vec3::splat(scale.max(0.01));
        transform.translation.y += time.delta_secs() * 2.0;

        if progress >= 1.0 {
            commands.entity(entity).despawn();
        }
    }
}
