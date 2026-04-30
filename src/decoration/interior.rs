use bevy::gltf::{Gltf, GltfMesh, GltfNode};
use bevy::prelude::*;
use crate::edit::PlacedItem;
use crate::input::CursorHit;
use crate::items::{AabbBounds, Form, ItemDef, ItemRegistry};
use crate::world::biome::WorldNoise;
use crate::world::terrain::Terrain;
use crate::building::placement::{snap_axis, cube_target_width, OCCUPIED_RADIUS, OCCUPIED_Y};
use crate::building::BuildPreview;

/// Marker for an entity whose interior mesh+material are pending GLB
/// load. `resolve_interior_spawns` polls these and once the named node
/// resolves, spawns Mesh3d+MeshMaterial3d children for each primitive.
/// Removed from the entity once resolved.
///
/// `child_offset` and `child_scale_mul` are pre-computed at spawn time
/// (when we already have the catalog AABB) so resolve doesn't need
/// registry / catalog access. They make every interior asset render with
/// its AABB centred at the parent transform — without that, GLB nodes
/// whose origin is at a corner / floor placed visibly off-grid even with
/// correct snap.
#[derive(Component)]
pub struct InteriorSpawnRequest {
    pub gltf: Handle<Gltf>,
    pub node_name: String,
    /// Local translation applied to each spawned child mesh. Cancels the
    /// asset's intrinsic origin offset (set to `-aabb.centre`).
    pub child_offset: Vec3,
    /// Per-axis scale multiplier applied on top of the GLB node's own
    /// scale. `Vec3::ONE` for most assets; doors set `x` so the world
    /// width = 1 cube cell.
    pub child_scale_mul: Vec3,
}

/// Compute the per-asset child offset (cancels intrinsic origin offset so
/// the AABB centres at the parent transform) and per-axis scale multiplier
/// (door / window categories get x stretched to fit a fixed cube width).
/// Returns `(child_offset, child_scale_mul, effective_aabb)` where the
/// effective AABB is the post-stretch AABB used for footprint and Y
/// placement.
pub(crate) fn interior_render_params(
    def: &ItemDef,
    aabb: AabbBounds,
) -> (Vec3, Vec3, AabbBounds) {
    let parent_scale = def.form.placement_scale();
    let mut scale_mul = Vec3::ONE;
    let mut effective = aabb;
    if let Some(target_world_x) = cube_target_width(def) {
        if aabb.size().x > 1e-4 {
            // World X = parent_scale * scale_mul.x * aabb.size().x → target
            let target = target_world_x / (parent_scale * aabb.size().x);
            scale_mul.x = target;
            effective = AabbBounds {
                min: Vec3::new(aabb.min.x * target, aabb.min.y, aabb.min.z),
                max: Vec3::new(aabb.max.x * target, aabb.max.y, aabb.max.z),
            };
        }
    }
    // Recentre: the child meshes already get scaled by node TRS + scale_mul,
    // so we shift them by -effective.centre() to land the AABB centre at
    // the parent's local origin.
    let child_offset = -effective.center();
    (child_offset, scale_mul, effective)
}

/// What an in-progress interior placement counts as "in the way".
#[derive(Clone, Copy)]
pub(crate) enum BlockingRule {
    /// Default for furniture / props / decorations: every placed piece
    /// blocks except floors (which are explicit "stand-on-top" surfaces).
    AnyExceptFloor,
    /// Carpets only: walls are the only blockers. Floors, other carpets,
    /// chairs, tables, lamps — all fine to overlap (a carpet visually
    /// goes under the items in the room).
    WallsOnly,
}

pub(crate) fn blocking_rule_for(def: &ItemDef) -> BlockingRule {
    if matches!(def.interior_category.as_deref(), Some("carpet")) {
        BlockingRule::WallsOnly
    } else {
        BlockingRule::AnyExceptFloor
    }
}

/// True if every cell in the footprint at `centre` is clear of placed
/// pieces under the given `rule`. See [`BlockingRule`] for which forms
/// count as blockers.
pub(crate) fn footprint_clear(
    centre: Vec3,
    cells: IVec2,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    registry: &ItemRegistry,
    rule: BlockingRule,
) -> bool {
    use crate::building::placement::footprint_cell_centres;
    footprint_cell_centres(centre, cells).into_iter().all(|c| {
        placed_q.iter().all(|(tf, b)| {
            let def = match registry.get(b.item) {
                Some(d) => d,
                None => return true,
            };
            let blocks = match rule {
                BlockingRule::AnyExceptFloor => !matches!(def.form, Form::Floor),
                BlockingRule::WallsOnly => matches!(def.form, Form::Wall),
            };
            if !blocks {
                return true;
            }
            !((tf.translation.x - c.x).abs() < OCCUPIED_RADIUS
                && (tf.translation.z - c.z).abs() < OCCUPIED_RADIUS
                && (tf.translation.y - c.y).abs() < OCCUPIED_Y)
        })
    })
}

/// Interior-item placement that respects the asset's pre-computed AABB:
/// snap XZ to the cube grid based on the asset's footprint cell count, set
/// Y so the asset's bottom rests exactly on the hit surface (terrain, wall
/// top, or floor top). Returns the entity *centre* position.
///
/// The AABB used for footprint + Y is the **effective** AABB from
/// `interior_render_params` — that's the post-stretch AABB for door
/// categories (forced 1m wide), and the original AABB for everything else.
/// `resolve_interior_spawns` recentres the children by `-effective.centre()`
/// so the rendered AABB ends up centred on the entity, hence the Y formula
/// is `scale * size.y / 2` rather than `-min.y * scale`.
pub(crate) fn compute_interior_placement(
    cursor_world: Vec3,
    cursor_hit: Option<CursorHit>,
    def: &ItemDef,
    aabb: AabbBounds,
    placed_q: &Query<(&Transform, &PlacedItem), Without<BuildPreview>>,
    registry: &ItemRegistry,
    terrain: &Terrain,
    noise: &WorldNoise,
) -> (Vec3, IVec2) {
    let scale = def.form.placement_scale();
    let (_, _, effective) = interior_render_params(def, aabb);
    let footprint = effective.footprint_cells(scale);
    let bottom_offset = scale * effective.size().y * 0.5;

    // Pick the surface XZ + Y. Walls / floors snap to their top; terrain hit
    // snaps to terrain height; otherwise fall back to the cursor's ground
    // projection. Terrain height is sampled at the *snapped* XZ so the
    // asset doesn't bob when the cursor moves within a single cell.
    let (raw_x, raw_z, surface_y_at) = if let Some(hit) = cursor_hit {
        if let Ok((tf, building)) = placed_q.get(hit.entity) {
            if let Some(hit_def) = registry.get(building.item) {
                let hit_top = tf.translation.y + hit_def.form.placement_lift();
                if hit.normal.y > 0.7 {
                    (tf.translation.x, tf.translation.z, Some(hit_top))
                } else {
                    (hit.point.x, hit.point.z, None)
                }
            } else {
                (hit.point.x, hit.point.z, None)
            }
        } else {
            (hit.point.x, hit.point.z, None)
        }
    } else {
        (cursor_world.x, cursor_world.z, None)
    };

    // Wall-element categories (door / window) snap *both* axes to cell
    // boundaries (integer XZ). Z is forced to integer regardless of the
    // asset's natural depth — even though footprint.z is usually 1 (so
    // `snap_axis` would give a cell centre), the door / window itself only
    // makes sense sitting on the line where two perpendicular walls would
    // meet, so we override.
    let force_integer_z = cube_target_width(def).is_some();
    let snap_x = snap_axis(raw_x, footprint.x);
    let snap_z = if force_integer_z {
        raw_z.round()
    } else {
        snap_axis(raw_z, footprint.y)
    };
    let surface_y = surface_y_at
        .unwrap_or_else(|| terrain.height_at_or_sample(snap_x, snap_z, noise));
    (Vec3::new(snap_x, surface_y + bottom_offset, snap_z), footprint)
}

/// Async resolver for `InteriorSpawnRequest` — once the parent GLB is
/// loaded and the named node's mesh asset is ready, spawn one
/// Mesh3d+MeshMaterial3d child per primitive on the placed entity, then
/// remove the request component. The node's local transform is ignored;
/// the placed entity already carries the world transform from placement.
pub(crate) fn resolve_interior_spawns(
    mut commands: Commands,
    requests: Query<(Entity, &InteriorSpawnRequest)>,
    gltfs: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<GltfNode>>,
    gltf_meshes: Res<Assets<GltfMesh>>,
) {
    for (entity, req) in &requests {
        let Some(gltf) = gltfs.get(&req.gltf) else { continue };
        let Some(node_handle) = gltf.named_nodes.get(req.node_name.as_str()) else {
            warn!("[interior] node '{}' not in parent GLB", req.node_name);
            commands.entity(entity).remove::<InteriorSpawnRequest>();
            continue;
        };
        let Some(node) = gltf_nodes.get(node_handle) else { continue };
        let Some(mesh_handle) = node.mesh.as_ref() else {
            // Some nodes are bare transforms with no mesh — nothing to render.
            commands.entity(entity).remove::<InteriorSpawnRequest>();
            continue;
        };
        let Some(gltf_mesh) = gltf_meshes.get(mesh_handle) else { continue };

        // Preserve the node's rotation + scale so per-item authoring
        // intent (e.g. plant.008 has scale 0.108) carries through.
        // Translation comes from the source scene's grid layout — we
        // explicitly drop it. `child_offset` re-shifts so the asset's AABB
        // centre lands at the parent transform; `child_scale_mul` adds the
        // door-width stretch on top of the node scale.
        let local_tf = Transform {
            translation: req.child_offset,
            rotation: node.transform.rotation,
            scale: node.transform.scale * req.child_scale_mul,
        };
        commands.entity(entity).with_children(|parent| {
            for prim in &gltf_mesh.primitives {
                let mat = prim
                    .material
                    .clone()
                    .unwrap_or_default();
                parent.spawn((
                    Mesh3d(prim.mesh.clone()),
                    MeshMaterial3d(mat),
                    local_tf,
                ));
            }
        });
        commands.entity(entity).remove::<InteriorSpawnRequest>();
    }
}
