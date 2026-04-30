//! Build mode tool palette (mirrors `world::edit_egui` in shape).
//!
//! Bottom-centre egui panel showing the active `BuildTool`, hotkey labels,
//! and (for Place) the currently selected placeable plus the `[ / ]`
//! cycle hint. Hidden when build mode is off.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

use crate::edit::{apply_redo, apply_undo, EditHistory};
use crate::edit::PlacedItem;
use super::{
    refresh_build_preview, BuildMode, BuildTool, PlaceableItems,
};
use crate::inventory::{Inventory, InventoryChanged};
use crate::items::{InteriorCatalog, ItemRegistry, ItemTags};

const PARCHMENT: egui::Color32 = egui::Color32::from_rgb(54, 38, 24);
const GOLD: egui::Color32 = egui::Color32::from_rgb(220, 168, 76);
const GOLD_DIM: egui::Color32 = egui::Color32::from_rgb(140, 105, 50);
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(172, 158, 130);

pub fn register(app: &mut App) {
    app.add_systems(
        EguiPrimaryContextPass,
        (draw_build_tool_hotbar, draw_decoration_catalog),
    );
}

fn panel_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(PARCHMENT)
        .stroke(egui::Stroke::new(2.0, GOLD))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .corner_radius(egui::CornerRadius::same(6))
}

#[allow(clippy::too_many_arguments)]
fn draw_build_tool_hotbar(
    mut contexts: EguiContexts,
    build_mode: Option<Res<BuildMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    mut history: ResMut<EditHistory>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut inventory: ResMut<Inventory>,
    mut inv_events: MessageWriter<InventoryChanged>,
    placed_q: Query<(Entity, &Transform, &PlacedItem)>,
    catalog: Res<InteriorCatalog>,
    mut indoor_settings: ResMut<crate::camera::occluder_fade::IndoorRevealSettings>,
) -> Result {
    let Some(mode) = build_mode else { return Ok(()) };
    let ctx = contexts.ctx_mut()?;
    let can_undo = history.can_undo();
    let can_redo = history.can_redo();

    egui::Window::new("build_tool_hotbar")
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -16.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(panel_frame())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, &tool) in BuildTool::ALL.iter().enumerate() {
                    let active = tool == mode.tool;
                    let label = format!("{}  {}", i + 1, tool.label());
                    let text = if active {
                        egui::RichText::new(label).color(GOLD).strong()
                    } else {
                        egui::RichText::new(label).color(TEXT_DIM)
                    };
                    ui.add(egui::Label::new(text).selectable(false));
                }

                ui.separator();

                match mode.tool {
                    BuildTool::Place => {
                        let item_label = mode
                            .selected_item(&placeables)
                            .and_then(|id| registry.get(id))
                            .map(|d| d.display_name.as_str())
                            .unwrap_or("(none)");
                        ui.colored_label(GOLD, format!("piece: {}", item_label));
                        ui.colored_label(GOLD_DIM, "[ / ]   shift+click = line");
                    }
                    BuildTool::Remove => {
                        ui.colored_label(GOLD_DIM, "click a placed cube to remove");
                    }
                }

                ui.separator();

                let undo_btn = egui::Button::new(
                    egui::RichText::new("⟲ Undo").color(if can_undo { GOLD } else { TEXT_DIM }),
                );
                if ui.add_enabled(can_undo, undo_btn).clicked() {
                    apply_undo(
                        &mut history,
                        &mut commands,
                        &registry,
                        &asset_server,
                        &mut meshes,
                        &mut materials,
                        &mut inventory,
                        &mut inv_events,
                        &catalog,
                    );
                }
                let redo_btn = egui::Button::new(
                    egui::RichText::new("⟳ Redo").color(if can_redo { GOLD } else { TEXT_DIM }),
                );
                if ui.add_enabled(can_redo, redo_btn).clicked() {
                    apply_redo(
                        &mut history,
                        &mut commands,
                        &registry,
                        &asset_server,
                        &mut meshes,
                        &mut materials,
                        &mut inventory,
                        &mut inv_events,
                        &placed_q,
                        &catalog,
                    );
                }
                ui.colored_label(GOLD_DIM, "Ctrl+Z / Ctrl+Shift+Z");

                ui.separator();

                // Indoor reveal controls — let the player toggle the
                // ceiling-fade effect off (e.g. while admiring the
                // exterior) and tweak how see-through it is when on.
                ui.checkbox(&mut indoor_settings.enabled, "X-ray (X)");
                ui.add_enabled(
                    indoor_settings.enabled,
                    egui::Slider::new(&mut indoor_settings.alpha, 0.0..=1.0)
                        .show_value(false)
                        .text("α"),
                );
            });
        });

    Ok(())
}

/// Right-side decoration catalog — DECORATION + FURNITURE placeables
/// grouped by category. Handles ~1000 items via collapsing category
/// headers and a scrollable area. A text-search box filters by display
/// name. "Other" group at the top holds the hand-authored decorations
/// (Bed, Chest, Lantern, etc.) that aren't part of the interior catalog.
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

const THUMB_SIZE: f32 = 56.0;

#[allow(clippy::too_many_arguments)]
fn draw_decoration_catalog(
    mut contexts: EguiContexts,
    build_mode: Option<ResMut<BuildMode>>,
    placeables: Res<PlaceableItems>,
    registry: Res<ItemRegistry>,
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    catalog: Res<InteriorCatalog>,
    mut state: Local<DecorationCatalogState>,
) -> Result {
    let Some(mut mode) = build_mode else { return Ok(()) };

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
        // `interior_category` and no thumbnail. Skip them — the grid is
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
    // `Assets<Image>` — until then the row falls back to a text-only
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
                            // Wrapping grid of thumbnail buttons — egui
                            // re-flows them based on available width when
                            // the player resizes the catalog.
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for row in &visible {
                                    let active = row.idx == mode.selected
                                        && mode.tool == BuildTool::Place;
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
                                        let item_id = placeables.0[row.idx];
                                        mode.tool = BuildTool::Place;
                                        mode.selected = row.idx;
                                        refresh_build_preview(
                                            &mut commands,
                                            &mut mode,
                                            item_id,
                                            &registry,
                                            &asset_server,
                                            &mut meshes,
                                            &mut materials,
                                            &catalog,
                                        );
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

