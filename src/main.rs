mod animals;
mod building;
mod camera;
mod crafting;
mod decoration;
mod edit;
mod gathering;
mod input;
mod inventory;
mod items;
mod memory;
mod particles;
mod player;
mod save;
mod state;
mod ui;
mod world;

use bevy::prelude::*;
use bevy::window::WindowResolution;
use bevy_egui::EguiPlugin;
use bevy_rapier3d::prelude::{NoUserData, RapierPhysicsPlugin};
use bevy_tnua::prelude::TnuaControllerPlugin;
use bevy_tnua_rapier3d::prelude::TnuaRapier3dPlugin;

use crate::player::ControlScheme;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cat World".into(),
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.54, 0.70, 0.52)))
        .add_plugins(EguiPlugin::default())
        // Physics + character controller. Rapier ticks in PostUpdate; the
        // tnua plugins ride along in Update so user input feeds the controller
        // before the physics step resolves the motor output (W0.3 + W0.4).
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(TnuaControllerPlugin::<ControlScheme>::new(Update))
        .add_plugins(TnuaRapier3dPlugin::new(Update))
        .add_plugins((
            state::StatePlugin,
            input::InputPlugin,
            items::ItemsPlugin,
            memory::MemoryPlugin,
            world::WorldPlugin,
            player::PlayerPlugin,
            camera::CameraPlugin,
            inventory::InventoryPlugin,
            gathering::GatheringPlugin,
            crafting::CraftingPlugin,
            edit::EditPlugin,
            building::BuildingPlugin,
            animals::AnimalPlugin,
            particles::ParticlePlugin,
            save::SavePlugin,
        ))
        // Separated from the tuple above: Bevy's add_plugins tuple cap is 15.
        .add_plugins(ui::GameUiPlugin)
        .add_plugins(decoration::DecorationPlugin)
        .run();
}
