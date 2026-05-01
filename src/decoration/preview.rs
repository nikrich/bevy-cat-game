//! Decoration ghost preview and placement computation.

use bevy::prelude::*;

use super::physics::*;
use crate::edit::PlacedItem;
use crate::input::{CursorHit, CursorState};
use crate::items::{Form, InteriorCatalog, ItemRegistry};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;
use super::{DecorationMode, DecorationTool};

/// Marker for the decoration ghost preview entity. One at a time.
/// Carries the `ItemId` of the piece the ghost is currently representing
/// (so `update_preview` can detect a selection change and respawn with
/// the right mesh) and the material handles for the body + forward face
/// (so the system can recolor in-place each frame when placement
/// validity flips between OK / blocked).
#[derive(Component)]
pub struct DecorationPreview {
    pub item: crate::items::ItemId,
    pub body_mat: Handle<StandardMaterial>,
    pub face_mat: Handle<StandardMaterial>,
}

/// Body / face colors for the OK and BLOCKED ghost states. Centralised
/// so toggling between them in `update_preview` is a single lookup.
fn ghost_body_color(blocked: bool) -> Color {
    if blocked {
        Color::srgba(0.95, 0.30, 0.30, 0.45)
    } else {
        Color::srgba(0.40, 0.90, 0.60, 0.40)
    }
}

fn ghost_face_color(blocked: bool) -> Color {
    if blocked {
        Color::srgba(1.0, 0.20, 0.15, 0.65)
    } else {
        Color::srgba(0.55, 1.00, 0.00, 0.55)
    }
}

fn ghost_face_emissive(blocked: bool) -> LinearRgba {
    if blocked {
        LinearRgba::from(Color::srgb(2.0, 0.35, 0.20))
    } else {
        LinearRgba::from(Color::srgb(0.9, 2.0, 0.2))
    }
}

/// Decide whether the ghost at `pos` would overlap an existing placed
/// piece in a way that should block placement. Two narrow conditions,
/// kept tight on purpose -- the previous AABB-with-buffer approach
/// false-positive-blocked beds inside small rooms because the bed AABB
/// extended toward distant walls.
///
/// 1. **Cursor on structural top face**: hovering on a wall / door /
///    window's top means the player is trying to stack furniture on a
///    1m-thick wall. Block.
/// 2. **Ghost centre inside a wall**: the centre of the ghost sits
///    inside a 1x1x1 structural cube. Block.
///
/// Floors / carpets / other furniture are allowed under or beside the
/// ghost on purpose (carpet-under-table, lamp-on-chest, bed-against-wall).
pub fn is_decoration_blocked<F: bevy::ecs::query::QueryFilter>(
    pos: Vec3,
    cursor_hit: Option<CursorHit>,
    placed_q: &Query<(&Transform, &PlacedItem), F>,
    registry: &ItemRegistry,
) -> bool {
    use crate::items::ItemTags;

    if let Some(hit) = cursor_hit {
        if hit.normal.y > 0.5 {
            if let Ok((_, item)) = placed_q.get(hit.entity) {
                if let Some(def) = registry.get(item.item) {
                    if def.tags.contains(ItemTags::STRUCTURAL)
                        && !matches!(def.form, Form::Floor)
                    {
                        return true;
                    }
                }
            }
        }
    }

    placed_q.iter().any(|(tf, item)| {
        let Some(def) = registry.get(item.item) else { return false };
        if matches!(def.form, Form::Floor) {
            return false;
        }
        if !def.tags.contains(ItemTags::STRUCTURAL) {
            return false;
        }
        let dx = (tf.translation.x - pos.x).abs();
        let dy = (tf.translation.y - pos.y).abs();
        let dz = (tf.translation.z - pos.z).abs();
        // Ghost centre inside the wall's 1x1x1 cube.
        dx < 0.5 && dz < 0.5 && dy < 0.5
    })
}

pub fn update_preview(
    mut commands: Commands,
    decoration_mode: Option<ResMut<DecorationMode>>,
    cursor: Res<CursorState>,
    placed_q: Query<(&Transform, &PlacedItem), Without<DecorationPreview>>,
    registry: Res<ItemRegistry>,
    terrain: Res<Terrain>,
    noise: Res<WorldNoise>,
    placeables: Res<crate::building::PlaceableItems>,
    catalog: Res<InteriorCatalog>,
    mut preview_q: Query<(Entity, &DecorationPreview, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mode) = decoration_mode else {
        // Mode off -- despawn any lingering preview.
        for (e, _, _) in &preview_q {
            commands.entity(e).despawn();
        }
        return;
    };
    if !matches!(mode.tool, DecorationTool::Place) {
        for (e, _, _) in &preview_q {
            commands.entity(e).despawn();
        }
        return;
    }
    let Some(item_id) = placeables.0.get(mode.selected).copied() else { return };
    let Some(def) = registry.get(item_id) else { return };

    let pos = compute_decoration_placement(
        cursor.cursor_world.unwrap_or(Vec3::ZERO),
        cursor.cursor_hit,
        def,
        &placed_q,
        &registry,
        &terrain,
        &noise,
        &catalog,
    );
    let blocked = is_decoration_blocked(pos, cursor.cursor_hit, &placed_q, &registry);

    // Ghost cuboid dimensions, used by the spawn branch for mesh sizing
    // and forward-indicator placement. For Form::Interior we size from
    // the AABB so the ghost matches the asset footprint.
    let dims = if matches!(def.form, Form::Interior) {
        def.interior_name
            .as_deref()
            .and_then(|name| catalog.aabb_for(name))
            .map(|aabb| {
                let scale = def.form.placement_scale();
                aabb.size() * scale
            })
            .unwrap_or(Vec3::splat(0.6))
    } else {
        Vec3::new(1.0, 0.8, 1.0)
    };

    // Reuse the existing ghost only when it represents the same item. If
    // the player picked a different piece in the catalog (`item_id`
    // changed), despawn so the spawn branch below rebuilds with the new
    // mesh / size. Without this the ghost shape gets stuck on the first
    // selection.
    let needs_respawn = match preview_q.single_mut() {
        Ok((entity, prev, mut tf)) => {
            if prev.item != item_id {
                commands.entity(entity).despawn();
                true
            } else {
                tf.translation = pos;
                tf.rotation = Quat::from_rotation_y(mode.rotation_radians);
                // Update body / face colors in-place each frame so the
                // ghost flips between OK and blocked tints as the cursor
                // crosses obstacles.
                if let Some(body) = materials.get_mut(&prev.body_mat) {
                    body.base_color = ghost_body_color(blocked);
                }
                if let Some(face) = materials.get_mut(&prev.face_mat) {
                    face.base_color = ghost_face_color(blocked);
                    face.emissive = ghost_face_emissive(blocked);
                }
                false
            }
        }
        Err(_) => true,
    };

    if needs_respawn {
        // dims was computed above so we could pass it to the blocked
        // check; reuse the same value here for mesh sizing.
        let body_mesh = if matches!(def.form, Form::Interior) {
            meshes.add(Mesh::from(Cuboid::new(dims.x, dims.y, dims.z)))
        } else {
            meshes.add(def.form.make_mesh())
        };
        let body_mat = materials.add(StandardMaterial {
            base_color: ghost_body_color(blocked),
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        // Forward indicator: a translucent lime slab pressed against the
        // piece's +Z face (its local forward). Reads at a glance like a
        // colored front door so the player sees orientation from any
        // camera angle. Translucent + emissive so it blends with the
        // body but stays visible against shaded interiors. Flips to red
        // when the position is blocked.
        let face_mesh = meshes.add(Mesh::from(Cuboid::new(
            dims.x * 0.95,
            dims.y * 0.95,
            0.02,
        )));
        let face_mat = materials.add(StandardMaterial {
            base_color: ghost_face_color(blocked),
            emissive: ghost_face_emissive(blocked),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        });
        let face_tf =
            Transform::from_translation(Vec3::new(0.0, 0.0, dims.z * 0.5 + 0.01));

        commands
            .spawn((
                DecorationPreview {
                    item: item_id,
                    body_mat: body_mat.clone(),
                    face_mat: face_mat.clone(),
                },
                Mesh3d(body_mesh),
                MeshMaterial3d(body_mat),
                Transform::from_translation(pos)
                    .with_rotation(Quat::from_rotation_y(mode.rotation_radians)),
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(face_mesh),
                    MeshMaterial3d(face_mat),
                    face_tf,
                ));
            });
    }
}

/// Top-level placement decision. Calls `pick_attach_surface` then snaps
/// XZ via `snap_to_fine_grid`. v1 -- no magnet anchors.
///
/// Lift / Y handling: most forms use `def.form.placement_lift()` directly
/// (a flat per-form constant). `Form::Interior` items have varying GLB
/// origins, so the lift is computed from their AABB via
/// `interior_render_params` -- without this, every interior asset sinks
/// into or floats above the surface depending on where the GLB author
/// put the origin.
///
/// The query filter is generic so callers can narrow further (e.g. the
/// Move tool excludes the currently-carried entity to avoid a query
/// conflict against its mutable Transform).
pub fn compute_decoration_placement<F: bevy::ecs::query::QueryFilter>(
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    def: &crate::items::ItemDef,
    placed_q: &Query<(&Transform, &PlacedItem), F>,
    registry: &ItemRegistry,
    terrain: &Terrain,
    noise: &WorldNoise,
    catalog: &InteriorCatalog,
) -> Vec3 {
    let lift = if matches!(def.form, Form::Interior) {
        def.interior_name
            .as_deref()
            .and_then(|name| catalog.aabb_for(name))
            .map(|aabb| {
                let scale = def.form.placement_scale();
                let (_, _, effective) = super::interior::interior_render_params(def, aabb);
                scale * effective.size().y * 0.5
            })
            .unwrap_or_else(|| def.form.placement_lift())
    } else {
        def.form.placement_lift()
    };

    let input = if let Some(hit) = cursor_hit {
        if let Ok((tf, building)) = placed_q.get(hit.entity) {
            let hit_def = registry.get(building.item);
            let top_y = tf.translation.y + hit_def.map(|d| d.form.placement_lift()).unwrap_or(0.0);
            let kind = if hit_def.map_or(false, |d| matches!(d.form, Form::Floor)) {
                AttachInputKind::Floor { top_y }
            } else {
                AttachInputKind::OtherPlaced { top_y }
            };
            AttachInput { point: hit.point, normal: hit.normal, kind }
        } else {
            AttachInput { point: hit.point, normal: hit.normal, kind: AttachInputKind::Terrain }
        }
    } else {
        AttachInput {
            point: cursor_world,
            normal: Vec3::Y,
            kind: AttachInputKind::Terrain,
        }
    };

    let surface = pick_attach_surface(input);
    match surface {
        AttachSurface::Terrain { xz } => {
            let x = snap_to_fine_grid(xz.x);
            let z = snap_to_fine_grid(xz.z);
            let y = terrain.height_at_or_sample(x, z, noise);
            Vec3::new(x, y + lift, z)
        }
        AttachSurface::FloorTop { xz, top_y } => {
            let x = snap_to_fine_grid(xz.x);
            let z = snap_to_fine_grid(xz.z);
            Vec3::new(x, top_y + lift, z)
        }
        AttachSurface::FurnitureTop { xz, top_y } => {
            let x = snap_to_fine_grid(xz.x);
            let z = snap_to_fine_grid(xz.z);
            Vec3::new(x, top_y + lift, z)
        }
        AttachSurface::WallFace { point, normal } => {
            let off = normal.normalize() * 0.05;
            let world = point + off;
            let x = snap_to_fine_grid(world.x);
            let z = snap_to_fine_grid(world.z);
            Vec3::new(x, world.y, z)
        }
    }
}
