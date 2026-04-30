//! Decoration catalog panel -- right-side egui window listing all
//! DECORATION + FURNITURE placeables grouped by category.
//!
//! Extracted from `building/ui.rs` so it can later move to the
//! `decoration` domain without touching the structural hotbar code.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::items::{ItemRegistry, ItemTags};
use crate::building::PlaceableItems;
use super::DecorationMode;
use super::DecorationTool;
use crate::building::ui::{GOLD, GOLD_DIM, TEXT_DIM, panel_frame};

/// Per-category open/closed UI state.  Persists between frames so a
/// player who collapses a section keeps it collapsed.
#[derive(Resource, Default)]
pub struct DecorationCatalogState {
    /// Per-category open/closed UI state. Persists between frames so a
    /// player who collapses a section keeps it collapsed.
    pub open: std::collections::HashMap<String, bool>,
    pub search: String,
    /// Cached `Handle<Image>` per interior item name. Populated lazily on
    /// first display so we don't load 1000 PNGs upfront. The loader queues
    /// the file via `asset_server.load`; the egui texture id is fetched
    /// each frame via `EguiContexts::image_id` once the asset is ready.
    pub thumb_handles: std::collections::HashMap<String, Handle<Image>>,
}

pub(super) const THUMB_SIZE: f32 = 56.0;

pub fn draw_decoration_catalog(
    mut contexts: EguiContexts,
    decoration_mode: Option<ResMut<DecorationMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut state: Local<DecorationCatalogState>,
) -> Result {
    let Some(mut mode) = decoration_mode else { return Ok(()) };

    // Group decoration placeables by category. "Other" collects the
    // hand-authored items that don't carry an interior_category (Bed,
    // Lantern, etc.). Sort categories alphabetically; "Other" pinned
    // first because it has the most-recognisable names. We also collect
    // each item's `interior_name` here so the inner loop can look up the
    // pre-baked thumbnail without having to re-query the registry.
    struct CatalogRow<'a> {
        idx: usize,
        display: &'a str,
        interior_name: Option<&'a str>,
    }
    let mut groups: std::collections::HashMap<String, Vec<CatalogRow<'_>>> = std::collections::HashMap::new();
    for (idx, item_id) in placeables.0.iter().enumerate() {
        let Some(def) = registry.get(*item_id) else { continue };
        let is_decor = def.tags.contains(ItemTags::DECORATION)
            || def.tags.contains(ItemTags::FURNITURE);
        if !is_decor {
            continue;
        }
        // Hand-authored decorations (Bed, Lantern, Chest, etc.) have no
        // `interior_category` and no thumbnail. Skip them -- the grid is
        // for the LowPoly Interior pack only. Move them to a different
        // panel later if they need to come back.
        let Some(cat) = def.interior_category.clone() else { continue };
        groups.entry(cat).or_default().push(CatalogRow {
            idx,
            display: def.display_name.as_str(),
            interior_name: def.interior_name.as_deref(),
        });
    }
    let mut sorted_groups: Vec<(String, Vec<CatalogRow<'_>>)> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| a.0.cmp(&b.0));

    // Pre-resolve egui texture IDs for every interior item that *might*
    // get drawn this frame. We can't call `contexts.add_image` once we've
    // borrowed `ctx_mut()`, so this pass goes first. Lazy: a thumbnail is
    // only loaded the first time an item shows up in `placeables`. The
    // `image_id` lookup returns `Some` once the PNG has loaded into
    // `Assets<Image>` -- until then the row falls back to a text-only
    // button so the catalog never blocks on disk I/O.
    let search_lower = state.search.to_lowercase();
    let has_filter = !search_lower.is_empty();
    let mut thumb_ids: std::collections::HashMap<usize, egui::TextureId> =
        std::collections::HashMap::new();
    for (_, items) in &sorted_groups {
        for row in items {
            let Some(interior_name) = row.interior_name else { continue };
            if has_filter && !row.display.to_lowercase().contains(&search_lower) {
                continue;
            }
            let handle = state
                .thumb_handles
                .entry(interior_name.to_string())
                .or_insert_with(|| {
                    asset_server.load(format!("thumbnails/{}.png", interior_name))
                })
                .clone();
            // Texture must be registered with egui at least once before
            // it can be sampled. `add_image` is idempotent for repeated
            // strong handles, but we use `image_id` to avoid the work
            // when it's already known.
            let id = contexts.image_id(&handle).unwrap_or_else(|| {
                contexts.add_image(bevy_egui::EguiTextureHandle::Strong(handle))
            });
            thumb_ids.insert(row.idx, id);
        }
    }
    let ctx = contexts.ctx_mut()?;

    egui::Window::new("build_decoration_catalog")
        .anchor(egui::Align2::RIGHT_TOP, [-16.0, 80.0])
        .collapsible(false)
        .resizable(true)
        .default_width(280.0)
        .min_width(220.0)
        .max_width(360.0)
        .default_height(520.0)
        .title_bar(false)
        .frame(panel_frame())
        .show(ctx, |ui| {
            ui.set_max_height(520.0);
            ui.colored_label(GOLD, "Decorations");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.colored_label(GOLD_DIM, "🔎");
                ui.add(
                    egui::TextEdit::singleline(&mut state.search)
                        .desired_width(160.0)
                        .hint_text("filter…"),
                );
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for (cat, items) in sorted_groups {
                        // Filter items first; skip empty categories.
                        let visible: Vec<&CatalogRow<'_>> = items
                            .iter()
                            .filter(|r| {
                                !has_filter || r.display.to_lowercase().contains(&search_lower)
                            })
                            .collect();
                        if visible.is_empty() {
                            continue;
                        }

                        let header = format!("{} ({})", category_label(&cat), visible.len());
                        // When a filter is active, force categories open
                        // so the player can see matches without expanding.
                        let default_open = has_filter;
                        let entry = state.open.entry(cat.clone()).or_insert(default_open);
                        if has_filter {
                            *entry = true;
                        }
                        let resp = egui::CollapsingHeader::new(
                            egui::RichText::new(header).color(GOLD).strong(),
                        )
                        .default_open(*entry)
                        .open(if has_filter { Some(true) } else { None })
                        .show(ui, |ui| {
                            // Wrapping grid of thumbnail buttons -- egui
                            // re-flows them based on available width when
                            // the player resizes the catalog.
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for row in &visible {
                                    let active = row.idx == mode.selected
                                        && mode.tool == DecorationTool::Place;
                                    let response = if let Some(tex_id) =
                                        thumb_ids.get(&row.idx).copied()
                                    {
                                        let img = egui::Image::new(
                                            egui::load::SizedTexture::new(
                                                tex_id,
                                                [THUMB_SIZE, THUMB_SIZE],
                                            ),
                                        );
                                        ui.add(
                                            egui::ImageButton::new(img).selected(active),
                                        )
                                    } else {
                                        // Thumbnail not loaded yet: text
                                        // fallback so the slot is still
                                        // selectable. Sized to match a
                                        // thumbnail so wrapping is stable.
                                        let text = if active {
                                            egui::RichText::new(row.display)
                                                .color(GOLD)
                                                .strong()
                                        } else {
                                            egui::RichText::new(row.display).color(TEXT_DIM)
                                        };
                                        ui.add_sized(
                                            [THUMB_SIZE, THUMB_SIZE],
                                            egui::Button::new(text),
                                        )
                                    };
                                    let response = response.on_hover_text(row.display);
                                    if response.clicked() {
                                        mode.tool = DecorationTool::Place;
                                        mode.selected = row.idx;
                                    }
                                }
                            });
                        });
                        // Persist user's collapse state when no filter.
                        if !has_filter {
                            *state.open.entry(cat).or_insert(default_open) =
                                resp.fully_open();
                        }
                    }
                });
        });

    Ok(())
}

/// `"floor_lamp"` -> `"Floor Lamp"`. Just a Title-Case prettifier.
fn category_label(raw: &str) -> String {
    raw.replace('_', " ")
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f
                    .to_uppercase()
                    .chain(c.flat_map(char::to_lowercase))
                    .collect::<String>(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
