use bevy::prelude::*;

use crate::crafting::CraftingState;
use leafwing_input_manager::prelude::ActionState;

use crate::input::{Action, CursorState};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{Form, ItemId, ItemRegistry, Material};
use crate::player::Player;
use crate::world::props::{GatheredCells, Prop, PropCell, PropKind};

pub struct GatheringPlugin;

impl Plugin for GatheringPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<GatherEvent>().add_systems(
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
    pub item: ItemId,
}

#[derive(Message)]
pub struct GatherEvent {
    pub entity: Entity,
    pub item: ItemId,
}

#[derive(Component)]
pub struct Gathering {
    pub timer: f32,
    pub item: ItemId,
}

fn prop_to_item(kind: &PropKind, registry: &ItemRegistry) -> Option<ItemId> {
    let (form, material) = match kind {
        PropKind::Tree => (Form::Log, Material::Oak),
        PropKind::PineTree => (Form::Log, Material::Pine),
        PropKind::Bush => (Form::BushSprig, Material::Bush),
        PropKind::Flower => (Form::Flower, Material::FlowerMix),
        PropKind::Mushroom => (Form::Mushroom, Material::Mushroom),
        PropKind::Cactus => (Form::CactusFlesh, Material::Cactus),
        PropKind::Rock | PropKind::Boulder => (Form::StoneChunk, Material::Stone),
        PropKind::DeadBush => (Form::Log, Material::Oak),
        PropKind::IceRock => (Form::StoneChunk, Material::Stone),
        PropKind::TundraGrass => return None,
    };
    registry.lookup(form, material)
}

fn detect_nearby_gatherables(
    mut commands: Commands,
    registry: Res<ItemRegistry>,
    player_query: Query<&GlobalTransform, With<Player>>,
    props: Query<(Entity, &GlobalTransform, &PropKind), (With<Prop>, Without<Gathering>)>,
) {
    let Ok(player_gt) = player_query.single() else {
        commands.remove_resource::<NearbyGatherable>();
        return;
    };
    let player_pos = player_gt.translation();

    let mut closest: Option<(Entity, f32, ItemId)> = None;

    for (entity, gt, kind) in &props {
        let Some(item) = prop_to_item(kind, &registry) else {
            continue;
        };

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
    action_state: Res<ActionState<Action>>,
    cursor: Res<CursorState>,
    nearby: Option<Res<NearbyGatherable>>,
    crafting: Res<CraftingState>,
    build_mode: Option<Res<crate::building::BuildMode>>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    mut gather_events: MessageWriter<GatherEvent>,
) {
    let Some(nearby) = nearby else { return };

    if crafting.open || build_mode.is_some() {
        return;
    }

    // Interact triggers from E (key/gamepad) or a left-click that the UI
    // focus pass already cleared as a world click.
    if action_state.just_pressed(&Action::Interact) || cursor.world_click {
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
    mut gathering: Query<(Entity, &mut Gathering, &mut Transform, Option<&PropCell>)>,
    time: Res<Time>,
    mut gathered: ResMut<GatheredCells>,
) {
    for (entity, mut gather, mut transform, cell) in &mut gathering {
        gather.timer += time.delta_secs();

        let progress = (gather.timer / GATHER_ANIM_DURATION).min(1.0);
        let scale = 1.0 - progress;
        transform.scale = Vec3::splat(scale.max(0.01));
        transform.translation.y += time.delta_secs() * 2.0;

        if progress >= 1.0 {
            // Mark this cell as gathered before despawning so chunk reloads
            // (and walking back into the same chunk) don't regrow the prop.
            if let Some(c) = cell {
                gathered.insert(*c);
            }
            commands.entity(entity).despawn();
        }
    }
}
