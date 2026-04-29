//! Reusable UI chrome built on Kenney's Adventure UI pack (PNG/Double).
//!
//! Each painted texture is 9-sliced so panels resize cleanly without distorting
//! corner ornaments. All sizing constants are calibrated for the actual asset
//! sizes documented inline.

use bevy::prelude::*;
use bevy::ui::widget::NodeImageMode;
use bevy_asset_loader::asset_collection::AssetCollection;

/// Eagerly-loaded UI chrome — fonts and the painted Kenney panels the HUD
/// builds on. W0.9 wires this through `bevy_asset_loader`'s `LoadingState`
/// so the resource is materialised before `OnEnter(GameState::Playing)`.
/// Per-prop glTFs continue to load lazily as chunks stream; only the small,
/// always-needed UI set is gated through the loading screen.
#[derive(AssetCollection, Resource, Clone)]
pub struct UiAssets {
    /// 128x128 painted parchment with carved-corner ornaments. The main panel chrome.
    #[asset(path = "ui/kenney/panel_brown_corners_a.png")]
    pub panel_bg: Handle<Image>,
    /// 128x128 darker brown panel, used for inset / nested surfaces (slot frames).
    #[asset(path = "ui/kenney/panel_brown_dark.png")]
    pub panel_dark: Handle<Image>,
    /// 96x48 brown button with subtle bevel. 9-sliced for tab + status pills.
    #[asset(path = "ui/kenney/button_brown.png")]
    pub button: Handle<Image>,
    /// 512x128 hanging banner with rope; sits on top of the panel as the title plate.
    #[asset(path = "ui/kenney/banner_hanging.png")]
    pub banner: Handle<Image>,
    /// 32x128 scroll-bar track; rendered vertically alongside scroll containers.
    #[asset(path = "ui/kenney/scrollbar_brown.png")]
    pub scrollbar: Handle<Image>,
    /// 96x128 brown hexagon, used as a number badge.
    #[asset(path = "ui/kenney/hexagon_brown.png")]
    pub hexagon: Handle<Image>,

    /// Hand-authored gold flourish curl that sits above the bottom hint line.
    #[asset(path = "ui/flourish.png")]
    pub flourish: Handle<Image>,
    /// Hand-authored gold gradient divider.
    #[asset(path = "ui/divider.png")]
    pub divider: Handle<Image>,

    #[asset(path = "fonts/Cinzel.ttf")]
    pub title_font: Handle<Font>,
    #[asset(path = "fonts/Nunito.ttf")]
    pub body_font: Handle<Font>,
}

// --- Palette (dark ink on parchment) -------------------------------------
// Body text is nearly black for legibility on the warm parchment panel.
// Title color is overridable so the banner can use white against the red plate.

pub const TEXT_TITLE: Color = Color::WHITE;
pub const TEXT_GOLD: Color = Color::srgb(0.55, 0.32, 0.08);
pub const TEXT_GOLD_DIM: Color = Color::srgba(0.55, 0.32, 0.08, 0.85);
pub const TEXT_BODY: Color = Color::srgb(0.08, 0.05, 0.02);
pub const TEXT_BODY_DIM: Color = Color::srgba(0.08, 0.05, 0.02, 0.78);
pub const TEXT_FAINT: Color = Color::srgba(0.08, 0.05, 0.02, 0.55);
pub const TEXT_DARK_INK: Color = Color::srgb(0.05, 0.03, 0.01);
pub const ACCENT_GOLD: Color = Color::srgb(0.86, 0.66, 0.30);
pub const NEED_RED: Color = Color::srgb(0.55, 0.10, 0.10);

// --- 9-slice helpers, calibrated to the Kenney asset sizes ---------------

/// 128x128 source -> 32px corner slices keeps the carved ornaments intact.
pub fn panel_image(handle: Handle<Image>) -> ImageNode {
    ImageNode {
        image: handle,
        image_mode: NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(32.0),
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        }),
        ..default()
    }
}

/// 128x128 dark inner panel; used for slot backgrounds.
pub fn slot_image(handle: Handle<Image>) -> ImageNode {
    ImageNode {
        image: handle,
        image_mode: NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(24.0),
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        }),
        ..default()
    }
}

/// 96x48 button with 16px corner bevels.
pub fn button_image(handle: Handle<Image>) -> ImageNode {
    ImageNode {
        image: handle,
        image_mode: NodeImageMode::Sliced(TextureSlicer {
            border: BorderRect::all(16.0),
            center_scale_mode: SliceScaleMode::Stretch,
            sides_scale_mode: SliceScaleMode::Stretch,
            max_corner_scale: 1.0,
        }),
        ..default()
    }
}

/// Plain (non-sliced) image -- for the banner, hexagon badge, scrollbar, etc.
pub fn plain_image(handle: Handle<Image>) -> ImageNode {
    ImageNode::new(handle)
}

// --- Text helpers ---------------------------------------------------------

pub fn title_text(assets: &UiAssets, text: &str, size: f32) -> impl Bundle {
    (
        Text::new(text.to_string()),
        TextFont {
            font: assets.title_font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(TEXT_TITLE),
    )
}

pub fn body_text(assets: &UiAssets, text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text.to_string()),
        TextFont {
            font: assets.body_font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
    )
}
