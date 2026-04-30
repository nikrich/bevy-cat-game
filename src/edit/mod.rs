use bevy::prelude::*;

pub mod highlight;
pub mod history;
pub mod placed_item;

pub use highlight::HighlightPlugin;
pub use history::{apply_redo, apply_undo, BuildOp, EditHistory, PieceRef};
pub use placed_item::PlacedItem;

pub struct EditPlugin;

impl Plugin for EditPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HighlightPlugin);
        history::register(app);
    }
}
