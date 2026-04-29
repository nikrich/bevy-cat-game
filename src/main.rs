mod animals;
mod building;
mod camera;
mod crafting;
mod gathering;
mod input;
mod inventory;
mod items;
mod memory;
mod particles;
mod player;
mod save;
mod ui;
mod world;

use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cat World".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.54, 0.70, 0.52)))
        .add_plugins((
            input::InputPlugin,
            items::ItemsPlugin,
            memory::MemoryPlugin,
            world::WorldPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            inventory::InventoryPlugin,
            gathering::GatheringPlugin,
            crafting::CraftingPlugin,
            building::BuildingPlugin,
            animals::AnimalPlugin,
            particles::ParticlePlugin,
            save::SavePlugin,
            ui::GameUiPlugin,
        ))
        .run();
}
