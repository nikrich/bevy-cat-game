use bevy::prelude::*;

pub mod highlight;
pub mod history;
pub mod placed_item;

pub use highlight::HighlightPlugin;
// pub use history::{...} and pub use placed_item::PlacedItem land in Tasks 2 and 4.

pub struct EditPlugin;

impl Plugin for EditPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HighlightPlugin);
        // history::register lands in Task 2.
    }
}
